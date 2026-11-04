//! Tracked SQL migrations: apply plain `.sql` files once each, in a fixed order, and record what
//! was applied in a table.
//!
//! A *plan* lists the files in the order to apply them. Each file is identified by its path as the
//! plan writes it (relative to a root directory), so the identity survives moving the checkout or
//! the container image. Every run:
//!
//! 1. takes a Postgres advisory lock, so two runners never apply at once;
//! 2. creates the tracking table and its runs table if they do not exist;
//! 3. on an empty tracking table only, records the *baseline* (files a database already has,
//!    without running them);
//! 4. checks every applied *versioned* file against its stored checksum and fails, before
//!    applying anything, if one changed;
//! 5. applies, in plan order, every versioned file not yet applied and every *repeatable* file
//!    that is new or whose checksum changed, recording each file as it completes;
//! 6. writes one summary row for the run.
//!
//! A file runs statement by statement in autocommit, as `psql -v ON_ERROR_STOP=1 -f` does, so
//! statements that cannot run in a transaction (`CREATE INDEX CONCURRENTLY`) work. A file whose
//! leading comments contain `-- lifeguard:single-transaction` runs in one transaction together
//! with its tracking row instead. A file is repeatable when the plan says so (`repeatable <path>`)
//! or its leading comments contain `-- lifeguard:repeatable`.
//!
//! See `README.md` (Tracked SQL migrations) for the tables, the plan format and adoption.

use may_postgres::Client;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Tracking table used when none is given.
pub const DEFAULT_TABLE: &str = "lifeguard_schema_migrations";

/// How a file is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// Applied once. Editing it afterwards fails the next run.
    Versioned,
    /// Applied again whenever its checksum changes (views, functions, policies, seeds).
    Repeatable,
}

impl FileKind {
    /// The value stored in the tracking table's `kind` column.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            FileKind::Versioned => "versioned",
            FileKind::Repeatable => "repeatable",
        }
    }
}

/// One file of a plan, read from disk.
#[derive(Debug, Clone)]
pub struct PlannedFile {
    /// Identity recorded in the tracking table: the path as the plan writes it.
    pub id: String,
    /// Where the file was read from.
    pub path: PathBuf,
    /// Versioned or repeatable.
    pub kind: FileKind,
    /// Run the whole file and its tracking row in one transaction.
    pub single_transaction: bool,
    /// SHA-256 of the file's bytes, lowercase hex (what `sha256sum` prints).
    pub checksum: String,
    /// The file's text, as read.
    pub sql: String,
}

/// What a run does with a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Not applied before: apply it.
    Apply,
    /// Repeatable file whose checksum changed (or that failed last time): apply it again.
    Reapply,
    /// Already applied with this checksum.
    Skip,
    /// Recorded from the baseline, not run.
    Baseline,
}

/// Which files an empty tracking table starts with.
#[derive(Debug, Clone, Default)]
pub enum Baseline {
    /// None: every file of the plan is applied.
    #[default]
    None,
    /// Every file of the plan is recorded as applied, with its current checksum, and nothing runs.
    /// For a database that already has the whole plan.
    All,
    /// The files a query returns. The query returns the file identity in its first column and,
    /// optionally, the checksum it was applied with in its second (`NULL`, or no second column:
    /// the file's current checksum). Plan files the query does not return are applied as usual.
    Query(String),
}

/// Settings for [`run`].
#[derive(Debug, Clone)]
pub struct Options {
    /// Tracking table, optionally schema-qualified (`app.schema_migrations`). The runs table is
    /// the same name with `_runs` appended. The schema must exist.
    pub table: String,
    /// Release (version, tag, image) recorded with every file this run applies and on its run row.
    pub release: String,
    /// SQL run before each file, e.g. `SET search_path TO app, public`.
    pub preamble: Option<String>,
    /// Baseline for an empty tracking table.
    pub baseline: Baseline,
    /// How long to wait for another runner to finish.
    pub lock_timeout: Duration,
    /// Report what would happen; write nothing.
    pub dry_run: bool,
    /// Start of the run's first log line (for example the tool version and the database, never a
    /// password), logged with the number of pending files once the lock is held.
    pub label: Option<String>,
}

impl Options {
    /// Defaults: [`DEFAULT_TABLE`], no preamble, no baseline, a 5-minute lock wait.
    #[must_use]
    pub fn new(release: impl Into<String>) -> Self {
        Self {
            table: DEFAULT_TABLE.to_string(),
            release: release.into(),
            preamble: None,
            baseline: Baseline::None,
            lock_timeout: Duration::from_secs(300),
            dry_run: false,
            label: None,
        }
    }
}

/// Rewrites a file's text before it runs (for example environment-specific role names). The
/// checksum is always taken from the file as written, never from the rewritten text.
pub type Filter<'a> = &'a dyn Fn(&PlannedFile) -> Result<String, String>;

/// Outcome of one file in a run.
#[derive(Debug, Clone)]
pub struct FileOutcome {
    /// The file's identity.
    pub id: String,
    /// What the run did (or, in a dry run, would do).
    pub action: Action,
    /// Time spent applying it, for applied files.
    pub duration: Option<Duration>,
}

/// Summary of a run.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// Row id in the runs table (`None` for a dry run).
    pub run_id: Option<i64>,
    /// Every plan file, in order.
    pub files: Vec<FileOutcome>,
    /// Tracked files the plan no longer lists (reported, not an error).
    pub not_in_plan: Vec<String>,
}

impl Report {
    /// Number of files with this action.
    #[must_use]
    pub fn count(&self, action: Action) -> usize {
        self.files.iter().filter(|f| f.action == action).count()
    }
}

/// A file whose stored checksum no longer matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    /// The file's identity.
    pub id: String,
    /// Checksum it was applied (or baselined) with.
    pub stored: String,
    /// Checksum of the file now.
    pub current: String,
}

/// Errors from tracked SQL migrations.
#[derive(Debug)]
pub enum TrackedError {
    /// A plan or SQL file could not be read.
    Io {
        /// The path.
        path: PathBuf,
        /// What went wrong.
        error: std::io::Error,
    },
    /// The plan, a file or the options are malformed.
    Invalid(String),
    /// Applied versioned files were edited. Nothing was applied.
    ChecksumMismatch {
        /// Tracking table, for the message.
        table: String,
        /// The edited files.
        files: Vec<ChangedFile>,
    },
    /// A statement of a file failed. Statements before it in an autocommit file stay applied.
    Statement {
        /// The file's identity.
        file: String,
        /// 1-based statement number within the file.
        index: usize,
        /// 1-based line where the statement starts.
        line: usize,
        /// The start of the statement.
        statement: String,
        /// The database's error.
        error: String,
    },
    /// The filter rejected a file.
    Filter {
        /// The file's identity.
        file: String,
        /// The filter's error.
        error: String,
    },
    /// Another runner held the lock for the whole lock timeout.
    LockTimeout(Duration),
    /// A tracking query failed.
    Database(String),
}

impl fmt::Display for TrackedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrackedError::Io { path, error } => {
                write!(f, "cannot read {}: {error}", path.display())
            }
            TrackedError::Invalid(msg) => write!(f, "{msg}"),
            TrackedError::ChecksumMismatch { table, files } => {
                writeln!(
                    f,
                    "{} applied migration file(s) changed after they were applied; nothing was applied:",
                    files.len()
                )?;
                for c in files {
                    writeln!(
                        f,
                        "  {} (applied {}, now {})",
                        c.id,
                        short(&c.stored),
                        short(&c.current)
                    )?;
                }
                write!(
                    f,
                    "Put the change in a new migration file and restore the applied file. A view, function or \
                     seed that should follow its file belongs in a repeatable file. If the edit is only cosmetic \
                     and must be accepted, update `checksum` for that file in {table} deliberately."
                )
            }
            TrackedError::Statement {
                file,
                index,
                line,
                statement,
                error,
            } => write!(
                f,
                "{file}: statement {index} (line {line}) failed: {error}; statement: {statement}"
            ),
            TrackedError::Filter { file, error } => write!(f, "{file}: filter failed: {error}"),
            TrackedError::LockTimeout(waited) => write!(
                f,
                "another migration run held the lock for {}s; try again when it has finished",
                waited.as_secs()
            ),
            TrackedError::Database(msg) => write!(f, "tracking query failed: {msg}"),
        }
    }
}

impl std::error::Error for TrackedError {}

fn short(checksum: &str) -> &str {
    checksum.get(..12).unwrap_or(checksum)
}

/// `s` on one line: its lines trimmed and joined with ` | ` (one log entry per message).
fn one_line(s: &str) -> String {
    s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// The server's error as one line: SQLSTATE, message, detail, hint, position (1-based character
/// offset into the statement) and context.
fn db_error(e: &may_postgres::Error) -> String {
    match e.as_db_error() {
        Some(db) => {
            let mut s = format!("{}: {}", db.code().code(), one_line(db.message()));
            if let Some(d) = db.detail() {
                s.push_str(&*format!(" (detail: {})", one_line(d)));
            }
            if let Some(h) = db.hint() {
                s.push_str(&*format!(" (hint: {})", one_line(h)));
            }
            match db.position() {
                Some(may_postgres::error::ErrorPosition::Original(p)) => {
                    s.push_str(&*format!(" (position: {p})"));
                }
                Some(may_postgres::error::ErrorPosition::Internal { position, query }) => {
                    let q: String = one_line(query).chars().take(200).collect();
                    s.push_str(&*format!(" (internal position {position} of: {q})"));
                }
                None => {}
            }
            if let Some(w) = db.where_() {
                s.push_str(&*format!(" (where: {})", one_line(w)));
            }
            s
        }
        None => one_line(&e.to_string()),
    }
}

/// The first 200 characters of a statement on one line, `…` when cut.
pub fn statement_head(stmt: &str) -> String {
    let flat = stmt.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut head: String = flat.chars().take(200).collect();
    if flat.chars().count() > 200 {
        head.push('…');
    }
    head
}

/// Where a connection string points, without credentials: `host[:port]/database` (`?` for a
/// part it does not say). Accepts `postgres://` / `postgresql://` URLs and `key=value` strings.
pub fn redacted_target(url: &str) -> String {
    let u = url.trim();
    let (mut host, mut port, mut db) = (String::new(), String::new(), String::new());
    let rest = u
        .strip_prefix("postgresql://")
        .or_else(|| u.strip_prefix("postgres://"));
    if let Some(rest) = rest {
        // Everything up to the last `@` is credentials, whatever characters the password has.
        let rest = rest.rfind('@').map_or(rest, |i| &rest[i + 1..]);
        let (main, query) = rest.split_once('?').unwrap_or((rest, ""));
        let (authority, path) = main.split_once('/').unwrap_or((main, ""));
        host = authority.to_string();
        db = path.to_string();
        for kv in query.split('&') {
            match kv.split_once('=') {
                Some(("host", v)) if host.is_empty() => host = v.to_string(),
                Some(("port", v)) if !host.contains(':') => port = v.to_string(),
                Some(("dbname", v)) if db.is_empty() => db = v.to_string(),
                _ => {}
            }
        }
    } else {
        for kv in u.split_whitespace() {
            let unq = |v: &str| v.trim_matches('\'').to_string();
            match kv.split_once('=') {
                Some(("host" | "hostaddr", v)) if host.is_empty() => host = unq(v),
                Some(("port", v)) => port = unq(v),
                Some(("dbname", v)) => db = unq(v),
                _ => {}
            }
        }
    }
    if host.is_empty() {
        host = "?".to_string();
    }
    if !port.is_empty() {
        host = format!("{host}:{port}");
    }
    if db.is_empty() {
        db = "?".to_string();
    }
    format!("{host}/{db}")
}

fn dbe(e: may_postgres::Error) -> TrackedError {
    TrackedError::Database(db_error(&e))
}

/// SHA-256 of `bytes` as lowercase hex.
#[must_use]
pub fn checksum(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&*format!("{b:02x}"));
    }
    s
}

/// Plan entries: `(path, kind the plan gives)`. One path per line; blank lines and lines starting
/// with `#` are ignored; `repeatable <path>` or `versioned <path>` sets the kind.
///
/// # Errors
///
/// Returns [`TrackedError::Invalid`] for an entry with no path or a path listed twice.
pub fn parse_plan(text: &str) -> Result<Vec<(String, Option<FileKind>)>, TrackedError> {
    let mut out: Vec<(String, Option<FileKind>)> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (kind, path) = match line.split_once(char::is_whitespace) {
            Some((w, rest)) if w.eq_ignore_ascii_case("repeatable") => {
                (Some(FileKind::Repeatable), rest.trim())
            }
            Some((w, rest)) if w.eq_ignore_ascii_case("versioned") => {
                (Some(FileKind::Versioned), rest.trim())
            }
            _ => (None, line),
        };
        if path.is_empty() {
            return Err(TrackedError::Invalid(format!(
                "plan line {}: no path",
                n + 1
            )));
        }
        if out.iter().any(|(p, _)| p == path) {
            return Err(TrackedError::Invalid(format!(
                "plan line {}: {path} is listed twice",
                n + 1
            )));
        }
        out.push((path.to_string(), kind));
    }
    Ok(out)
}

/// Directives in a file's leading comment lines: `(repeatable, single_transaction)`.
#[must_use]
pub fn directives(sql: &str) -> (bool, bool) {
    let (mut repeatable, mut single) = (false, false);
    for line in sql.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let Some(comment) = t.strip_prefix("--") else {
            break;
        };
        let c = comment.trim().to_ascii_lowercase();
        if let Some(d) = c.strip_prefix("lifeguard:") {
            match d.trim() {
                "repeatable" => repeatable = true,
                "single-transaction" => single = true,
                _ => {}
            }
        }
    }
    (repeatable, single)
}

/// Read every file a plan lists. Paths are relative to `root`.
///
/// # Errors
///
/// Returns [`TrackedError::Io`] when the plan or a file cannot be read and
/// [`TrackedError::Invalid`] for a malformed plan.
pub fn load_plan(plan: &Path, root: &Path) -> Result<Vec<PlannedFile>, TrackedError> {
    let text = std::fs::read_to_string(plan).map_err(|error| TrackedError::Io {
        path: plan.to_path_buf(),
        error,
    })?;
    load_entries(&parse_plan(&text)?, root)
}

/// Read the files of parsed plan entries (see [`parse_plan`]). Paths are relative to `root`.
///
/// # Errors
///
/// Returns [`TrackedError::Io`] when a file cannot be read.
pub fn load_entries(
    entries: &[(String, Option<FileKind>)],
    root: &Path,
) -> Result<Vec<PlannedFile>, TrackedError> {
    entries
        .iter()
        .map(|(id, kind)| {
            let path = root.join(id);
            let bytes = std::fs::read(&path).map_err(|error| TrackedError::Io {
                path: path.clone(),
                error,
            })?;
            let sql = String::from_utf8(bytes.clone())
                .map_err(|_| TrackedError::Invalid(format!("{id}: not UTF-8")))?;
            let (rep, single_transaction) = directives(&sql);
            // Repeatable when the plan or the file says so.
            let kind = if rep || *kind == Some(FileKind::Repeatable) {
                FileKind::Repeatable
            } else {
                FileKind::Versioned
            };
            Ok(PlannedFile {
                id: id.clone(),
                path,
                kind,
                single_transaction,
                checksum: checksum(&bytes),
                sql,
            })
        })
        .collect()
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c >= 0x80
}

fn is_ident_char(c: u8) -> bool {
    is_ident_start(c) || c.is_ascii_digit() || c == b'$'
}

fn line_of(sql: &str, offset: usize) -> usize {
    sql.as_bytes()[..offset]
        .iter()
        .filter(|&&c| c == b'\n')
        .count()
        + 1
}

/// End (exclusive) of the dollar-quote opening tag at `i`, if `i` starts one (`$$` or `$tag$`).
fn dollar_tag_end(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i + 1;
    if b.get(j) == Some(&b'$') {
        return Some(j + 1);
    }
    if !b.get(j).copied().is_some_and(is_ident_start) {
        return None; // `$1` parameter, or a lone `$`
    }
    while j < b.len() && is_ident_char(b[j]) && b[j] != b'$' {
        j += 1;
    }
    (b.get(j) == Some(&b'$')).then_some(j + 1)
}

/// Split SQL text into statements the way `psql` does: on `;` outside quotes, quoted identifiers,
/// dollar quotes, comments and `BEGIN ATOMIC .. END` bodies. Returns `(start line, statement)`;
/// statements made only of comments are dropped.
///
/// # Errors
///
/// Returns [`TrackedError::Invalid`] for an unterminated quote or comment, or a `psql`
/// meta-command (`\..`), which only `psql` understands.
pub fn split_statements(sql: &str) -> Result<Vec<(usize, String)>, TrackedError> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize; // start of the current statement's text
    let mut code_at: Option<usize> = None; // first code byte of the current statement
    let mut atomic = 0usize; // depth inside BEGIN ATOMIC .. END (CASE .. END nests)
    let mut prev_word: Option<&str> = None;
    let unterminated = |what: &str, at: usize| {
        TrackedError::Invalid(format!(
            "unterminated {what} starting on line {}",
            line_of(sql, at)
        ))
    };
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c == b'-' && b.get(i + 1) == Some(&b'-') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let open = i;
            let mut depth = 1;
            i += 2;
            while i < b.len() && depth > 0 {
                if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if depth > 0 {
                return Err(unterminated("comment", open));
            }
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if code_at.is_none() && (c != b';' || atomic > 0) {
            if c == b'\\' {
                return Err(TrackedError::Invalid(format!(
                    "line {}: psql meta-commands (\\..) are not supported",
                    line_of(sql, i)
                )));
            }
            code_at = Some(i);
        }
        match c {
            b'\'' => {
                let escapes =
                    i > 0 && matches!(b[i - 1], b'E' | b'e') && (i < 2 || !is_ident_char(b[i - 2]));
                let open = i;
                i += 1;
                loop {
                    match b.get(i) {
                        None => return Err(unterminated("string", open)),
                        Some(b'\\') if escapes => i += 2,
                        Some(b'\'') if b.get(i + 1) == Some(&b'\'') => i += 2,
                        Some(b'\'') => {
                            i += 1;
                            break;
                        }
                        Some(_) => i += 1,
                    }
                }
                prev_word = None;
            }
            b'"' => {
                let open = i;
                i += 1;
                loop {
                    match b.get(i) {
                        None => return Err(unterminated("quoted identifier", open)),
                        Some(b'"') if b.get(i + 1) == Some(&b'"') => i += 2,
                        Some(b'"') => {
                            i += 1;
                            break;
                        }
                        Some(_) => i += 1,
                    }
                }
                prev_word = None;
            }
            b'$' => {
                if let Some(end) = dollar_tag_end(b, i) {
                    let tag = &sql[i..end];
                    match sql[end..].find(tag) {
                        Some(p) => i = end + p + tag.len(),
                        None => return Err(unterminated("dollar-quoted string", i)),
                    }
                } else {
                    i += 1;
                }
                prev_word = None;
            }
            b';' if atomic == 0 => {
                if code_at.is_some() {
                    let text = sql[start..i].trim();
                    let line = line_of(sql, code_at.unwrap_or(start));
                    out.push((line, text.to_string()));
                }
                start = i + 1;
                code_at = None;
                prev_word = None;
                i += 1;
            }
            c if is_ident_start(c) => {
                let s = i;
                while i < b.len() && is_ident_char(b[i]) {
                    i += 1;
                }
                let w = &sql[s..i];
                if w.eq_ignore_ascii_case("atomic")
                    && prev_word.is_some_and(|p| p.eq_ignore_ascii_case("begin"))
                {
                    atomic += 1;
                } else if atomic > 0 && w.eq_ignore_ascii_case("case") {
                    atomic += 1;
                } else if atomic > 0 && w.eq_ignore_ascii_case("end") {
                    atomic -= 1;
                }
                prev_word = Some(w);
            }
            _ => {
                prev_word = None;
                i += 1;
            }
        }
    }
    if let Some(at) = code_at {
        out.push((line_of(sql, at), sql[start..].trim().to_string()));
    }
    Ok(out)
}

/// Tracking table and runs table names, validated.
struct Tables {
    files: String,
    runs: String,
}

impl Tables {
    fn new(table: &str) -> Result<Self, TrackedError> {
        let ok_part = |p: &str| {
            !p.is_empty()
                && p.len() <= 59
                && (p.as_bytes()[0].is_ascii_alphabetic() || p.as_bytes()[0] == b'_')
                && p.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        };
        let parts: Vec<&str> = table.split('.').collect();
        if parts.is_empty() || parts.len() > 2 || !parts.iter().all(|p| ok_part(p)) {
            return Err(TrackedError::Invalid(format!(
                "tracking table {table:?}: expected `name` or `schema.name` (letters, digits, underscores)"
            )));
        }
        Ok(Self {
            files: table.to_string(),
            runs: format!("{table}_runs"),
        })
    }

    fn create(&self, client: &Client) -> Result<(), TrackedError> {
        client
            .batch_execute(&*format!(
                "CREATE TABLE IF NOT EXISTS {f} (
                   file        text PRIMARY KEY,
                   kind        text NOT NULL CHECK (kind IN ('versioned', 'repeatable')),
                   checksum    text NOT NULL,
                   release     text NOT NULL,
                   status      text NOT NULL CHECK (status IN ('applied', 'baselined', 'failed')),
                   applied_at  timestamptz NOT NULL DEFAULT now(),
                   duration_ms bigint,
                   run_id      bigint,
                   error       text
                 );
                 CREATE TABLE IF NOT EXISTS {r} (
                   run_id      bigserial PRIMARY KEY,
                   release     text NOT NULL,
                   status      text NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
                   started_at  timestamptz NOT NULL DEFAULT now(),
                   finished_at timestamptz,
                   duration_ms bigint,
                   files       integer,
                   applied     integer,
                   reapplied   integer,
                   baselined   integer,
                   skipped     integer,
                   failed_file text,
                   error       text
                 );",
                f = self.files,
                r = self.runs
            ))
            .map_err(dbe)
    }

    fn exists(&self, client: &Client) -> Result<bool, TrackedError> {
        let row = client
            .query_one("SELECT to_regclass($1) IS NOT NULL", &[&self.files])
            .map_err(dbe)?;
        Ok(row.get(0))
    }
}

/// A tracked file: `(kind, checksum, status)`.
type Tracked = HashMap<String, (String, String, String)>;

fn load_tracked(client: &Client, t: &Tables) -> Result<Tracked, TrackedError> {
    let rows = client
        .query(
            &*format!("SELECT file, kind, checksum, status FROM {}", t.files),
            &[],
        )
        .map_err(dbe)?;
    Ok(rows
        .iter()
        .map(|r| (r.get::<_, String>(0), (r.get(1), r.get(2), r.get(3))))
        .collect())
}

/// What to do with each file, given the tracked state. Versioned files whose checksum changed are
/// returned separately.
fn decide(files: &[PlannedFile], tracked: &Tracked) -> (Vec<Action>, Vec<ChangedFile>) {
    let mut changed = Vec::new();
    let actions = files
        .iter()
        .map(|f| match tracked.get(&f.id) {
            None => Action::Apply,
            Some((_, _, status)) if status == "failed" => {
                if f.kind == FileKind::Repeatable {
                    Action::Reapply
                } else {
                    Action::Apply
                }
            }
            Some((_, sum, _)) if *sum == f.checksum => Action::Skip,
            Some((_, sum, _)) => {
                if f.kind == FileKind::Repeatable {
                    Action::Reapply
                } else {
                    changed.push(ChangedFile {
                        id: f.id.clone(),
                        stored: sum.clone(),
                        current: f.checksum.clone(),
                    });
                    Action::Skip
                }
            }
        })
        .collect();
    (actions, changed)
}

struct Lock<'a> {
    client: &'a Client,
    key: String,
}

impl<'a> Lock<'a> {
    fn acquire(client: &'a Client, key: &str, timeout: Duration) -> Result<Self, TrackedError> {
        let start = Instant::now();
        loop {
            let row = client
                .query_one("SELECT pg_try_advisory_lock(hashtext($1)::bigint)", &[&key])
                .map_err(dbe)?;
            if row.get::<_, bool>(0) {
                return Ok(Self {
                    client,
                    key: key.to_string(),
                });
            }
            if start.elapsed() >= timeout {
                return Err(TrackedError::LockTimeout(start.elapsed()));
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }
}

impl Drop for Lock<'_> {
    fn drop(&mut self) {
        let _ = self.client.query(
            "SELECT pg_advisory_unlock(hashtext($1)::bigint)",
            &[&self.key],
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn record(
    client: &Client,
    t: &Tables,
    f: &PlannedFile,
    release: &str,
    status: &str,
    duration: Option<Duration>,
    run_id: Option<i64>,
    error: Option<&str>,
    checksum: &str,
) -> Result<(), TrackedError> {
    let ms = duration.map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
    client
        .execute(
            &*format!(
                "INSERT INTO {} (file, kind, checksum, release, status, applied_at, duration_ms, run_id, error)
                 VALUES ($1, $2, $3, $4, $5, now(), $6, $7, $8)
                 ON CONFLICT (file) DO UPDATE SET kind = excluded.kind, checksum = excluded.checksum,
                   release = excluded.release, status = excluded.status, applied_at = excluded.applied_at,
                   duration_ms = excluded.duration_ms, run_id = excluded.run_id, error = excluded.error",
                t.files
            ),
            &[
                &f.id,
                &f.kind.as_str(),
                &checksum,
                &release,
                &status,
                &ms,
                &run_id,
                &error,
            ],
        )
        .map(|_| ())
        .map_err(dbe)
}

fn baseline_rows(
    client: &Client,
    baseline: &Baseline,
    files: &[PlannedFile],
) -> Result<HashMap<String, Option<String>>, TrackedError> {
    match baseline {
        Baseline::None => Ok(HashMap::new()),
        Baseline::All => Ok(files.iter().map(|f| (f.id.clone(), None)).collect()),
        Baseline::Query(q) => {
            let rows = client
                .query(q.as_str(), &[])
                .map_err(|e| TrackedError::Database(format!("baseline query: {}", db_error(&e))))?;
            rows.iter()
                .map(|r| {
                    let file: String = r.try_get(0).map_err(|e| {
                        TrackedError::Invalid(format!(
                            "baseline query: first column must be the file (text): {e}"
                        ))
                    })?;
                    let sum: Option<String> = if r.len() > 1 {
                        r.try_get(1).map_err(|e| {
                            TrackedError::Invalid(format!(
                                "baseline query: second column must be the checksum (text): {e}"
                            ))
                        })?
                    } else {
                        None
                    };
                    Ok((file, sum))
                })
                .collect()
        }
    }
}

fn apply_file(
    client: &Client,
    t: &Tables,
    f: &PlannedFile,
    sql: &str,
    opts: &Options,
    run_id: i64,
) -> Result<Duration, TrackedError> {
    let statements =
        split_statements(sql).map_err(|e| TrackedError::Invalid(format!("{}: {e}", f.id)))?;
    let start = Instant::now();
    if let Some(p) = &opts.preamble {
        client
            .batch_execute(p)
            .map_err(|e| TrackedError::Statement {
                file: f.id.clone(),
                index: 0,
                line: 0,
                statement: "(preamble)".to_string(),
                error: db_error(&e),
            })?;
    }
    if f.single_transaction {
        client.batch_execute("BEGIN").map_err(dbe)?;
    }
    for (n, (line, stmt)) in statements.iter().enumerate() {
        if let Err(e) = client.batch_execute(stmt) {
            if f.single_transaction {
                let _ = client.batch_execute("ROLLBACK");
            }
            return Err(TrackedError::Statement {
                file: f.id.clone(),
                index: n + 1,
                line: *line,
                statement: statement_head(stmt),
                error: db_error(&e),
            });
        }
    }
    let took = start.elapsed();
    if f.single_transaction {
        let rec = record(
            client,
            t,
            f,
            &opts.release,
            "applied",
            Some(took),
            Some(run_id),
            None,
            &f.checksum,
        )
        .and_then(|()| client.batch_execute("COMMIT").map_err(dbe));
        if let Err(e) = rec {
            let _ = client.batch_execute("ROLLBACK");
            return Err(e);
        }
    } else {
        record(
            client,
            t,
            f,
            &opts.release,
            "applied",
            Some(took),
            Some(run_id),
            None,
            &f.checksum,
        )?;
    }
    Ok(took)
}

fn finish_run(
    client: &Client,
    t: &Tables,
    run_id: i64,
    started: Instant,
    report: &Report,
    failure: Option<(&str, &str)>,
) -> Result<(), TrackedError> {
    let n = |a: Action| i32::try_from(report.count(a)).unwrap_or(i32::MAX);
    let ms = i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX);
    let status = if failure.is_some() {
        "failed"
    } else {
        "succeeded"
    };
    let (failed_file, error) = match failure {
        Some((f, e)) => (Some(f), Some(e)),
        None => (None, None),
    };
    let files = i32::try_from(report.files.len()).unwrap_or(i32::MAX);
    client
        .execute(
            &*format!(
                "UPDATE {} SET status = $2, finished_at = now(), duration_ms = $3, files = $4, applied = $5,
                   reapplied = $6, baselined = $7, skipped = $8, failed_file = $9, error = $10
                 WHERE run_id = $1",
                t.runs
            ),
            &[
                &run_id,
                &status,
                &ms,
                &files,
                &n(Action::Apply),
                &n(Action::Reapply),
                &n(Action::Baseline),
                &n(Action::Skip),
                &failed_file,
                &error,
            ],
        )
        .map(|_| ())
        .map_err(dbe)
}

/// Apply a plan. See the module documentation for what a run does.
///
/// Progress goes to `log` (one line per file applied, baselined or skipped). On error the run row
/// is marked failed with the file and error, a failing file is recorded with status `failed` (so
/// the next run applies it again), and files applied before it stay recorded.
///
/// # Errors
///
/// [`TrackedError::ChecksumMismatch`] when applied versioned files were edited (nothing is
/// applied), [`TrackedError::Statement`] when a file fails, and lock, filter and database errors.
pub fn run(
    client: &Client,
    files: &[PlannedFile],
    opts: &Options,
    filter: Option<Filter<'_>>,
    log: &mut dyn FnMut(&str),
) -> Result<Report, TrackedError> {
    let t = Tables::new(&opts.table)?;
    if opts.release.trim().is_empty() {
        return Err(TrackedError::Invalid(
            "release must not be empty".to_string(),
        ));
    }

    if opts.dry_run {
        let tracked = if t.exists(client)? {
            load_tracked(client, &t)?
        } else {
            Tracked::new()
        };
        let base = if tracked.is_empty() {
            baseline_rows(client, &opts.baseline, files)?
        } else {
            HashMap::new()
        };
        let mut view = tracked;
        for f in files {
            if let Some(sum) = base.get(&f.id) {
                view.insert(
                    f.id.clone(),
                    (
                        String::new(),
                        sum.clone().unwrap_or_else(|| f.checksum.clone()),
                        "baselined".into(),
                    ),
                );
            }
        }
        let (actions, changed) = decide(files, &view);
        if !changed.is_empty() {
            return Err(TrackedError::ChecksumMismatch {
                table: t.files,
                files: changed,
            });
        }
        let mut report = Report::default();
        for (f, a) in files.iter().zip(actions) {
            let a = if base.contains_key(&f.id) && a == Action::Skip {
                Action::Baseline
            } else {
                a
            };
            log(&*format!("{:<9} {}", label(a, true), f.id));
            report.files.push(FileOutcome {
                id: f.id.clone(),
                action: a,
                duration: None,
            });
        }
        return Ok(report);
    }

    let _lock = Lock::acquire(client, &t.files, opts.lock_timeout)?;
    t.create(client)?;
    let mut tracked = load_tracked(client, &t)?;

    let started = Instant::now();
    let run_id: i64 = client
        .query_one(
            &*format!(
                "INSERT INTO {} (release, status) VALUES ($1, 'running') RETURNING run_id",
                t.runs
            ),
            &[&opts.release],
        )
        .map_err(dbe)?
        .get(0);

    let mut report = Report {
        run_id: Some(run_id),
        ..Report::default()
    };
    let result = run_inner(
        client,
        &t,
        files,
        opts,
        filter,
        log,
        run_id,
        &mut tracked,
        &mut report,
    );
    let failure = match &result {
        Ok(()) => None,
        Err(e) => Some(e.to_string()),
    };
    let failed_file = match &result {
        Err(TrackedError::Statement { file, .. } | TrackedError::Filter { file, .. }) => {
            file.clone()
        }
        Err(TrackedError::ChecksumMismatch { files, .. }) => files
            .iter()
            .map(|c| c.id.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    };
    let fin = finish_run(
        client,
        &t,
        run_id,
        started,
        &report,
        failure.as_deref().map(|e| (failed_file.as_str(), e)),
    );
    result?;
    fin?;
    Ok(report)
}

/// The run's first line: `<label>: <pending> pending of <files> file(s) (...)`.
#[allow(clippy::too_many_arguments)]
pub fn start_line(
    label: Option<&str>,
    pending: usize,
    total: usize,
    apply: usize,
    reapply: usize,
    unchanged: usize,
    baselined: usize,
) -> String {
    format!(
        "{}{pending} pending of {total} file(s) ({apply} new, {reapply} changed repeatable, {unchanged} unchanged, {baselined} baselined)",
        label.map(|l| format!("{l}: ")).unwrap_or_default()
    )
}

fn label(a: Action, dry: bool) -> &'static str {
    match (a, dry) {
        (Action::Apply, false) => "applied",
        (Action::Apply, true) => "apply",
        (Action::Reapply, false) => "reapplied",
        (Action::Reapply, true) => "reapply",
        (Action::Skip, _) => "skip",
        (Action::Baseline, false) => "baselined",
        (Action::Baseline, true) => "baseline",
    }
}

#[allow(clippy::too_many_arguments)]
fn run_inner(
    client: &Client,
    t: &Tables,
    files: &[PlannedFile],
    opts: &Options,
    filter: Option<Filter<'_>>,
    log: &mut dyn FnMut(&str),
    run_id: i64,
    tracked: &mut Tracked,
    report: &mut Report,
) -> Result<(), TrackedError> {
    let mut baselined: Vec<&str> = Vec::new();
    if tracked.is_empty() {
        let base = baseline_rows(client, &opts.baseline, files)?;
        for f in files {
            if let Some(sum) = base.get(&f.id) {
                let sum = sum.clone().unwrap_or_else(|| f.checksum.clone());
                record(
                    client,
                    t,
                    f,
                    &opts.release,
                    "baselined",
                    None,
                    Some(run_id),
                    None,
                    &sum,
                )?;
                tracked.insert(
                    f.id.clone(),
                    (f.kind.as_str().into(), sum, "baselined".into()),
                );
                baselined.push(&f.id);
            }
        }
        let unknown = base
            .keys()
            .filter(|k| !files.iter().any(|f| &f.id == *k))
            .count();
        if !base.is_empty() {
            log(&*format!(
                "baseline: {} file(s) recorded as already applied{}",
                baselined.len(),
                if unknown > 0 {
                    format!(" ({unknown} baseline entr(ies) not in the plan, ignored)")
                } else {
                    String::new()
                }
            ));
        }
    }

    report.not_in_plan = tracked
        .keys()
        .filter(|k| !files.iter().any(|f| &f.id == *k))
        .cloned()
        .collect();
    report.not_in_plan.sort();

    let (actions, changed) = decide(files, tracked);
    if !changed.is_empty() {
        return Err(TrackedError::ChecksumMismatch {
            table: t.files.clone(),
            files: changed,
        });
    }
    let n = |want: Action| actions.iter().filter(|a| **a == want).count();
    log(&*start_line(
        opts.label.as_deref(),
        n(Action::Apply) + n(Action::Reapply),
        files.len(),
        n(Action::Apply),
        n(Action::Reapply),
        n(Action::Skip).saturating_sub(baselined.len()),
        baselined.len(),
    ));

    for (f, action) in files.iter().zip(actions) {
        let action = if action == Action::Skip && baselined.contains(&f.id.as_str()) {
            Action::Baseline
        } else {
            action
        };
        let mut outcome = FileOutcome {
            id: f.id.clone(),
            action,
            duration: None,
        };
        if matches!(action, Action::Apply | Action::Reapply) {
            let sql = match filter {
                Some(flt) => match flt(f) {
                    Ok(s) => s,
                    Err(error) => {
                        let _ = record(
                            client,
                            t,
                            f,
                            &opts.release,
                            "failed",
                            None,
                            Some(run_id),
                            Some(&error),
                            &f.checksum,
                        );
                        return Err(TrackedError::Filter {
                            file: f.id.clone(),
                            error,
                        });
                    }
                },
                None => f.sql.clone(),
            };
            log(&*format!("{:<9} {}", "start", f.id));
            let began = Instant::now();
            match apply_file(client, t, f, &sql, opts, run_id) {
                Ok(took) => {
                    outcome.duration = Some(took);
                    log(&*format!(
                        "{:<9} {} ({} ms)",
                        label(action, false),
                        f.id,
                        took.as_millis()
                    ));
                }
                Err(e) => {
                    log(&*format!(
                        "{:<9} {} after {} ms",
                        "failed",
                        f.id,
                        began.elapsed().as_millis()
                    ));
                    let msg = e.to_string();
                    let _ = record(
                        client,
                        t,
                        f,
                        &opts.release,
                        "failed",
                        None,
                        Some(run_id),
                        Some(&msg),
                        &f.checksum,
                    );
                    report.files.push(outcome);
                    return Err(e);
                }
            }
        }
        report.files.push(outcome);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stmts(sql: &str) -> Vec<String> {
        split_statements(sql)
            .unwrap()
            .into_iter()
            .map(|(_, s)| s)
            .collect()
    }

    #[test]
    fn splits_on_semicolons_outside_quotes_and_comments() {
        let sql = "CREATE TABLE a (x text DEFAULT ';');\n-- a comment; with a semicolon\nINSERT INTO a VALUES ('it''s;'); /* block; /* nested; */ still */ SELECT \"we;ird\" FROM a";
        assert_eq!(
            stmts(sql),
            vec![
                "CREATE TABLE a (x text DEFAULT ';')",
                "-- a comment; with a semicolon\nINSERT INTO a VALUES ('it''s;')",
                "/* block; /* nested; */ still */ SELECT \"we;ird\" FROM a",
            ]
        );
    }

    #[test]
    fn dollar_quotes_and_parameters() {
        let sql = "DO $$ BEGIN PERFORM 1; END $$;\nCREATE FUNCTION f() RETURNS text AS $fn$ SELECT 'a;b' $x$ ; $fn$ LANGUAGE sql;\nPREPARE p AS SELECT $1::int;";
        let s = stmts(sql);
        assert_eq!(s.len(), 3, "{s:?}");
        assert!(s[0].starts_with("DO $$") && s[0].ends_with("END $$"));
        assert!(s[1].contains("$x$ ; $fn$"));
        assert_eq!(s[2], "PREPARE p AS SELECT $1::int");
    }

    #[test]
    fn escape_strings_and_standard_strings() {
        let s = stmts("SELECT E'a\\'; b'; SELECT 'c\\'; SELECT 1");
        assert_eq!(s, vec!["SELECT E'a\\'; b'", "SELECT 'c\\'", "SELECT 1"]);
    }

    #[test]
    fn begin_atomic_bodies_stay_whole() {
        let sql = "CREATE FUNCTION g(i int) RETURNS int LANGUAGE sql BEGIN ATOMIC SELECT CASE WHEN i > 0 THEN 1 ELSE 0 END; SELECT 2; END;\nSELECT 3;";
        let s = stmts(sql);
        assert_eq!(s.len(), 2, "{s:?}");
        assert!(s[0].ends_with("SELECT 2; END"));
        assert_eq!(s[1], "SELECT 3");
    }

    #[test]
    fn comment_only_and_trailing_text() {
        assert!(stmts("-- nothing\n/* here */\n").is_empty());
        assert!(stmts(";\n-- c\n;").is_empty());
        assert_eq!(stmts("SELECT 1;\n-- tail"), vec!["SELECT 1"]);
        assert_eq!(stmts("SELECT 1"), vec!["SELECT 1"]);
    }

    #[test]
    fn start_lines() {
        let s = split_statements("-- header\n\nSELECT 1;\n\nSELECT\n 2;").unwrap();
        assert_eq!(s.iter().map(|(l, _)| *l).collect::<Vec<_>>(), vec![3, 5]);
    }

    #[test]
    fn rejects_unterminated_and_meta_commands() {
        assert!(split_statements("SELECT 'x").is_err());
        assert!(split_statements("SELECT $$x").is_err());
        assert!(split_statements("/* x").is_err());
        assert!(split_statements("\\set x 1\nSELECT 1;").is_err());
    }

    #[test]
    fn plan_parsing() {
        let p =
            parse_plan("# header\n\na.sql\nrepeatable  views/v.sql\nversioned b.sql\n").unwrap();
        assert_eq!(
            p,
            vec![
                ("a.sql".to_string(), None),
                ("views/v.sql".to_string(), Some(FileKind::Repeatable)),
                ("b.sql".to_string(), Some(FileKind::Versioned)),
            ]
        );
        assert!(parse_plan("a.sql\na.sql").is_err());
    }

    #[test]
    fn directives_in_leading_comments_only() {
        assert_eq!(
            directives("-- lifeguard:repeatable\nCREATE VIEW v AS SELECT 1;"),
            (true, false)
        );
        assert_eq!(
            directives("\n-- Lifeguard: single-transaction\n-- other\nSELECT 1"),
            (false, true)
        );
        assert_eq!(
            directives("SELECT 1;\n-- lifeguard:repeatable"),
            (false, false)
        );
    }

    #[test]
    fn redacted_target_never_shows_credentials() {
        let cases = [
            (
                "postgresql://pw_migrator:s3cr3t%40x@10.1.2.3:5432/pw_pp?sslmode=require",
                "10.1.2.3:5432/pw_pp",
            ),
            ("postgres://u:pa:ss@db.internal/app", "db.internal/app"),
            ("postgresql://u:pa/s?s@h:5432/d", "h:5432/d"),
            ("postgresql://u@[::1]:6432/x", "[::1]:6432/x"),
            (
                "postgresql:///app?host=/var/run/postgresql&port=5433",
                "/var/run/postgresql:5433/app",
            ),
            (
                "host=pwdb port=5432 user=pw password=s3cr3t dbname=pw_pp",
                "pwdb:5432/pw_pp",
            ),
            (
                "postgresql://u:s3cr3t@h1:5432,h2:5432/d",
                "h1:5432,h2:5432/d",
            ),
        ];
        for (url, want) in cases {
            let got = redacted_target(url);
            assert_eq!(got, want, "{url}");
            assert!(!got.contains("s3cr3t") && !got.contains("pa"), "{got}");
        }
    }

    #[test]
    fn statement_head_is_one_line_and_cut_at_200() {
        assert_eq!(statement_head("SELECT\n  1,\n\t2"), "SELECT 1, 2");
        let long = format!("SELECT '{}'", "x".repeat(300));
        let h = statement_head(&long);
        assert_eq!(h.chars().count(), 201);
        assert!(h.ends_with('…'));
    }

    #[test]
    fn statement_error_is_one_line() {
        let e = TrackedError::Statement {
            file: "migrations/001.sql".into(),
            index: 3,
            line: 12,
            statement: "ALTER TABLE t ADD COLUMN c int".into(),
            error: "42P01: relation \"t\" does not exist (position: 13)".into(),
        };
        let s = e.to_string();
        assert!(!s.contains('\n'), "{s}");
        assert!(
            s.starts_with("migrations/001.sql: statement 3 (line 12) failed: 42P01"),
            "{s}"
        );
        assert!(
            s.ends_with("; statement: ALTER TABLE t ADD COLUMN c int"),
            "{s}"
        );
    }

    #[test]
    fn start_line_counts() {
        assert_eq!(
            start_line(Some("lifeguard-migrate 0.1.0 db=h:5432/d"), 2, 135, 1, 1, 133, 0),
            "lifeguard-migrate 0.1.0 db=h:5432/d: 2 pending of 135 file(s) (1 new, 1 changed repeatable, 133 unchanged, 0 baselined)"
        );
        assert!(start_line(None, 0, 0, 0, 0, 0, 0).starts_with("0 pending"));
    }

    #[test]
    fn checksum_is_sha256_hex() {
        assert_eq!(
            checksum(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn table_names() {
        assert!(Tables::new("schema_migrations").is_ok());
        assert!(Tables::new("app.schema_migrations").is_ok());
        assert!(Tables::new("a.b.c").is_err());
        assert!(Tables::new("x; DROP TABLE y").is_err());
        assert!(Tables::new("").is_err());
        assert_eq!(Tables::new("app.m").unwrap().runs, "app.m_runs");
    }

    fn pf(id: &str, kind: FileKind, sum: &str) -> PlannedFile {
        PlannedFile {
            id: id.into(),
            path: PathBuf::from(id),
            kind,
            single_transaction: false,
            checksum: sum.into(),
            sql: String::new(),
        }
    }

    #[test]
    fn decisions() {
        let files = vec![
            pf("new.sql", FileKind::Versioned, "a"),
            pf("same.sql", FileKind::Versioned, "b"),
            pf("edited.sql", FileKind::Versioned, "c2"),
            pf("view.sql", FileKind::Repeatable, "d2"),
            pf("view_same.sql", FileKind::Repeatable, "e"),
            pf("failed.sql", FileKind::Versioned, "f2"),
        ];
        let mut t = Tracked::new();
        let rec = |k: &str, s: &str, st: &str| (k.to_string(), s.to_string(), st.to_string());
        t.insert("same.sql".into(), rec("versioned", "b", "applied"));
        t.insert("edited.sql".into(), rec("versioned", "c1", "applied"));
        t.insert("view.sql".into(), rec("repeatable", "d1", "applied"));
        t.insert("view_same.sql".into(), rec("repeatable", "e", "baselined"));
        t.insert("failed.sql".into(), rec("versioned", "f1", "failed"));
        let (a, changed) = decide(&files, &t);
        assert_eq!(
            a,
            vec![
                Action::Apply,
                Action::Skip,
                Action::Skip,
                Action::Reapply,
                Action::Skip,
                Action::Apply
            ]
        );
        assert_eq!(
            changed,
            vec![ChangedFile {
                id: "edited.sql".into(),
                stored: "c1".into(),
                current: "c2".into()
            }]
        );
    }
}
