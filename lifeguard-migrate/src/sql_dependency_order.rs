//! Order SQL migrations using foreign-key edges parsed from SQL and Rust sources.

use crate::dependency_ordering::{
    extract_foreign_key_table, topological_sort, validate_foreign_key_references, TableInfo,
};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// `foreign_key = "other_table(col) ..."` attributes in Rust entity sources.
pub fn extract_foreign_key_targets_from_rust_source(content: &str) -> Vec<String> {
    let re = Regex::new(r#"#\[foreign_key\s*=\s*"([^"]+)""#).expect("regex");
    let mut out = Vec::new();
    for cap in re.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            out.push(extract_foreign_key_table(m.as_str()));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// `REFERENCES other_table (` in migration SQL (inline FKs).
pub fn extract_referenced_tables_from_migration_sql(sql: &str) -> Vec<String> {
    let re =
        Regex::new(r"(?i)REFERENCES\s+([a-zA-Z_][a-zA-Z0-9_]*(?:\.[a-zA-Z_][a-zA-Z0-9_]*)?)\s*\(")
            .expect("regex");
    let mut out = Vec::new();
    for cap in re.captures_iter(sql) {
        let raw = cap.get(1).unwrap().as_str();
        let table = if let Some(dot) = raw.rfind('.') {
            &raw[dot + 1..]
        } else {
            raw
        };
        out.push(table.to_string());
    }
    out.sort();
    out.dedup();
    out
}

fn normalize_sql_table_name(raw: &str) -> String {
    if let Some(dot) = raw.rfind('.') {
        raw[dot + 1..].to_string()
    } else {
        raw.to_string()
    }
}

/// Every `CREATE TABLE IF NOT EXISTS name` and `CREATE [OR REPLACE] VIEW [IF NOT EXISTS] name`
/// in a file (PostgreSQL), normalized, sorted, deduped.
///
/// Views are registered as first-class creatable objects so later migrations that read from them
/// (via `FROM` / `JOIN` parsed by [`extract_view_source_tables_from_migration_sql`]) can be
/// ordered after their creator in [`write_apply_order_file`].
pub fn extract_created_tables_from_migration_sql(sql: &str) -> Vec<String> {
    // Unquoted or "quoted" identifier, optionally prefixed by a schema segment.
    const QNAME: &str = r#"(?:(?:"[^"]+"|[a-zA-Z_][a-zA-Z0-9_]*)(?:\.(?:"[^"]+"|[a-zA-Z_][a-zA-Z0-9_]*))?)"#;

    let table_re = Regex::new(&format!(
        r"(?i)CREATE\s+TABLE\s+IF\s+NOT\s+EXISTS\s+({QNAME})\s*\(",
    ))
    .expect("CREATE TABLE regex");
    let view_re = Regex::new(&format!(
        r"(?i)CREATE\s+(?:OR\s+REPLACE\s+)?VIEW(?:\s+IF\s+NOT\s+EXISTS)?\s+({QNAME})\s+AS\b",
    ))
    .expect("CREATE VIEW regex");

    let mut out: Vec<String> = Vec::new();
    for cap in table_re.captures_iter(sql) {
        if let Some(m) = cap.get(1) {
            out.push(normalize_sql_table_name(m.as_str()));
        }
    }
    for cap in view_re.captures_iter(sql) {
        if let Some(m) = cap.get(1) {
            out.push(normalize_sql_table_name(m.as_str()));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Tables referenced from `FROM` / `JOIN` clauses inside `CREATE [OR REPLACE] VIEW … AS …`
/// statements, normalized (schema stripped), sorted, deduped.
///
/// Views are not FK objects: PostgreSQL never treats them as dependencies of later `REFERENCES`
/// clauses, but the SELECT body must be parseable at `CREATE VIEW` time — so any table cited via
/// `FROM <table>` or `<kind> JOIN <table>` must already exist. CTE names introduced by `WITH`
/// inside the same view are excluded so they are not mistaken for real tables.
pub fn extract_view_source_tables_from_migration_sql(sql: &str) -> Vec<String> {
    // Capture everything after `… AS` up to the next `;` (or end of input) — that is the SELECT
    // body whose `FROM` / `JOIN` clauses we care about.
    const QNAME: &str = r#"(?:(?:"[^"]+"|[a-zA-Z_][a-zA-Z0-9_]*)(?:\.(?:"[^"]+"|[a-zA-Z_][a-zA-Z0-9_]*))?)"#;
    let view_body_re = Regex::new(&format!(
        r"(?is)CREATE\s+(?:OR\s+REPLACE\s+)?VIEW(?:\s+IF\s+NOT\s+EXISTS)?\s+{QNAME}\s+AS\s+(.*?)(?:;|\z)",
    ))
    .expect("CREATE VIEW body regex");

    // `FROM <qname>` or `[INNER|LEFT|RIGHT|FULL] [OUTER] JOIN <qname>`. We do not require `\b`
    // before `FROM`/`JOIN` because `(?i)` plus word-prefix tokens in real SQL (commas, newlines,
    // parens) keep false positives rare; we do gate on whitespace after the keyword.
    let from_join_re = Regex::new(&format!(
        r"(?is)\b(?:FROM|JOIN)\s+(?:ONLY\s+|LATERAL\s+)?({QNAME})",
    ))
    .expect("FROM/JOIN regex");

    // CTE name(s) declared as `WITH name AS (` or `, name AS (` — these are not real tables.
    let cte_re = Regex::new(r"(?is)(?:\bWITH\s+|,\s*)([a-zA-Z_][a-zA-Z0-9_]*)\s+AS\s*\(")
        .expect("CTE regex");

    let mut out: Vec<String> = Vec::new();
    for vcap in view_body_re.captures_iter(sql) {
        let body = vcap.get(1).map(|m| m.as_str()).unwrap_or("");

        let cte_names: HashSet<String> = cte_re
            .captures_iter(body)
            .filter_map(|c| c.get(1).map(|m| m.as_str().to_ascii_lowercase()))
            .collect();

        for cap in from_join_re.captures_iter(body) {
            let raw = cap.get(1).unwrap().as_str();
            let bare = normalize_sql_table_name(raw);
            let bare_stripped = bare.trim_matches('"').to_string();
            // Skip CTE aliases (case-insensitive match on the unqualified name).
            if cte_names.contains(&bare_stripped.to_ascii_lowercase()) {
                continue;
            }
            out.push(bare_stripped);
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Tables targeted by top-level `ALTER TABLE name` (additive migrations).
///
/// Accepts PostgreSQL optional clauses before the table name: `IF EXISTS`, `ONLY` (in that order).
pub fn extract_alter_table_targets_from_migration_sql(sql: &str) -> Vec<String> {
    let re = Regex::new(
        r"(?i)ALTER\s+TABLE\s+(?:IF\s+EXISTS\s+)?(?:ONLY\s+)?([a-zA-Z_][a-zA-Z0-9_]*(?:\.[a-zA-Z_][a-zA-Z0-9_]*)?)\s+",
    )
    .expect("ALTER TABLE regex");
    let mut out = Vec::new();
    for cap in re.captures_iter(sql) {
        if let Some(m) = cap.get(1) {
            out.push(normalize_sql_table_name(m.as_str()));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Order `(table_name, sql)` snippets (e.g. one CREATE per table) for apply order.
pub fn order_migrations_by_foreign_key_sql(
    rows: Vec<(String, String)>,
) -> Result<Vec<(String, String)>, String> {
    if rows.is_empty() {
        return Ok(rows);
    }
    let names: HashSet<String> = rows.iter().map(|(n, _)| n.clone()).collect();
    let mut tables: Vec<TableInfo> = Vec::new();
    for (name, sql) in &rows {
        let deps: Vec<String> = extract_referenced_tables_from_migration_sql(sql)
            .into_iter()
            .filter(|d| names.contains(d) && d != name)
            .collect();
        tables.push(TableInfo {
            name: name.clone(),
            sql: String::new(),
            dependencies: deps,
        });
    }
    validate_foreign_key_references(&tables)?;
    let order = topological_sort(&tables)?;
    let mut by_name: HashMap<String, String> = rows.into_iter().collect();
    let mut out = Vec::new();
    for n in order {
        if let Some(sql) = by_name.remove(&n) {
            out.push((n, sql));
        }
    }
    Ok(out)
}

fn collect_sql_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_sql_files(&path, out)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("sql") {
            out.push(path);
        }
    }
    Ok(())
}

/// Tables targeted by `INSERT INTO`, `COPY … FROM`, or `UPDATE` statements in a seed SQL blob.
///
/// Returns unqualified (schema-stripped) table names, sorted, deduped. `SELECT` / DDL is ignored.
pub fn extract_inserted_tables_from_sql(sql: &str) -> Vec<String> {
    const QNAME: &str = r#"(?:(?:"[^"]+"|[a-zA-Z_][a-zA-Z0-9_]*)(?:\.(?:"[^"]+"|[a-zA-Z_][a-zA-Z0-9_]*))?)"#;
    let insert_re = Regex::new(&format!(r"(?is)\bINSERT\s+INTO\s+({QNAME})"))
        .expect("INSERT INTO regex");
    // `COPY <table> (cols) FROM ...` — we only bind targets of `FROM` (data loads), not `TO`
    // (exports). Anchored to whitespace + `FROM` to avoid catching `COPY ... TO` which is a dump.
    let copy_re =
        Regex::new(&format!(r"(?is)\bCOPY\s+({QNAME})\s*(?:\([^)]*\))?\s+FROM\b"))
            .expect("COPY FROM regex");
    let update_re =
        Regex::new(&format!(r"(?is)\bUPDATE\s+(?:ONLY\s+)?({QNAME})\s+SET\b"))
            .expect("UPDATE SET regex");

    let mut out: Vec<String> = Vec::new();
    for re in [&insert_re, &copy_re, &update_re] {
        for cap in re.captures_iter(sql) {
            if let Some(m) = cap.get(1) {
                out.push(normalize_sql_table_name(m.as_str()).trim_matches('"').to_string());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Table-level foreign-key edges `(from_table, to_table)` parsed from a migration SQL blob.
///
/// Extracts:
///
/// - Inline FKs in `CREATE TABLE X (... col T REFERENCES Y(col) ...)` — edge `(X, Y)`.
/// - `ALTER TABLE X … REFERENCES Y(col)` — edge `(X, Y)` for any REFERENCES inside the statement.
///
/// Both bare and schema-qualified identifiers are handled; returned names are unqualified
/// (schema-stripped). Self-references are preserved — callers decide whether to filter them.
pub fn extract_table_level_fk_edges_from_migration_sql(sql: &str) -> Vec<(String, String)> {
    const QNAME: &str = r#"(?:(?:"[^"]+"|[a-zA-Z_][a-zA-Z0-9_]*)(?:\.(?:"[^"]+"|[a-zA-Z_][a-zA-Z0-9_]*))?)"#;
    let ref_re = Regex::new(&format!(r"(?i)REFERENCES\s+({QNAME})\s*\("))
        .expect("REFERENCES regex");
    let mut out: Vec<(String, String)> = Vec::new();
    let bytes = sql.as_bytes();

    // 1) CREATE TABLE <from_table> ( <body> ) → all REFERENCES inside <body>.
    let create_re = Regex::new(&format!(
        r"(?i)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?({QNAME})\s*\(",
    ))
    .expect("CREATE TABLE header regex");
    for cap in create_re.captures_iter(sql) {
        let Some(name_m) = cap.get(1) else { continue };
        let from_table = normalize_sql_table_name(name_m.as_str())
            .trim_matches('"')
            .to_string();
        let header_end = cap.get(0).unwrap().end();
        // `header_end` points just past the `(`. Find matching `)`.
        let Some(body_end) = find_matching_close_paren(bytes, header_end - 1) else {
            continue;
        };
        let body = &sql[header_end..body_end];
        for rcap in ref_re.captures_iter(body) {
            let to_raw = rcap.get(1).unwrap().as_str();
            let to_table = normalize_sql_table_name(to_raw)
                .trim_matches('"')
                .to_string();
            out.push((from_table.clone(), to_table));
        }
    }

    // 2) ALTER TABLE <from_table> … ; — all REFERENCES up to the next top-level `;`.
    let alter_re = Regex::new(&format!(
        r"(?i)ALTER\s+TABLE\s+(?:IF\s+EXISTS\s+)?(?:ONLY\s+)?({QNAME})\b",
    ))
    .expect("ALTER TABLE header regex");
    for cap in alter_re.captures_iter(sql) {
        let Some(name_m) = cap.get(1) else { continue };
        let from_table = normalize_sql_table_name(name_m.as_str())
            .trim_matches('"')
            .to_string();
        let header_end = cap.get(0).unwrap().end();
        // Statement ends at next `;` at paren depth 0 (or end of input).
        let stmt_end = find_stmt_terminator(bytes, header_end).unwrap_or(bytes.len());
        let stmt = &sql[header_end..stmt_end];
        for rcap in ref_re.captures_iter(stmt) {
            let to_raw = rcap.get(1).unwrap().as_str();
            let to_table = normalize_sql_table_name(to_raw)
                .trim_matches('"')
                .to_string();
            out.push((from_table.clone(), to_table));
        }
    }

    out.sort();
    out.dedup();
    out
}

/// Find the index of the matching `)` given the byte index of the opening `(`.
fn find_matching_close_paren(bytes: &[u8], open: usize) -> Option<usize> {
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    let mut depth: usize = 0;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Find the index of the next `;` at paren depth 0, starting at `from`.
fn find_stmt_terminator(bytes: &[u8], from: usize) -> Option<usize> {
    let mut depth: usize = 0;
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b';' if depth == 0 => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

/// Build `seed_order.txt` at `out_path`: one relative-to-`seeds_root` path per line, in an order
/// that respects table-level foreign-key edges across seeds.
///
/// Workflow:
///
/// 1. Every `.sql` file under `migrations_dir` (recursive) is parsed for table-level FK edges
///    (both inline `CREATE TABLE` and `ALTER TABLE`). The union forms `table_fk_deps`.
/// 2. Every `seed_files[i]` is parsed for `INSERT INTO` / `COPY … FROM` / `UPDATE` targets.
/// 3. An edge `seed_i → seed_j` is added whenever `seed_j` targets a table `T` such that `T`'s FK
///    list includes a table `U` that `seed_i` populates. Tables populated by the same seed are
///    treated as "already satisfied" within that seed.
/// 4. The graph is topologically sorted with the same timestamp-aware tie-break used by
///    [`write_apply_order_file`], so independent seeds still flow in chronological order.
/// 5. A cycle returns `Err` with a message mentioning the involved seeds.
///
/// If `migrations_dir` is missing or empty the FK graph is empty — seeds are ordered by the
/// timestamp / path tie-break alone, so the output is still deterministic and safe to consume.
pub fn write_seed_order_file(
    migrations_dir: &Path,
    seeds_root: &Path,
    seed_files: &[PathBuf],
    out_path: &Path,
) -> Result<(), String> {
    use std::collections::BTreeSet;

    // 1) Table-level FK deps from every migration .sql file.
    let mut fk_deps: HashMap<String, HashSet<String>> = HashMap::new();
    if migrations_dir.is_dir() {
        let mut files: Vec<PathBuf> = Vec::new();
        collect_sql_files(migrations_dir, &mut files).map_err(|e| e.to_string())?;
        for path in &files {
            let content = fs::read_to_string(path)
                .map_err(|e| format!("{}: {}", path.display(), e))?;
            for (from, to) in extract_table_level_fk_edges_from_migration_sql(&content) {
                if from == to {
                    continue; // self-FKs do not create cross-seed ordering
                }
                fk_deps.entry(from).or_default().insert(to);
            }
        }
    }

    // 2) Seed -> target tables, plus reverse (table -> seeds that populate it).
    let seeds_root_canon = seeds_root
        .canonicalize()
        .unwrap_or_else(|_| seeds_root.to_path_buf());

    let mut seed_targets: Vec<(PathBuf, String, Vec<String>)> = Vec::new();
    for seed_path in seed_files {
        // Enforce strict `YYYYMMDDHHMMSS_<slug>.sql` naming for anything that ends up in
        // seed_order.txt. Seeds without a timestamp prefix are skipped with a stderr warning
        // instead of being silently ordered by lex — without a timestamp we have no deterministic
        // tie-break, and historically this class of file (`company_demo_organization.sql`) has
        // been the exact cause of FK-violating seed apply order. See
        // `postmortem-lifeguard-default-expr-2026-04.md` and the seed-order ADR.
        let filename = seed_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if parse_leading_timestamp(filename).is_none() {
            if std::env::var("LIFEGUARD_SILENCE_UNTIMESTAMPED_SEEDS")
                .ok()
                .as_deref()
                != Some("1")
            {
                eprintln!(
                    "warning: [lifeguard-migrate] skipping seed `{}` — missing \
                     `YYYYMMDDHHMMSS_<slug>.sql` timestamp prefix, so it will NOT be applied \
                     via seed_order.txt. Rename the file (e.g. `git mv {} {{timestamp}}_{}`) \
                     to bring it back into the ordered pipeline; set \
                     LIFEGUARD_SILENCE_UNTIMESTAMPED_SEEDS=1 to silence this warning.",
                    seed_path.display(),
                    filename,
                    filename
                );
            }
            continue;
        }
        let content = fs::read_to_string(seed_path)
            .map_err(|e| format!("{}: {}", seed_path.display(), e))?;
        let targets = extract_inserted_tables_from_sql(&content);
        let rel = seed_path
            .canonicalize()
            .unwrap_or_else(|_| seed_path.clone())
            .strip_prefix(&seeds_root_canon)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| seed_path.clone())
            .to_string_lossy()
            .replace('\\', "/");
        seed_targets.push((seed_path.clone(), rel, targets));
    }

    let mut populators: HashMap<String, BTreeSet<String>> = HashMap::new();
    for (_path, rel, targets) in &seed_targets {
        for t in targets {
            populators.entry(t.clone()).or_default().insert(rel.clone());
        }
    }

    // 3) Seed -> seed deps.
    let all_rels: HashSet<String> = seed_targets.iter().map(|(_, r, _)| r.clone()).collect();
    let mut deps: HashMap<String, HashSet<String>> = HashMap::new();
    for rel in &all_rels {
        deps.insert(rel.clone(), HashSet::new());
    }
    for (_path, rel, targets) in &seed_targets {
        let self_targets: HashSet<&String> = targets.iter().collect();
        for target in targets {
            let Some(target_refs) = fk_deps.get(target) else {
                continue;
            };
            for referenced in target_refs {
                if self_targets.contains(referenced) {
                    continue;
                }
                if let Some(seeds) = populators.get(referenced) {
                    for populator in seeds {
                        if populator != rel {
                            deps.get_mut(rel).unwrap().insert(populator.clone());
                        }
                    }
                }
            }
        }
    }

    // 4) Kahn + timestamp tie-break.
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    for r in &all_rels {
        in_degree.insert(r.clone(), deps.get(r).map(|s| s.len()).unwrap_or(0));
    }
    let mut reverse: HashMap<String, Vec<String>> = HashMap::new();
    for (r, ds) in &deps {
        for p in ds {
            reverse.entry(p.clone()).or_default().push(r.clone());
        }
    }

    let mut queue: Vec<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(k, _)| k.clone())
        .collect();

    let mut ordered: Vec<String> = Vec::new();
    loop {
        queue.sort_by(|a, b| compare_apply_order_entries(a, b));
        if queue.is_empty() {
            break;
        }
        let current = queue.remove(0);
        ordered.push(current.clone());
        if let Some(dependents) = reverse.get(&current) {
            for dep in dependents {
                let deg = in_degree.get_mut(dep).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push(dep.clone());
                }
            }
        }
    }

    if ordered.len() != all_rels.len() {
        let remaining: Vec<String> = all_rels
            .iter()
            .filter(|r| !ordered.contains(r))
            .cloned()
            .collect();
        return Err(format!(
            "Circular seed dependency detected among: {remaining:?}"
        ));
    }

    // 5) Emit.
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut f = fs::File::create(out_path).map_err(|e| e.to_string())?;
    writeln!(
        f,
        "# Auto-generated FK-safe seed order. Regenerate with: cargo run -p <your-migrator>"
    )
    .map_err(|e| e.to_string())?;
    for rel in ordered {
        writeln!(f, "{rel}").map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Parse a leading `YYYYMMDDHHMMSS` (14-digit) timestamp from a migration filename.
///
/// Accepts the filename *basename* (path separators are stripped first). Returns `None` if the
/// filename doesn't start with exactly 14 ASCII digits followed by `_` or `.` — this is tight on
/// purpose: the tie-break only wants to privilege real timestamps, never random numeric prefixes
/// that happen to parse as `u64`.
#[must_use]
pub fn parse_leading_timestamp(rel_or_filename: &str) -> Option<u64> {
    let basename = rel_or_filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(rel_or_filename);
    const TS_LEN: usize = 14;
    if basename.len() <= TS_LEN {
        return None;
    }
    let (head, tail) = basename.split_at(TS_LEN);
    // Exactly 14 ASCII digits, followed by `_` or `.` (e.g. `20260101000000_foo.sql`).
    if !head.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let next = tail.as_bytes().first().copied();
    if next != Some(b'_') && next != Some(b'.') {
        return None;
    }
    head.parse::<u64>().ok()
}

/// Ordering for two relative migration paths once their FK / view / ALTER dependencies are
/// satisfied. Earliest parseable `YYYYMMDDHHMMSS` first; timestamped files precede
/// non-timestamped files; within each bucket fall back to full relative path lex order.
fn compare_apply_order_entries(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let ta = parse_leading_timestamp(a);
    let tb = parse_leading_timestamp(b);
    match (ta, tb) {
        (Some(x), Some(y)) => x.cmp(&y).then_with(|| a.cmp(b)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

/// Build `apply_order.txt` under `migrations_dir`: one relative path per line (FK order).
///
/// `REFERENCES` / `ALTER TABLE` targets depend on the migration file that `CREATE TABLE IF NOT EXISTS`
/// for that table (lexicographically smallest rel when several files create the same name).
pub fn write_apply_order_file(migrations_dir: &Path) -> Result<(), String> {
    let migrations_dir = migrations_dir
        .canonicalize()
        .map_err(|e| format!("migrations dir {:?}: {}", migrations_dir, e))?;

    let mut files: Vec<PathBuf> = Vec::new();
    collect_sql_files(&migrations_dir, &mut files).map_err(|e| e.to_string())?;
    files.sort();

    #[derive(Debug)]
    struct FileMeta {
        rel: String,
        created: Vec<String>,
        alters: Vec<String>,
        refs: Vec<String>,
    }

    let mut metas: Vec<FileMeta> = Vec::new();
    for path in &files {
        let rel = path
            .strip_prefix(&migrations_dir)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let content = fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let created = extract_created_tables_from_migration_sql(&content);
        let alters = extract_alter_table_targets_from_migration_sql(&content);
        // Foreign-key references (`REFERENCES x(`) plus tables a view reads from (`FROM` /
        // `JOIN` inside `CREATE VIEW`). Both are "this file cannot run until X exists".
        let mut refs = extract_referenced_tables_from_migration_sql(&content);
        for t in extract_view_source_tables_from_migration_sql(&content) {
            refs.push(t);
        }
        refs.sort();
        refs.dedup();
        metas.push(FileMeta {
            rel,
            created,
            alters,
            refs,
        });
    }

    // table -> canonical creator file rel (lexicographically smallest wins)
    let mut table_creator: HashMap<String, String> = HashMap::new();
    for m in &metas {
        for t in &m.created {
            table_creator
                .entry(t.clone())
                .and_modify(|e| {
                    if m.rel < *e {
                        *e = m.rel.clone();
                    }
                })
                .or_insert_with(|| m.rel.clone());
        }
    }

    let all_rels: HashSet<String> = metas.iter().map(|m| m.rel.clone()).collect();

    // file rel -> set of prerequisite file rels
    let mut deps: HashMap<String, HashSet<String>> = HashMap::new();
    for m in &metas {
        let mut d = HashSet::new();
        for r in &m.refs {
            if let Some(creator) = table_creator.get(r) {
                if *creator != m.rel {
                    d.insert(creator.clone());
                }
            }
        }
        for t in &m.alters {
            if let Some(creator) = table_creator.get(t) {
                if *creator != m.rel {
                    d.insert(creator.clone());
                }
            }
        }
        deps.insert(m.rel.clone(), d);
    }

    // Kahn: prerequisite P must run before F when P is in deps[F]. in_degree[F] = |deps[F]|.
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    for r in &all_rels {
        let n = deps.get(r).map(|s| s.len()).unwrap_or(0);
        in_degree.insert(r.clone(), n);
    }

    let mut reverse: HashMap<String, Vec<String>> = HashMap::new();
    for (r, ds) in &deps {
        for p in ds {
            reverse.entry(p.clone()).or_default().push(r.clone());
        }
    }

    let mut queue: Vec<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(k, _)| k.clone())
        .collect();

    let mut ordered: Vec<String> = Vec::new();
    loop {
        // Tie-break among files with satisfied deps: earliest filename timestamp first, then
        // full relative path. Files whose filename does not start with a parseable
        // `YYYYMMDDHHMMSS` timestamp sort after all timestamped files (still stable by path).
        // FK / view / ALTER edges still dominate — only the in_degree==0 cohort is sorted here.
        queue.sort_by(|a, b| compare_apply_order_entries(a, b));
        if queue.is_empty() {
            break;
        }
        let current = queue.remove(0);
        ordered.push(current.clone());
        if let Some(dependents) = reverse.get(&current) {
            for dep in dependents {
                let deg = in_degree.get_mut(dep).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push(dep.clone());
                }
            }
        }
    }

    if ordered.len() != all_rels.len() {
        return Err(
            "Circular foreign-key / migration dependency among SQL files (check REFERENCES / ALTER order)"
                .to_string(),
        );
    }

    let out_path = migrations_dir.join("apply_order.txt");
    let mut f = fs::File::create(&out_path).map_err(|e| e.to_string())?;
    writeln!(
        f,
        "# Auto-generated FK-safe apply order. Regenerate with: cargo run -p hauliage_migrator"
    )
    .map_err(|e| e.to_string())?;
    for rel in ordered {
        writeln!(f, "{}", rel).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufRead;
    use tempfile::tempdir;

    #[test]
    fn extract_created_tables_multiple_per_file() {
        let sql = r"
CREATE TABLE IF NOT EXISTS zebra (id INT PRIMARY KEY);
CREATE TABLE IF NOT EXISTS public.alpha (id INT PRIMARY KEY REFERENCES zebra(id));
";
        assert_eq!(
            extract_created_tables_from_migration_sql(sql),
            vec!["alpha".to_string(), "zebra".to_string()]
        );
    }

    #[test]
    fn apply_order_registers_every_create_in_file() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        fs::write(
            base.join("001_parent.sql"),
            "CREATE TABLE IF NOT EXISTS parent (id INT PRIMARY KEY);\n",
        )
        .unwrap();
        fs::write(
            base.join("002_multi.sql"),
            r"CREATE TABLE IF NOT EXISTS child (id INT PRIMARY KEY, pid INT REFERENCES parent(id));
CREATE TABLE IF NOT EXISTS grand (id INT PRIMARY KEY, cid INT REFERENCES child(id));
",
        )
        .unwrap();
        fs::write(
            base.join("003_other.sql"),
            "CREATE TABLE IF NOT EXISTS other (id INT PRIMARY KEY, gid INT REFERENCES grand(id));\n",
        )
        .unwrap();

        write_apply_order_file(base).unwrap();
        let order_path = base.join("apply_order.txt");
        let f = fs::File::open(&order_path).unwrap();
        let lines: Vec<String> = std::io::BufReader::new(f)
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();

        let pos = |name: &str| {
            lines
                .iter()
                .position(|l| l.ends_with(name))
                .unwrap_or_else(|| panic!("missing {name} in {lines:?}"))
        };
        assert!(
            pos("001_parent.sql") < pos("002_multi.sql"),
            "parent before multi: {lines:?}"
        );
        assert!(
            pos("002_multi.sql") < pos("003_other.sql"),
            "multi (creates grand) before other refs grand: {lines:?}"
        );
    }

    #[test]
    fn references_parsing() {
        let sql = "job_id VARCHAR REFERENCES telemetry_locations(job_id)";
        assert_eq!(
            extract_referenced_tables_from_migration_sql(sql),
            vec!["telemetry_locations".to_string()]
        );
    }

    #[test]
    fn extract_alter_table_if_exists_table_name_not_if() {
        let sql = "ALTER TABLE IF EXISTS foo ADD COLUMN bar INT;";
        assert_eq!(
            extract_alter_table_targets_from_migration_sql(sql),
            vec!["foo".to_string()]
        );
    }

    #[test]
    fn extract_alter_table_only_before_name() {
        let sql = "ALTER TABLE ONLY public.baz ADD CONSTRAINT u UNIQUE (id);";
        assert_eq!(
            extract_alter_table_targets_from_migration_sql(sql),
            vec!["baz".to_string()]
        );
    }

    #[test]
    fn extract_alter_table_if_exists_only_combined() {
        let sql = "ALTER TABLE IF EXISTS ONLY qux DROP COLUMN old;";
        assert_eq!(
            extract_alter_table_targets_from_migration_sql(sql),
            vec!["qux".to_string()]
        );
    }

    #[test]
    fn order_migrations_self_referential_fk_single_table() {
        let sql = "CREATE TABLE IF NOT EXISTS employees (id SERIAL PRIMARY KEY, manager_id INT REFERENCES employees(id));";
        let ordered =
            order_migrations_by_foreign_key_sql(vec![("employees".into(), sql.into())]).unwrap();
        assert_eq!(ordered.len(), 1);
        assert_eq!(ordered[0].0, "employees");
    }

    #[test]
    fn order_migrations_timeline_after_locations() {
        let loc = (
            "telemetry_locations".into(),
            "CREATE TABLE IF NOT EXISTS telemetry_locations (job_id TEXT PRIMARY KEY);".into(),
        );
        let line = (
            "telemetry_timeline".into(),
            "CREATE TABLE IF NOT EXISTS telemetry_timeline (id SERIAL PRIMARY KEY, job_id TEXT REFERENCES telemetry_locations(job_id));".into(),
        );
        let ordered = order_migrations_by_foreign_key_sql(vec![line.clone(), loc.clone()]).unwrap();
        assert_eq!(ordered[0].0, "telemetry_locations");
        assert_eq!(ordered[1].0, "telemetry_timeline");
    }

    // --- Views: CREATE VIEW registers the view as a creatable object ---

    #[test]
    fn extract_created_tables_includes_create_or_replace_view() {
        let sql = r"
CREATE OR REPLACE VIEW hauliage.gics_industries_view AS
SELECT code, name, parent_code FROM hauliage.company_gics_categories WHERE level = 3;
";
        assert_eq!(
            extract_created_tables_from_migration_sql(sql),
            vec!["gics_industries_view".to_string()]
        );
    }

    #[test]
    fn extract_created_tables_includes_bare_create_view_if_not_exists() {
        let sql = "CREATE VIEW IF NOT EXISTS public.my_view AS SELECT 1 AS x;";
        assert_eq!(
            extract_created_tables_from_migration_sql(sql),
            vec!["my_view".to_string()]
        );
    }

    #[test]
    fn extract_created_tables_create_view_no_replace_no_if_not_exists() {
        let sql = "CREATE VIEW hauliage.v AS SELECT 1;";
        assert_eq!(
            extract_created_tables_from_migration_sql(sql),
            vec!["v".to_string()]
        );
    }

    // --- Views: extracting source tables the view reads from ---

    #[test]
    fn extract_view_source_tables_simple_from() {
        let sql = r"
CREATE OR REPLACE VIEW hauliage.gics_industries_view AS
SELECT code, name, parent_code FROM hauliage.company_gics_categories WHERE level = 3;
";
        assert_eq!(
            extract_view_source_tables_from_migration_sql(sql),
            vec!["company_gics_categories".to_string()]
        );
    }

    #[test]
    fn extract_view_source_tables_with_joins_and_alias() {
        let sql = r#"
CREATE OR REPLACE VIEW analytics.v AS
SELECT o.id, c.name
FROM hauliage.orders o
INNER JOIN hauliage.customers AS c ON c.id = o.customer_id
LEFT OUTER JOIN "hauliage"."shipments" s ON s.order_id = o.id
WHERE o.active;
"#;
        let got = extract_view_source_tables_from_migration_sql(sql);
        assert_eq!(
            got,
            vec![
                "customers".to_string(),
                "orders".to_string(),
                "shipments".to_string()
            ]
        );
    }

    #[test]
    fn extract_view_source_tables_ignores_non_view_files() {
        let sql = "CREATE TABLE IF NOT EXISTS t (id INT PRIMARY KEY, other_id INT REFERENCES other(id)); SELECT * FROM bogus;";
        assert!(extract_view_source_tables_from_migration_sql(sql).is_empty());
    }

    #[test]
    fn extract_view_source_tables_handles_subselects_and_ctes() {
        let sql = r"
CREATE OR REPLACE VIEW v AS
WITH recent AS (SELECT * FROM hauliage.events WHERE ts > now() - interval '1 day')
SELECT r.id, u.name
FROM recent r
JOIN hauliage.users u ON u.id = r.user_id;
";
        let got = extract_view_source_tables_from_migration_sql(sql);
        assert!(
            got.contains(&"events".to_string()),
            "should find events: {got:?}"
        );
        assert!(
            got.contains(&"users".to_string()),
            "should find users: {got:?}"
        );
    }

    // --- apply_order.txt: view must follow its source table even with later-lex filename ---

    #[test]
    fn apply_order_places_view_after_its_source_table_despite_lex_order() {
        // Source table file lex-sorts *after* the view file — without view-aware parsing, the view
        // would be emitted first and fail at runtime ("relation does not exist").
        let dir = tempdir().unwrap();
        let base = dir.path();
        let sub = base.join("company");
        fs::create_dir_all(&sub).unwrap();

        // View file has *earlier* lex order than the table file below
        fs::write(
            sub.join("20260417054900_gics_industries_view.sql"),
            "CREATE OR REPLACE VIEW hauliage.gics_industries_view AS \
             SELECT code FROM hauliage.company_gics_categories WHERE level = 3;\n",
        )
        .unwrap();
        // Table creation file has later timestamp (lex-greater)
        fs::write(
            sub.join("20260417054999_create_company_gics_categories.sql"),
            "CREATE TABLE IF NOT EXISTS hauliage.company_gics_categories \
             (code VARCHAR(8) PRIMARY KEY, name TEXT);\n",
        )
        .unwrap();

        write_apply_order_file(base).unwrap();
        let order_path = base.join("apply_order.txt");
        let f = fs::File::open(&order_path).unwrap();
        let lines: Vec<String> = std::io::BufReader::new(f)
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();

        let pos = |needle: &str| {
            lines
                .iter()
                .position(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("missing {needle} in {lines:?}"))
        };
        assert!(
            pos("create_company_gics_categories.sql") < pos("gics_industries_view.sql"),
            "table create must precede view despite lex ordering: {lines:?}"
        );
    }

    // --- Timestamp-aware tie-break in write_apply_order_file ---

    #[test]
    fn apply_order_tie_break_prefers_timestamp_over_path() {
        // Two independent tables (no FK edges). Without timestamp-aware sorting the alphabetical
        // path order puts `aaa/...` before `zzz/...`, but the `zzz/...` file has the *earlier*
        // timestamp. We want the earlier-timestamp file to come first so `apply_order.txt` reads
        // in roughly chronological order for independent migrations.
        let dir = tempdir().unwrap();
        let base = dir.path();
        let aaa = base.join("aaa");
        let zzz = base.join("zzz");
        fs::create_dir_all(&aaa).unwrap();
        fs::create_dir_all(&zzz).unwrap();

        // Earlier timestamp, later path component
        fs::write(
            zzz.join("20260101000000_early_table.sql"),
            "CREATE TABLE IF NOT EXISTS public.early_table (id INT PRIMARY KEY);\n",
        )
        .unwrap();
        // Later timestamp, earlier path component
        fs::write(
            aaa.join("20260501000000_late_table.sql"),
            "CREATE TABLE IF NOT EXISTS public.late_table (id INT PRIMARY KEY);\n",
        )
        .unwrap();

        write_apply_order_file(base).unwrap();
        let order_path = base.join("apply_order.txt");
        let f = fs::File::open(&order_path).unwrap();
        let lines: Vec<String> = std::io::BufReader::new(f)
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();

        let pos = |needle: &str| {
            lines
                .iter()
                .position(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("missing {needle} in {lines:?}"))
        };
        assert!(
            pos("early_table.sql") < pos("late_table.sql"),
            "earlier-timestamp file must be ordered before later-timestamp file \
             regardless of path lex order: {lines:?}"
        );
    }

    #[test]
    fn apply_order_tie_break_still_topological_when_fk_edges_conflict() {
        // FK edge must still win over timestamp. Create a later-timestamped "parent" that a
        // earlier-timestamped "child" references — child must still come *after* parent.
        let dir = tempdir().unwrap();
        let base = dir.path();
        fs::write(
            base.join("20260101000000_child.sql"),
            "CREATE TABLE IF NOT EXISTS child (id INT PRIMARY KEY, pid INT REFERENCES parent(id));\n",
        )
        .unwrap();
        fs::write(
            base.join("20260501000000_parent.sql"),
            "CREATE TABLE IF NOT EXISTS parent (id INT PRIMARY KEY);\n",
        )
        .unwrap();

        write_apply_order_file(base).unwrap();
        let order_path = base.join("apply_order.txt");
        let f = fs::File::open(&order_path).unwrap();
        let lines: Vec<String> = std::io::BufReader::new(f)
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();
        let pos = |needle: &str| {
            lines
                .iter()
                .position(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("missing {needle} in {lines:?}"))
        };
        assert!(
            pos("parent.sql") < pos("child.sql"),
            "FK dependency must dominate timestamp tie-break: {lines:?}"
        );
    }

    #[test]
    fn apply_order_tie_break_non_numeric_prefix_sorts_last() {
        // Files that don't have a parseable leading timestamp (`manual.sql`) should come after
        // files that do, and then fall back to path order among themselves.
        let dir = tempdir().unwrap();
        let base = dir.path();
        fs::write(
            base.join("20260101000000_a.sql"),
            "CREATE TABLE IF NOT EXISTS a (id INT PRIMARY KEY);\n",
        )
        .unwrap();
        fs::write(
            base.join("manual_b.sql"),
            "CREATE TABLE IF NOT EXISTS b (id INT PRIMARY KEY);\n",
        )
        .unwrap();
        write_apply_order_file(base).unwrap();
        let order_path = base.join("apply_order.txt");
        let f = fs::File::open(&order_path).unwrap();
        let lines: Vec<String> = std::io::BufReader::new(f)
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();
        let pos = |needle: &str| {
            lines
                .iter()
                .position(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("missing {needle} in {lines:?}"))
        };
        assert!(
            pos("20260101000000_a.sql") < pos("manual_b.sql"),
            "timestamped files should precede non-timestamped ones: {lines:?}"
        );
    }

    #[test]
    fn apply_order_places_view_after_source_even_across_services() {
        let dir = tempdir().unwrap();
        let base = dir.path();
        let src_dir = base.join("backing");
        let view_dir = base.join("aviews"); // alphabetically earlier than "backing"
        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(&view_dir).unwrap();

        fs::write(
            view_dir.join("20260101010101_v.sql"),
            "CREATE OR REPLACE VIEW public.v AS SELECT id FROM public.backing_table;\n",
        )
        .unwrap();
        fs::write(
            src_dir.join("20260101010102_backing_table.sql"),
            "CREATE TABLE IF NOT EXISTS public.backing_table (id INT PRIMARY KEY);\n",
        )
        .unwrap();

        write_apply_order_file(base).unwrap();
        let order_path = base.join("apply_order.txt");
        let f = fs::File::open(&order_path).unwrap();
        let lines: Vec<String> = std::io::BufReader::new(f)
            .lines()
            .map_while(Result::ok)
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();

        let pos = |needle: &str| {
            lines
                .iter()
                .position(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("missing {needle} in {lines:?}"))
        };
        assert!(
            pos("backing_table.sql") < pos("aviews/20260101010101_v.sql"),
            "backing table must be created before the view: {lines:?}"
        );
    }

    // ------------------------------------------------------------------
    // Seed-order support (analogous to apply-order, but for data seeds).
    // ------------------------------------------------------------------

    #[test]
    fn extract_inserted_tables_plain_insert_into() {
        let sql = "INSERT INTO hauliage.registered_addresses (id, country_code) VALUES ('a', 'ZW');";
        assert_eq!(
            extract_inserted_tables_from_sql(sql),
            vec!["registered_addresses".to_string()]
        );
    }

    #[test]
    fn extract_inserted_tables_handles_quoted_and_bare_identifiers() {
        let sql = r#"
INSERT INTO "hauliage"."organization_profiles" (id) VALUES ('x');
INSERT INTO organization_preferences (id) VALUES ('y');
        "#;
        let got = extract_inserted_tables_from_sql(sql);
        assert_eq!(
            got,
            vec![
                "organization_preferences".to_string(),
                "organization_profiles".to_string()
            ]
        );
    }

    #[test]
    fn extract_inserted_tables_includes_copy_and_update_targets() {
        let sql = r"
COPY hauliage.locations_countries (code, name) FROM STDIN;
UPDATE hauliage.organization_profiles SET company_size = 'large' WHERE id = 'x';
";
        let got = extract_inserted_tables_from_sql(sql);
        assert_eq!(
            got,
            vec![
                "locations_countries".to_string(),
                "organization_profiles".to_string()
            ]
        );
    }

    #[test]
    fn extract_inserted_tables_ignores_select_and_ddl() {
        let sql = r"
SELECT code FROM locations_countries;
CREATE TABLE IF NOT EXISTS widgets (id INT PRIMARY KEY);
ALTER TABLE widgets ADD COLUMN sku VARCHAR(50);
";
        assert!(extract_inserted_tables_from_sql(sql).is_empty());
    }

    // --- extract_table_level_fk_edges_from_migration_sql ---

    #[test]
    fn fk_edges_from_inline_references_in_create_table() {
        let sql = r"
CREATE TABLE IF NOT EXISTS hauliage.registered_addresses (
    id UUID PRIMARY KEY,
    country_code VARCHAR(2) NOT NULL REFERENCES hauliage.locations_countries(code) ON UPDATE CASCADE
);
";
        let edges = extract_table_level_fk_edges_from_migration_sql(sql);
        assert_eq!(
            edges,
            vec![(
                "registered_addresses".to_string(),
                "locations_countries".to_string()
            )]
        );
    }

    #[test]
    fn fk_edges_from_alter_table_add_column_references() {
        let sql = r"
ALTER TABLE hauliage.organization_profiles
  ADD COLUMN IF NOT EXISTS gics_industry_code VARCHAR(8) REFERENCES hauliage.company_gics_categories(code) ON DELETE SET NULL;
";
        let edges = extract_table_level_fk_edges_from_migration_sql(sql);
        assert_eq!(
            edges,
            vec![(
                "organization_profiles".to_string(),
                "company_gics_categories".to_string()
            )]
        );
    }

    #[test]
    fn fk_edges_multi_table_multi_edge_file() {
        let sql = r"
CREATE TABLE IF NOT EXISTS a (id INT PRIMARY KEY);
CREATE TABLE IF NOT EXISTS b (id INT PRIMARY KEY, a_id INT REFERENCES a(id));
CREATE TABLE IF NOT EXISTS c (
    id INT PRIMARY KEY,
    a_id INT REFERENCES a(id),
    b_id INT REFERENCES b(id)
);
";
        let mut edges = extract_table_level_fk_edges_from_migration_sql(sql);
        edges.sort();
        assert_eq!(
            edges,
            vec![
                ("b".to_string(), "a".to_string()),
                ("c".to_string(), "a".to_string()),
                ("c".to_string(), "b".to_string()),
            ]
        );
    }

    #[test]
    fn fk_edges_self_reference_is_preserved() {
        // Self-FKs are real edges; callers that don't want them filter at use-site. (Consistent
        // with how `order_migrations_by_foreign_key_sql` handles self-FK filtering upstream.)
        let sql = "CREATE TABLE IF NOT EXISTS tree (id INT PRIMARY KEY, parent_id INT REFERENCES tree(id));";
        let edges = extract_table_level_fk_edges_from_migration_sql(sql);
        assert_eq!(edges, vec![("tree".to_string(), "tree".to_string())]);
    }

    // --- write_seed_order_file ---

    #[test]
    fn seed_order_places_location_seed_before_company_seed_across_services() {
        // Reproduce the Hauliage failure mode:
        //
        //   company/…/company_demo_organization.sql
        //     INSERT INTO registered_addresses … country_code='ZW'
        //   locations/…/seed_normalized_locations.sql
        //     INSERT INTO locations_countries
        //
        //  `registered_addresses.country_code -> locations_countries(code)` is a migration-level
        //  FK. Seed order must therefore be: locations-seed, then company-seed. Alphabetical-by-
        //  path was putting them in the opposite order.
        let base = tempdir().unwrap();
        let base = base.path();

        // Minimal migrations graph: registered_addresses FKs to locations_countries.
        let migrations_dir = base.join("migrations");
        fs::create_dir_all(&migrations_dir).unwrap();
        fs::write(
            migrations_dir.join("01_countries.sql"),
            "CREATE TABLE IF NOT EXISTS locations_countries (code VARCHAR(2) PRIMARY KEY, name TEXT);\n",
        )
        .unwrap();
        fs::write(
            migrations_dir.join("02_addresses.sql"),
            "CREATE TABLE IF NOT EXISTS registered_addresses (\
                id UUID PRIMARY KEY, \
                country_code VARCHAR(2) NOT NULL REFERENCES locations_countries(code)\
             );\n",
        )
        .unwrap();

        // Seeds. Use `YYYYMMDDHHMMSS_<slug>.sql` naming — the enforcement in
        // `write_seed_order_file` skips any seed without a timestamp prefix, so the legacy
        // untimestamped files this test used previously would now be filtered out of the output.
        let seeds_root = base.join("microservices");
        let company_seed = seeds_root
            .join("company/impl/seeds/20260422000000_company_demo_organization.sql");
        let loc_seed = seeds_root
            .join("locations/impl/seeds/20260414191699_seed_normalized_locations.sql");
        fs::create_dir_all(company_seed.parent().unwrap()).unwrap();
        fs::create_dir_all(loc_seed.parent().unwrap()).unwrap();
        fs::write(
            &company_seed,
            "INSERT INTO registered_addresses (id, country_code) VALUES ('x', 'ZW');\n",
        )
        .unwrap();
        fs::write(
            &loc_seed,
            "INSERT INTO locations_countries (code, name) VALUES ('ZW', 'Zimbabwe');\n",
        )
        .unwrap();

        let out = base.join("seed_order.txt");
        write_seed_order_file(
            &migrations_dir,
            &seeds_root,
            &[company_seed.clone(), loc_seed.clone()],
            &out,
        )
        .unwrap();

        let text = fs::read_to_string(&out).unwrap();
        let lines: Vec<&str> = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();
        let pos = |needle: &str| {
            lines
                .iter()
                .position(|l| l.contains(needle))
                .unwrap_or_else(|| panic!("missing {needle} in {lines:?}"))
        };
        assert!(
            pos("seed_normalized_locations.sql") < pos("company_demo_organization.sql"),
            "locations seed must precede company seed so the FK to locations_countries is \
             satisfied: {lines:?}"
        );
        // Paths should be relative to seeds_root so setup-db.sh can resolve them directly.
        for line in &lines {
            assert!(
                !line.starts_with('/'),
                "line must be relative to seeds_root: {line}"
            );
        }
    }

    #[test]
    fn seed_order_preserves_timestamp_order_when_no_cross_fks() {
        // Two seeds for independent tables — timestamp tie-break should place the earlier one
        // first.
        let base = tempdir().unwrap();
        let base = base.path();
        let migrations_dir = base.join("migrations");
        fs::create_dir_all(&migrations_dir).unwrap();
        fs::write(
            migrations_dir.join("01_a.sql"),
            "CREATE TABLE IF NOT EXISTS a (id INT PRIMARY KEY);\n",
        )
        .unwrap();
        fs::write(
            migrations_dir.join("02_b.sql"),
            "CREATE TABLE IF NOT EXISTS b (id INT PRIMARY KEY);\n",
        )
        .unwrap();

        let seeds_root = base.join("seeds");
        fs::create_dir_all(&seeds_root).unwrap();
        let s_later = seeds_root.join("20260501000000_seed_a.sql");
        let s_earlier = seeds_root.join("20260101000000_seed_b.sql");
        fs::write(&s_later, "INSERT INTO a (id) VALUES (1);\n").unwrap();
        fs::write(&s_earlier, "INSERT INTO b (id) VALUES (1);\n").unwrap();

        let out = base.join("seed_order.txt");
        write_seed_order_file(
            &migrations_dir,
            &seeds_root,
            &[s_later.clone(), s_earlier.clone()],
            &out,
        )
        .unwrap();

        let text = fs::read_to_string(&out).unwrap();
        let lines: Vec<&str> = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();
        assert_eq!(lines.len(), 2);
        assert!(
            lines[0].contains("20260101000000_seed_b.sql"),
            "earlier-timestamp seed first: {lines:?}"
        );
    }

    #[test]
    fn seed_order_tolerates_missing_migration_graph() {
        // If the migrations directory is empty or unreachable, seeds must still be ordered in a
        // deterministic (timestamp → path) fallback rather than erroring.
        let base = tempdir().unwrap();
        let base = base.path();
        let migrations_dir = base.join("missing_migrations");

        let seeds_root = base.join("seeds");
        fs::create_dir_all(&seeds_root).unwrap();
        let s_a = seeds_root.join("20260101000000_a.sql");
        let s_b = seeds_root.join("20260201000000_b.sql");
        fs::write(&s_a, "INSERT INTO a (id) VALUES (1);\n").unwrap();
        fs::write(&s_b, "INSERT INTO b (id) VALUES (1);\n").unwrap();

        let out = base.join("seed_order.txt");
        write_seed_order_file(
            &migrations_dir,
            &seeds_root,
            &[s_b.clone(), s_a.clone()],
            &out,
        )
        .unwrap();
        let text = fs::read_to_string(&out).unwrap();
        let lines: Vec<&str> = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("20260101000000_a.sql"));
        assert!(lines[1].contains("20260201000000_b.sql"));
    }

    #[test]
    fn seed_order_skips_seeds_without_timestamp_prefix() {
        // Files lacking the strict `YYYYMMDDHHMMSS_<slug>.sql` naming convention must be excluded
        // from seed_order.txt — they're a liability for FK-correct ordering because without a
        // timestamp there's no deterministic tie-break.
        let base = tempdir().unwrap();
        let base = base.path();
        let migrations_dir = base.join("migrations");
        fs::create_dir_all(&migrations_dir).unwrap();
        fs::write(
            migrations_dir.join("01_a.sql"),
            "CREATE TABLE IF NOT EXISTS a (id INT PRIMARY KEY);\n",
        )
        .unwrap();

        let seeds_root = base.join("seeds");
        fs::create_dir_all(&seeds_root).unwrap();
        let good = seeds_root.join("20260501000000_seed_a.sql");
        let bad_no_prefix = seeds_root.join("company_demo_orphan.sql");
        let bad_short_ts = seeds_root.join("2026050_bad.sql");
        fs::write(&good, "INSERT INTO a (id) VALUES (1);\n").unwrap();
        fs::write(&bad_no_prefix, "INSERT INTO a (id) VALUES (2);\n").unwrap();
        fs::write(&bad_short_ts, "INSERT INTO a (id) VALUES (3);\n").unwrap();

        let out = base.join("seed_order.txt");
        write_seed_order_file(
            &migrations_dir,
            &seeds_root,
            &[good.clone(), bad_no_prefix.clone(), bad_short_ts.clone()],
            &out,
        )
        .unwrap();
        let text = fs::read_to_string(&out).unwrap();
        let lines: Vec<&str> = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
            .collect();
        assert_eq!(lines.len(), 1, "exactly the one correctly-named seed: {lines:?}");
        assert!(lines[0].contains("20260501000000_seed_a.sql"));
        assert!(!text.contains("company_demo_orphan.sql"), "no-prefix seed must be absent");
        assert!(!text.contains("2026050_bad.sql"), "short-prefix seed must be absent");
    }

    #[test]
    fn seed_order_detects_cycles_and_errors() {
        // Contrived: seedA populates A which FKs to B; seedB populates B which FKs to A.
        let base = tempdir().unwrap();
        let base = base.path();
        let migrations_dir = base.join("migrations");
        fs::create_dir_all(&migrations_dir).unwrap();
        // Intentional cycle A↔B to exercise error path.
        fs::write(
            migrations_dir.join("01_a.sql"),
            "CREATE TABLE IF NOT EXISTS a (id INT PRIMARY KEY, b_id INT REFERENCES b(id));\n",
        )
        .unwrap();
        fs::write(
            migrations_dir.join("02_b.sql"),
            "CREATE TABLE IF NOT EXISTS b (id INT PRIMARY KEY, a_id INT REFERENCES a(id));\n",
        )
        .unwrap();

        let seeds_root = base.join("seeds");
        fs::create_dir_all(&seeds_root).unwrap();
        // Valid `YYYYMMDDHHMMSS_<slug>.sql` prefixes so the seeds are actually considered;
        // the untimestamped-skip enforcement happens before cycle detection runs.
        let sa = seeds_root.join("20260101000000_a.sql");
        let sb = seeds_root.join("20260101000001_b.sql");
        fs::write(&sa, "INSERT INTO a (id) VALUES (1);\n").unwrap();
        fs::write(&sb, "INSERT INTO b (id) VALUES (1);\n").unwrap();

        let out = base.join("seed_order.txt");
        let err = write_seed_order_file(&migrations_dir, &seeds_root, &[sa, sb], &out)
            .expect_err("cycle should error");
        assert!(
            err.to_lowercase().contains("cycle") || err.to_lowercase().contains("circular"),
            "error should mention cycle/circular: {err}"
        );
    }
}
