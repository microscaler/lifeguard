//! Per-table migration file emission for the `migrations/<service>/<timestamp>_<table>.sql`
//! layout.
//!
//! Consumers that organize migrations as **one file per table per change** (e.g. the Hauliage
//! microservices monorepo) used to reimplement column / index / identity-tracking inline. This
//! module centralizes that logic on top of [`crate::generated_migration_diff`] so every consumer
//! gets:
//!
//! - **Stable identity via merged baselines.** All prior `*_<table>.sql` files for the same
//!   logical object are merged in filename order; the merged effective schema is compared to the
//!   fresh codegen output after whitespace / trailing-semicolon / comment normalization
//!   ([`normalize_table_sql_blob`](crate::generated_migration_diff::normalize_table_sql_blob)).
//!   If they match, no new file is written — this is the fast path even after many historical
//!   migrations exist for the same table, which the previous in-consumer implementation did not
//!   support.
//! - **Additive table deltas.** For tables, the existing diff engine emits
//!   `ALTER TABLE … ADD COLUMN IF NOT EXISTS` and `CREATE … INDEX IF NOT EXISTS` lines for any
//!   new columns / indexes.
//! - **Idempotent view re-writes.** For views (`CREATE OR REPLACE VIEW`), the fresh body is
//!   written whole — `OR REPLACE` is safe to re-run against an existing view, so we never need to
//!   emit a "diff" of a view.
//! - **Single run-timestamp.** Callers pass one timestamp string for the whole generation pass,
//!   so every file written during one run sorts consistently together rather than drifting by
//!   sub-seconds across tables.
//!
//! For the one-big-file-per-run layout (`*_generated_from_entities.sql`), use
//! [`crate::generated_migration_diff`] directly.

use crate::generated_migration_diff::{
    accumulate_table_baselines_from_dir, build_service_migration_body_from_accumulated,
    combined_old_section, normalize_table_sql_blob, service_migration_is_empty, TableBaselineParts,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// macOS AppleDouble resource forks (`._foo.sql`) are binary and must never be read as UTF-8 SQL.
#[must_use]
pub fn is_macos_resource_fork_file_name(name: &str) -> bool {
    name.starts_with("._")
}

/// Outcome of a single `write_per_table_migration_file` invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmissionOutcome {
    /// No prior file for this logical object existed; a full CREATE was written.
    Initial { path: PathBuf },
    /// At least one prior file existed; a diff (or a replacement view body) was written.
    Delta { path: PathBuf },
    /// The fresh codegen output matches the merged effective schema — nothing to do.
    Skipped,
}

impl EmissionOutcome {
    /// Convenience: path written, if any.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Initial { path } | Self::Delta { path } => Some(path.as_path()),
            Self::Skipped => None,
        }
    }
}

/// Optional `-- Migration: <name>\n-- Generated: <ts>\n\n` file header (matches the existing
/// Hauliage-style format). Keep `None` to emit SQL only.
#[derive(Debug, Clone, Copy)]
pub struct MigrationHeader<'a> {
    pub migration_name: &'a str,
    pub generated_timestamp: &'a str,
}

/// Write a per-table migration file for `table_name` under `service_dir`.
///
/// `fresh_sql` is the codegen output (a full `CREATE TABLE` or `CREATE [OR REPLACE] VIEW`
/// statement). `timestamp` is the filename prefix (`YYYYMMDDHHMMSS` by convention) and should be
/// shared across all tables emitted in the same generation pass.
///
/// See the module-level docs for the full identity / diff semantics.
pub fn write_per_table_migration_file(
    service_dir: &Path,
    table_name: &str,
    fresh_sql: &str,
    timestamp: &str,
    header: Option<MigrationHeader<'_>>,
) -> std::io::Result<EmissionOutcome> {
    fs::create_dir_all(service_dir)?;

    let baseline = accumulate_table_parts_from_dir(service_dir, table_name)?;
    let header_text = header
        .map(|h| {
            format!(
                "-- Migration: {}\n-- Generated: {}\n\n",
                h.migration_name, h.generated_timestamp
            )
        })
        .unwrap_or_default();
    let fresh_trimmed = fresh_sql.trim();

    let Some(parts) = baseline else {
        let path = service_dir.join(format!("{timestamp}_{table_name}.sql"));
        let content = format!("{header_text}{fresh_trimmed}\n");
        fs::write(&path, content)?;
        return Ok(EmissionOutcome::Initial { path });
    };
    let combined = combined_old_section(&parts);

    // Fast-skip: merged effective schema (including all prior ALTER/ADD/CREATE deltas) already
    // matches the fresh codegen output.
    if normalize_table_sql_blob(&combined) == normalize_table_sql_blob(fresh_trimmed) {
        return Ok(EmissionOutcome::Skipped);
    }

    // Views: `CREATE OR REPLACE VIEW` replaces the definition atomically, so on any change we
    // write the whole fresh body rather than trying to diff SELECT clauses.
    if looks_like_view(fresh_trimmed) {
        let path = service_dir.join(format!("{timestamp}_{table_name}.sql"));
        let content = format!("{header_text}{fresh_trimmed}\n");
        fs::write(&path, content)?;
        return Ok(EmissionOutcome::Delta { path });
    }

    // Tables: hand off to the diff engine, then strip its `-- Table: <name>\n` section prefix
    // (the per-file layout embeds the table name in the filename instead).
    let mut acc = BTreeMap::new();
    acc.insert(table_name.to_string(), parts);
    let body = build_service_migration_body_from_accumulated(
        &acc,
        &[(table_name.to_string(), fresh_trimmed.to_string())],
    );
    if service_migration_is_empty(&body) {
        return Ok(EmissionOutcome::Skipped);
    }
    let diff_sql = strip_table_section_header(&body, table_name)
        .trim()
        .to_string();
    if diff_sql.is_empty() {
        return Ok(EmissionOutcome::Skipped);
    }

    let path = service_dir.join(format!("{timestamp}_{table_name}.sql"));
    let content = format!("{header_text}{diff_sql}\n");
    fs::write(&path, content)?;
    Ok(EmissionOutcome::Delta { path })
}

/// Public helper (used by tests and consumers that want baseline introspection without writing).
///
/// Also exposed so callers can build the baseline once and reuse it across several operations.
pub fn accumulate_per_table_parts_from_files(
    files: &[PathBuf],
) -> std::io::Result<TableBaselineParts> {
    let mut parts = TableBaselineParts::default();
    for path in files {
        let content = fs::read_to_string(path)?;
        let payload = strip_migration_file_header(&content).trim().to_string();
        if payload.is_empty() {
            continue;
        }
        if payload_is_full_object(&payload) {
            parts.last_create_section = Some(payload);
            parts.delta_section_fragments.clear();
        } else {
            parts.delta_section_fragments.push(payload);
        }
    }
    Ok(parts)
}

/// Parse a migration filename under the strict `YYYYMMDDHHMMSS_<table>.sql` grammar.
///
/// Returns `Some((timestamp, table_slug))` only when **all** of the following hold:
///
/// - The filename ends in `.sql`.
/// - The leading 14 characters are ASCII digits (a full `YYYYMMDDHHMMSS` stamp).
/// - Character 15 is exactly `_`.
/// - There is at least one character of table slug between the underscore and `.sql`.
///
/// This rejects files the old heuristic would have mis-grouped: `manual_backup.sql`,
/// `20260417054933.sql`, `20260417054933_.sql`, `2026041705493_short.sql`, and any non-`.sql`
/// file. The `_generated_from_entities.sql` big-file layout is also excluded here (those belong
/// to [`crate::generated_migration_diff::accumulate_table_baselines_from_dir`]).
#[must_use]
pub fn parse_per_table_migration_filename(name: &str) -> Option<(u64, String)> {
    const TS_LEN: usize = 14;
    let stem = name.strip_suffix(".sql")?;
    // Exclude the big-file layout explicitly so callers can delegate to that accumulator.
    if stem.ends_with("_generated_from_entities") {
        return None;
    }
    if stem.len() < TS_LEN + 2 {
        // Need at least 14 digits + `_` + ≥ 1 char for the slug.
        return None;
    }
    let (ts_str, rest) = stem.split_at(TS_LEN);
    if !ts_str.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !rest.starts_with('_') {
        return None;
    }
    let slug = &rest[1..];
    if slug.is_empty() {
        return None;
    }
    let ts = ts_str.parse::<u64>().ok()?;
    Some((ts, slug.to_string()))
}

/// Convenience: baseline across every `YYYYMMDDHHMMSS_<table>.sql` under `service_dir`
/// (timestamp-ordered), in the form used by [`crate::generated_migration_diff`] APIs.
///
/// Files that do not match the strict per-table grammar are silently skipped — they never
/// contribute to any table's baseline. The big-file layout (`*_generated_from_entities.sql`) is
/// still merged in via [`crate::generated_migration_diff::accumulate_table_baselines_from_dir`]
/// so mixed directories are safe.
pub fn accumulate_per_table_baselines_from_dir(
    service_dir: &Path,
) -> std::io::Result<BTreeMap<String, TableBaselineParts>> {
    let big_file_map = accumulate_table_baselines_from_dir(service_dir);

    let mut per_table: BTreeMap<String, TableBaselineParts> = BTreeMap::new();
    if service_dir.is_dir() {
        // Every table a migration file names: canonical `<ts>_<table>.sql` slugs plus the targets
        // of CREATE TABLE / ALTER TABLE / CREATE INDEX statements in any file.
        let mut tables: BTreeSet<String> = BTreeSet::new();
        for path in migration_files_in_dir(service_dir)? {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if let Some((_, table)) = parse_per_table_migration_filename(name) {
                tables.insert(table);
            }
            let content = fs::read_to_string(&path)?;
            for stmt in statements(strip_migration_file_header(&content)) {
                if let Some(t) = baseline_statement_table(&stmt) {
                    tables.insert(t);
                }
            }
        }
        for table in tables {
            if let Some(parts) = accumulate_table_parts_from_dir(service_dir, &table)? {
                per_table.insert(table, parts);
            }
        }
    }

    // Merge: per-table entries override big-file entries with the same key (shouldn't collide in
    // practice, but keep it deterministic).
    let mut out = big_file_map;
    for (k, v) in per_table {
        out.insert(k, v);
    }
    Ok(out)
}

/// Forward migration files directly in `service_dir`, in filename (timestamp) order. Rollback
/// migrations (`*.down.sql`) and macOS resource forks are skipped; subdirectories such as
/// `rollback/` are not read.
fn migration_files_in_dir(service_dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = Vec::new();
    if !service_dir.is_dir() {
        return Ok(files);
    }
    for entry in fs::read_dir(service_dir)?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if is_macos_resource_fork_file_name(name)
            || !name.ends_with(".sql")
            || crate::sql_dependency_order::is_rollback_migration_path(name)
        {
            continue;
        }
        files.push(path);
    }
    files.sort();
    Ok(files)
}

/// SQL statements in `sql`, each ending in `;`. Unsplittable text (an unterminated quote) is
/// returned as a single statement.
fn statements(sql: &str) -> Vec<String> {
    match crate::tracked::split_statements(sql) {
        Ok(stmts) => stmts
            .into_iter()
            .map(|(_, stmt)| format!("{};", stmt.trim()))
            .collect(),
        Err(_) => vec![sql.trim().to_string()],
    }
}

/// The statement with leading `--` comment lines removed.
fn without_leading_comments(stmt: &str) -> &str {
    let mut rest = stmt.trim_start();
    while rest.starts_with("--") {
        rest = rest
            .split_once('\n')
            .map_or("", |(_, tail)| tail)
            .trim_start();
    }
    rest
}

/// Unqualified, unquoted, lower-cased table name from a possibly schema-qualified identifier
/// token (`pricewhisperer.pw_orders`, `"pw_orders"`, `pw_orders(`).
fn bare_table_name(token: &str) -> Option<String> {
    let ident = token.split('(').next()?.trim_end_matches(';');
    let last = ident.rsplit('.').next()?.trim_matches('"');
    (!last.is_empty()).then(|| last.to_ascii_lowercase())
}

/// The table a statement shapes, for baseline purposes: `CREATE TABLE`, `ALTER TABLE` and
/// `CREATE [UNIQUE] INDEX .. ON`. `None` for anything else.
fn baseline_statement_table(stmt: &str) -> Option<String> {
    let body = without_leading_comments(stmt);
    let words: Vec<&str> = body.split_whitespace().collect();
    let upper: Vec<String> = words.iter().map(|w| w.to_ascii_uppercase()).collect();
    let at = |i: usize| upper.get(i).map(String::as_str);
    let mut i;
    match (at(0), at(1)) {
        (Some("CREATE"), Some("TABLE")) => {
            i = 2;
            if at(i) == Some("IF") && at(i + 1) == Some("NOT") && at(i + 2) == Some("EXISTS") {
                i += 3;
            }
        }
        (Some("ALTER"), Some("TABLE")) => {
            i = 2;
            if at(i) == Some("IF") && at(i + 1) == Some("EXISTS") {
                i += 2;
            }
            if at(i) == Some("ONLY") {
                i += 1;
            }
        }
        (Some("CREATE"), Some("INDEX" | "UNIQUE")) => {
            i = upper.iter().position(|w| w == "ON")? + 1;
            if at(i) == Some("ONLY") {
                i += 1;
            }
        }
        _ => return None,
    }
    bare_table_name(words.get(i)?)
}

/// Split `text` at commas outside parentheses and single-quoted strings.
fn split_top_level_commas(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '\'' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth = depth.saturating_sub(1),
            ',' if !quoted && depth == 0 => {
                parts.push(std::mem::take(&mut current));
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    parts.push(current);
    parts
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// `ALTER TABLE t clause, clause;` (possibly over several lines) as one single-line
/// `ALTER TABLE t clause;` per clause, the shape the baseline column parser reads. Whitespace is
/// collapsed; anything that is not an `ALTER TABLE` is returned unchanged.
fn alter_clauses_one_per_line(stmt: &str) -> String {
    let collapsed = without_leading_comments(stmt)
        .trim()
        .trim_end_matches(';')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let words: Vec<&str> = collapsed.split(' ').collect();
    let upper: Vec<String> = words.iter().map(|w| w.to_ascii_uppercase()).collect();
    let at = |i: usize| upper.get(i).map(String::as_str);
    if at(0) != Some("ALTER") || at(1) != Some("TABLE") {
        return stmt.to_string();
    }
    let mut name = 2;
    if at(name) == Some("IF") && at(name + 1) == Some("EXISTS") {
        name += 2;
    }
    if at(name) == Some("ONLY") {
        name += 1;
    }
    if name + 1 >= words.len() {
        return stmt.to_string();
    }
    let header = words[..=name].join(" ");
    split_top_level_commas(&words[name + 1..].join(" "))
        .into_iter()
        .map(|clause| format!("{header} {clause};"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// `content` without its lifeguard `-- Migration:` header, but only when the text
/// [`strip_migration_file_header`] would drop is all comments and blank lines. A hand-written file
/// whose comment block runs straight into SQL has no blank line after its header, and stripping
/// up to the first blank line would drop its first statements.
fn without_generated_header(content: &str) -> &str {
    let stripped = strip_migration_file_header(content);
    let dropped = &content[..content.len() - stripped.len()];
    if dropped
        .lines()
        .all(|line| line.trim().is_empty() || line.trim_start().starts_with("--"))
    {
        stripped
    } else {
        content
    }
}

/// Merged baseline for `table_name` from every forward migration file in `service_dir`, in
/// filename (timestamp) order. A canonical `<ts>_<table_name>.sql` file contributes its whole
/// payload, as before. Any other file contributes its statements that create, alter or index
/// `table_name`, so tables created inside another table's file and columns added by hand-written
/// migrations with other names are part of the baseline. `None` when no file mentions the table.
///
/// # Errors
///
/// Returns an I/O error when the directory or a file cannot be read.
pub fn accumulate_table_parts_from_dir(
    service_dir: &Path,
    table_name: &str,
) -> std::io::Result<Option<TableBaselineParts>> {
    let mut parts = TableBaselineParts::default();
    let mut found = false;
    for path in migration_files_in_dir(service_dir)? {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let canonical =
            parse_per_table_migration_filename(name).is_some_and(|(_, slug)| slug == table_name);
        let content = fs::read_to_string(&path)?;
        let payload = without_generated_header(&content).trim();
        if canonical {
            if payload.is_empty() {
                continue;
            }
            found = true;
            // The whole payload, unless the file also creates, alters or indexes other tables
            // (a hand-written file creating two tables): then only this table's statements.
            let stmts = statements(payload);
            let foreign =
                |stmt: &String| baseline_statement_table(stmt).is_some_and(|t| t != table_name);
            let own = if stmts.iter().any(foreign) {
                stmts
                    .iter()
                    .filter(|stmt| !foreign(*stmt))
                    .map(|stmt| without_leading_comments(stmt))
                    .collect::<Vec<_>>()
                    .join(
                        "
",
                    )
            } else {
                payload.to_string()
            };
            if payload_is_full_object(&own) {
                parts.last_create_section = Some(own);
                parts.delta_section_fragments.clear();
            } else {
                parts.delta_section_fragments.push(own);
            }
            continue;
        }
        for stmt in statements(payload) {
            if baseline_statement_table(&stmt).as_deref() != Some(table_name) {
                continue;
            }
            found = true;
            let body = without_leading_comments(&stmt).to_string();
            let upper = body.to_ascii_uppercase();
            if upper.starts_with("CREATE TABLE") {
                parts.last_create_section = Some(body);
                parts.delta_section_fragments.clear();
            } else if upper.starts_with("ALTER TABLE") {
                parts
                    .delta_section_fragments
                    .push(alter_clauses_one_per_line(&body));
            } else {
                parts.delta_section_fragments.push(body);
            }
        }
    }
    Ok(found.then_some(parts))
}

/// Find all files in `service_dir` that belong to `table_name` under the per-table layout.
///
/// Matches any filename ending in `_{table_name}.sql`. Returned list is filename-sorted (which
/// sorts chronologically for `YYYYMMDDHHMMSS_<table>.sql`).
pub fn find_existing_per_table_files(
    service_dir: &Path,
    table_name: &str,
) -> std::io::Result<Vec<PathBuf>> {
    let suffix = format!("_{table_name}.sql");
    let mut files: Vec<PathBuf> = Vec::new();
    if !service_dir.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(service_dir)?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| !is_macos_resource_fork_file_name(n) && n.ends_with(&suffix))
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn looks_like_view(sql: &str) -> bool {
    let up = sql.trim_start().to_ascii_uppercase();
    up.starts_with("CREATE OR REPLACE VIEW ") || up.starts_with("CREATE VIEW ")
}

fn payload_is_full_object(payload: &str) -> bool {
    let up = payload.to_ascii_uppercase();
    up.contains("CREATE TABLE IF NOT EXISTS ")
        || up.contains("CREATE OR REPLACE VIEW ")
        || up.contains("CREATE VIEW ")
}

/// Strip the `-- Migration: <name>\n-- Generated: <ts>\n\n` header (if any) from a file payload.
///
/// Tolerant: if the file doesn't start with `--`, the whole content is returned.
pub fn strip_migration_file_header(content: &str) -> &str {
    let leading = content.trim_start_matches('\n');
    if !leading.starts_with("--") {
        return content;
    }
    if let Some(idx) = content.find("\n\n") {
        return content[idx + 2..].trim_start_matches('\n');
    }
    content
}

fn strip_table_section_header(body: &str, table_name: &str) -> String {
    let marker = format!("-- Table: {table_name}\n");
    body.strip_prefix(&marker)
        .map(str::to_string)
        .unwrap_or_else(|| body.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn baseline_includes_statements_from_every_forward_migration_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let d = dir.path();
        fs::write(
            d.join("20260101000000_a.sql"),
            "CREATE TABLE IF NOT EXISTS s.a (\n    id INTEGER PRIMARY KEY\n);\n\
             CREATE TABLE IF NOT EXISTS s.b (\n    id INTEGER PRIMARY KEY\n);\n",
        )
        .expect("write");
        fs::write(
            d.join("20260102000000_a_extra_columns.sql"),
            "ALTER TABLE s.a ADD COLUMN IF NOT EXISTS note TEXT;\n",
        )
        .expect("write");
        fs::create_dir_all(d.join("rollback")).expect("mkdir");
        fs::write(
            d.join("rollback/20260103000000_a.down.sql"),
            "ALTER TABLE s.a ADD COLUMN IF NOT EXISTS rolled_back TEXT;\n",
        )
        .expect("write");
        fs::write(
            d.join("20260104000000_a.down.sql"),
            "ALTER TABLE s.a ADD COLUMN IF NOT EXISTS also_rolled_back TEXT;\n",
        )
        .expect("write");

        let columns = |table: &str| -> Vec<String> {
            let parts = accumulate_table_parts_from_dir(d, table)
                .expect("read")
                .expect("baseline");
            crate::generated_migration_diff::column_map_from_merged_baseline(&parts)
                .into_keys()
                .collect()
        };
        // `note` comes from a hand-written file with another name; rollbacks never count.
        assert_eq!(columns("a"), vec!["id", "note"]);
        // A multi-line ALTER with several ADD COLUMN clauses counts every clause.
        fs::write(
            d.join("20260105000000_a_state.sql"),
            "ALTER TABLE s.a\n    ADD COLUMN IF NOT EXISTS state VARCHAR(12) NOT NULL DEFAULT 'x,y',\n    \
             ADD COLUMN IF NOT EXISTS amount NUMERIC(18, 6);\n",
        )
        .expect("write");
        assert_eq!(columns("a"), vec!["amount", "id", "note", "state"]);
        // `b` is created inside `a`'s file.
        assert_eq!(columns("b"), vec!["id"]);
        assert!(accumulate_table_parts_from_dir(d, "c")
            .expect("read")
            .is_none());
    }

    #[test]
    fn baseline_statement_targets() {
        for (stmt, table) in [
            (
                "CREATE TABLE IF NOT EXISTS pricewhisperer.pw_orders (id INT)",
                Some("pw_orders"),
            ),
            ("CREATE TABLE t(id INT)", Some("t")),
            (
                "-- note\nALTER TABLE IF EXISTS ONLY \"s\".\"T1\" ADD COLUMN x INT",
                Some("t1"),
            ),
            (
                "CREATE UNIQUE INDEX IF NOT EXISTS i ON s.t USING btree (a)",
                Some("t"),
            ),
            ("CREATE INDEX i ON ONLY t (a)", Some("t")),
            (
                "CREATE OR REPLACE FUNCTION f() RETURNS trigger AS $$ BEGIN END $$",
                None,
            ),
            ("INSERT INTO t VALUES (1)", None),
        ] {
            assert_eq!(baseline_statement_table(stmt).as_deref(), table, "{stmt}");
        }
    }

    fn write(path: &Path, body: &str) {
        fs::write(path, body).expect("write test fixture");
    }

    #[test]
    fn initial_emission_writes_full_create() {
        let dir = tempdir().unwrap();
        let out = write_per_table_migration_file(
            dir.path(),
            "widgets",
            "CREATE TABLE IF NOT EXISTS widgets (id UUID PRIMARY KEY);",
            "20260417120000",
            None,
        )
        .unwrap();
        let EmissionOutcome::Initial { path } = out else {
            panic!("expected Initial, got {out:?}");
        };
        assert!(path.ends_with("20260417120000_widgets.sql"));
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.contains("CREATE TABLE IF NOT EXISTS widgets"));
    }

    #[test]
    fn identical_second_run_is_skipped_even_after_prior_delta() {
        // Two prior files: the full CREATE, then a later ALTER. Fresh codegen matches the
        // merged baseline → no new file should be written (the key regression fix).
        let dir = tempdir().unwrap();
        write(
            &dir.path().join("20260101000000_widgets.sql"),
            "-- Migration: widgets\n-- Generated: 20260101000000\n\n\
             CREATE TABLE IF NOT EXISTS widgets (\n    id UUID PRIMARY KEY,\n    name VARCHAR(255) NOT NULL\n);\n",
        );
        write(
            &dir.path().join("20260102000000_widgets.sql"),
            "-- Migration: widgets\n-- Generated: 20260102000000\n\n\
             ALTER TABLE widgets ADD COLUMN IF NOT EXISTS sku VARCHAR(50) NOT NULL DEFAULT '';\n",
        );
        let fresh = "CREATE TABLE IF NOT EXISTS widgets (\n    id UUID PRIMARY KEY,\n    name VARCHAR(255) NOT NULL,\n    sku VARCHAR(50) NOT NULL DEFAULT ''\n);\n";
        let out =
            write_per_table_migration_file(dir.path(), "widgets", fresh, "20260417120000", None)
                .unwrap();
        assert_eq!(out, EmissionOutcome::Skipped, "got {out:?}");
        // No new file
        let files: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            files.iter().all(|n| !n.starts_with("20260417120000")),
            "unexpected new file: {files:?}"
        );
    }

    #[test]
    fn additive_column_emits_alter_delta_only() {
        let dir = tempdir().unwrap();
        write(
            &dir.path().join("20260101000000_widgets.sql"),
            "-- Migration: widgets\n-- Generated: 20260101000000\n\n\
             CREATE TABLE IF NOT EXISTS widgets (\n    id UUID PRIMARY KEY\n);\n",
        );
        let fresh = "CREATE TABLE IF NOT EXISTS widgets (\n    id UUID PRIMARY KEY,\n    sku VARCHAR(50) NOT NULL DEFAULT ''\n);\n";
        let out =
            write_per_table_migration_file(dir.path(), "widgets", fresh, "20260417120000", None)
                .unwrap();
        let EmissionOutcome::Delta { path } = out else {
            panic!("expected Delta, got {out:?}");
        };
        let body = fs::read_to_string(&path).unwrap();
        assert!(
            body.contains("ALTER TABLE widgets ADD COLUMN IF NOT EXISTS sku"),
            "body: {body}"
        );
        assert!(
            !body.contains("CREATE TABLE IF NOT EXISTS"),
            "should not re-emit CREATE TABLE: {body}"
        );
        // Table section header should not leak into the per-file format.
        assert!(!body.starts_with("-- Table:"), "body: {body}");
    }

    #[test]
    fn view_identical_second_run_is_skipped() {
        let dir = tempdir().unwrap();
        write(
            &dir.path().join("20260101000000_gics_view.sql"),
            "-- Migration: gics_view\n-- Generated: 20260101000000\n\n\
             CREATE OR REPLACE VIEW schema.gics_view AS\n\
             SELECT id FROM schema.gics_categories WHERE level = 3;\n",
        );
        let fresh = "CREATE OR REPLACE VIEW schema.gics_view AS\nSELECT id FROM schema.gics_categories WHERE level = 3;\n";
        let out =
            write_per_table_migration_file(dir.path(), "gics_view", fresh, "20260417120000", None)
                .unwrap();
        assert_eq!(out, EmissionOutcome::Skipped, "got {out:?}");
    }

    #[test]
    fn view_definition_change_rewrites_full_body() {
        let dir = tempdir().unwrap();
        write(
            &dir.path().join("20260101000000_gics_view.sql"),
            "-- Migration: gics_view\n-- Generated: 20260101000000\n\n\
             CREATE OR REPLACE VIEW schema.gics_view AS SELECT id FROM schema.gics_categories WHERE level = 3;\n",
        );
        let fresh = "CREATE OR REPLACE VIEW schema.gics_view AS\nSELECT id, name FROM schema.gics_categories WHERE level = 3;\n";
        let out =
            write_per_table_migration_file(dir.path(), "gics_view", fresh, "20260417120000", None)
                .unwrap();
        let EmissionOutcome::Delta { path } = out else {
            panic!("expected Delta, got {out:?}");
        };
        let body = fs::read_to_string(&path).unwrap();
        assert!(
            body.contains("CREATE OR REPLACE VIEW schema.gics_view"),
            "body: {body}"
        );
        assert!(body.contains("SELECT id, name"));
    }

    #[test]
    fn header_option_writes_migration_header() {
        let dir = tempdir().unwrap();
        let out = write_per_table_migration_file(
            dir.path(),
            "widgets",
            "CREATE TABLE IF NOT EXISTS widgets (id UUID PRIMARY KEY);",
            "20260417120000",
            Some(MigrationHeader {
                migration_name: "widgets",
                generated_timestamp: "20260417120000",
            }),
        )
        .unwrap();
        let EmissionOutcome::Initial { path } = out else {
            panic!("expected Initial, got {out:?}");
        };
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.starts_with("-- Migration: widgets\n"));
        assert!(body.contains("-- Generated: 20260417120000"));
    }

    #[test]
    fn find_existing_per_table_files_ignores_unrelated() {
        let dir = tempdir().unwrap();
        write(&dir.path().join("20260101000000_widgets.sql"), "x");
        write(&dir.path().join("20260101000000_orders.sql"), "y");
        write(&dir.path().join("apply_order.txt"), "z");
        let found = find_existing_per_table_files(dir.path(), "widgets").unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("_widgets.sql"));
    }

    #[test]
    fn baseline_keeps_statements_that_follow_a_comment_block_without_a_blank_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        let d = dir.path();
        fs::write(
            d.join("20260101000000_t.sql"),
            "-- Migration: t\n-- Generated: 20260101000000\n\nCREATE TABLE IF NOT EXISTS s.t (\n    id INT PRIMARY KEY\n);\n",
        )
        .unwrap();
        fs::write(
            d.join("20260102000000_t_more.sql"),
            "-- Migration: t_more\n-- Generated: 20260102000000\n-- Notes that run straight into SQL.\nALTER TABLE s.t\n    ADD COLUMN IF NOT EXISTS a TEXT NOT NULL DEFAULT 'x, y',\n    ADD COLUMN IF NOT EXISTS b INT;\nALTER TABLE s.u\n    ADD COLUMN IF NOT EXISTS c INT;\n\nCREATE INDEX IF NOT EXISTS t_a ON s.t (a);\n",
        )
        .unwrap();
        let parts = accumulate_table_parts_from_dir(d, "t").unwrap().unwrap();
        let combined = crate::generated_migration_diff::combined_old_section(&parts);
        assert!(
            combined.contains(
                "ALTER TABLE s.t ADD COLUMN IF NOT EXISTS a TEXT NOT NULL DEFAULT 'x, y';"
            ),
            "{combined}"
        );
        assert!(
            combined.contains("ALTER TABLE s.t ADD COLUMN IF NOT EXISTS b INT;"),
            "{combined}"
        );
        assert!(
            combined.contains("CREATE INDEX IF NOT EXISTS t_a ON s.t (a);"),
            "{combined}"
        );
        assert!(!combined.contains("s.u"), "{combined}");
    }

    #[test]
    fn generated_header_is_stripped_only_when_nothing_but_comments_precede_the_blank_line() {
        let generated =
            "-- Migration: x\n-- Generated: 20260101000000\n\nALTER TABLE x ADD COLUMN y INT;\n";
        assert_eq!(
            without_generated_header(generated),
            "ALTER TABLE x ADD COLUMN y INT;\n"
        );
        let hand_written = "-- Migration: x\n-- notes\nALTER TABLE x ADD COLUMN y INT;\n\nCREATE INDEX i ON x (y);\n";
        assert_eq!(without_generated_header(hand_written), hand_written);
    }

    #[test]
    fn strip_migration_file_header_removes_lifeguard_header() {
        let content =
            "-- Migration: x\n-- Generated: 20260101000000\n\nALTER TABLE x ADD COLUMN y INT;\n";
        assert_eq!(
            strip_migration_file_header(content),
            "ALTER TABLE x ADD COLUMN y INT;\n"
        );
    }

    #[test]
    fn strip_migration_file_header_passthrough_when_no_header() {
        let content = "CREATE TABLE IF NOT EXISTS widgets (id INT);\n";
        assert_eq!(strip_migration_file_header(content), content);
    }

    // --- Strict YYYYMMDDHHMMSS_<name>.sql filename grammar for directory accumulation ---

    #[test]
    fn parse_per_table_migration_filename_accepts_canonical() {
        let got = parse_per_table_migration_filename("20260417054933_widgets.sql");
        assert_eq!(got, Some((20_260_417_054_933, "widgets".to_string())));
    }

    #[test]
    fn parse_per_table_migration_filename_accepts_underscore_in_table_slug() {
        let got = parse_per_table_migration_filename("20260417054933_organization_profiles.sql");
        assert_eq!(
            got,
            Some((20_260_417_054_933, "organization_profiles".to_string()))
        );
    }

    #[test]
    fn parse_per_table_migration_filename_rejects_short_timestamp() {
        // 13 digits — not a full `YYYYMMDDHHMMSS`.
        assert!(parse_per_table_migration_filename("2026041705493_widgets.sql").is_none());
    }

    #[test]
    fn parse_per_table_migration_filename_rejects_long_timestamp() {
        // 15 digits — extra digit that would have been silently absorbed by the old heuristic.
        assert!(parse_per_table_migration_filename("202604170549333_widgets.sql").is_none());
    }

    #[test]
    fn parse_per_table_migration_filename_rejects_non_digit_timestamp() {
        assert!(parse_per_table_migration_filename("manual_backup_widgets.sql").is_none());
    }

    #[test]
    fn parse_per_table_migration_filename_rejects_missing_table_slug() {
        assert!(parse_per_table_migration_filename("20260417054933_.sql").is_none());
        assert!(parse_per_table_migration_filename("20260417054933.sql").is_none());
    }

    #[test]
    fn parse_per_table_migration_filename_rejects_non_sql_extension() {
        assert!(parse_per_table_migration_filename("20260417054933_widgets.txt").is_none());
    }

    #[test]
    fn accumulate_baselines_skips_non_canonical_filenames() {
        // Only `20260417054933_widgets.sql` should be considered — the other files must not
        // pollute the per-table baseline map.
        let dir = tempdir().unwrap();
        write(
            &dir.path().join("20260417054933_widgets.sql"),
            "-- Migration: widgets\n-- Generated: 20260417054933\n\n\
             CREATE TABLE IF NOT EXISTS widgets (id UUID PRIMARY KEY);\n",
        );
        // Missing `_<name>` part — would previously be mis-grouped into the empty-table bucket.
        write(&dir.path().join("20260417054933.sql"), "-- stray\n");
        // Underscore-separated but no timestamp prefix.
        write(&dir.path().join("manual_seed.sql"), "-- stray\n");
        // Timestamp with the wrong length.
        write(
            &dir.path().join("2026041705493_wrong_len.sql"),
            "-- stray\n",
        );
        // The big-file layout — should be picked up by the big-file accumulator, not here.
        write(
            &dir.path()
                .join("20260417054933_generated_from_entities.sql"),
            "-- Table: elsewhere\nCREATE TABLE IF NOT EXISTS elsewhere (id INT PRIMARY KEY);\n",
        );

        let map = accumulate_per_table_baselines_from_dir(dir.path()).unwrap();
        assert!(
            map.contains_key("widgets"),
            "widgets must be present: {:?}",
            map.keys().collect::<Vec<_>>()
        );
        assert!(
            !map.contains_key(""),
            "empty-string table name must not leak in: {:?}",
            map.keys().collect::<Vec<_>>()
        );
        assert!(
            !map.contains_key("wrong_len"),
            "non-canonical timestamp must be skipped: {:?}",
            map.keys().collect::<Vec<_>>()
        );
        assert!(
            !map.contains_key("seed"),
            "non-timestamped file must be skipped: {:?}",
            map.keys().collect::<Vec<_>>()
        );
        // But the big-file layout should still contribute via the big-file accumulator.
        assert!(
            map.contains_key("elsewhere"),
            "big-file layout must still be surfaced: {:?}",
            map.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn find_existing_per_table_files_ignores_macos_resource_forks() {
        let dir = tempdir().unwrap();
        write(
            &dir.path().join("20260417054933_notification_alerts.sql"),
            "CREATE TABLE IF NOT EXISTS notification_alerts (id UUID PRIMARY KEY);\n",
        );
        // Binary AppleDouble fork — must not be matched even though it ends with the suffix.
        fs::write(
            &dir.path().join("._20260417054933_notification_alerts.sql"),
            &[0x00, 0x05, 0x16, 0x07, 0xff],
        )
        .unwrap();

        let files = find_existing_per_table_files(dir.path(), "notification_alerts").unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0]
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| !n.starts_with("._")));
    }
}
