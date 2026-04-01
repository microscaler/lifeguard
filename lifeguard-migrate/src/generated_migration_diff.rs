//! Delta SQL for entity-driven migrations.
//!
//! When prior `*_generated_from_entities.sql` files exist in the service directory, new runs
//! merge **all** of them in timestamp order, then compare the effective schema to the current
//! generator output and emit **`ALTER TABLE ... ADD COLUMN IF NOT EXISTS`** (and new index lines)
//! instead of duplicating full `CREATE TABLE` bodies. Delta-only files (ALTER without a new
//! `CREATE TABLE` in that file) stay merged with the last full snapshot so the latest file is
//! never misread as the whole baseline.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Locate the newest `TIMESTAMP_generated_from_entities.sql` in `dir` (by numeric timestamp prefix).
#[must_use]
pub fn find_latest_generated_migration(dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    let read = fs::read_dir(dir).ok()?;
    for ent in read.flatten() {
        let name = ent.file_name();
        let name = name.to_string_lossy();
        let Some(prefix) = name.strip_suffix("_generated_from_entities.sql") else {
            continue;
        };
        let Some(ts) = prefix.parse::<u64>().ok() else {
            continue;
        };
        match best {
            None => best = Some((ts, ent.path())),
            Some((t0, _)) if ts > t0 => best = Some((ts, ent.path())),
            _ => {}
        }
    }
    best.map(|(_, p)| p)
}

/// All `TIMESTAMP_generated_from_entities.sql` in `dir`, sorted by numeric timestamp ascending.
#[must_use]
pub fn list_generated_migration_paths_chronological(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<(u64, PathBuf)> = Vec::new();
    let Ok(read) = fs::read_dir(dir) else {
        return Vec::new();
    };
    for ent in read.flatten() {
        let name = ent.file_name();
        let name = name.to_string_lossy();
        let Some(prefix) = name.strip_suffix("_generated_from_entities.sql") else {
            continue;
        };
        let Some(ts) = prefix.parse::<u64>().ok() else {
            continue;
        };
        v.push((ts, ent.path()));
    }
    v.sort_by_key(|(ts, _)| *ts);
    v.into_iter().map(|(_, p)| p).collect()
}

/// Per-table state after replaying migration files in chronological order.
#[derive(Clone, Default)]
pub struct TableBaselineParts {
    /// Last section for this table that contained a full `CREATE TABLE IF NOT EXISTS`.
    pub last_create_section: Option<String>,
    /// Sections that were not a full `CREATE` (typically `ALTER TABLE ... ADD COLUMN`, extra indexes).
    pub delta_section_fragments: Vec<String>,
}

/// Column names → definition tail (everything after the column name) from a merged table baseline:
/// `CREATE TABLE` column lines plus `ADD COLUMN` / `ADD COLUMN IF NOT EXISTS` from merged deltas.
///
/// Used by [`crate::schema_migration_compare`] for column-level reconciliation with `information_schema`.
#[must_use]
pub fn column_map_from_merged_baseline(parts: &TableBaselineParts) -> BTreeMap<String, String> {
    let combined = combined_old_section(parts);
    let mut cols = BTreeMap::new();
    if let Some(sec) = parts.last_create_section.as_ref() {
        if let Some((_, body, _)) = extract_create_and_tail(sec) {
            cols = parse_column_defs_from_create_body(&body);
        }
    }
    for (name, def) in parse_add_columns_from_alter_blob(&combined) {
        cols.insert(name, def);
    }
    cols
}

/// Replay every `*_generated_from_entities.sql` under `dir` (oldest first) and merge table sections.
#[must_use]
pub fn accumulate_table_baselines_from_dir(dir: &Path) -> BTreeMap<String, TableBaselineParts> {
    let mut map: BTreeMap<String, TableBaselineParts> = BTreeMap::new();
    for path in list_generated_migration_paths_chronological(dir) {
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let sections = extract_table_sections(&content);
        for (table_name, sec) in sections {
            let entry = map.entry(table_name).or_default();
            if extract_create_and_tail(&sec).is_some() {
                entry.last_create_section = Some(sec);
                entry.delta_section_fragments.clear();
            } else {
                entry.delta_section_fragments.push(sec);
            }
        }
    }
    map
}

#[must_use]
pub fn combined_old_section(parts: &TableBaselineParts) -> String {
    let mut s = parts.last_create_section.clone().unwrap_or_default();
    for frag in &parts.delta_section_fragments {
        if !s.is_empty() && !frag.trim().is_empty() {
            s.push('\n');
        }
        s.push_str(frag);
    }
    s
}

/// After `ON`, optional **`USING`** *method*, require `(` for the btree key list.
fn tail_after_qualified_table_for_create_index(after_table: &str) -> Option<&str> {
    let mut t = after_table.trim_start();
    if t.starts_with('(') {
        return Some(t);
    }
    if t.len() < 6 || !t[..6].eq_ignore_ascii_case("using ") {
        return None;
    }
    t = t[6..].trim_start();
    let end = t
        .find(|c: char| c.is_whitespace() || c == '(')
        .unwrap_or(t.len());
    if t.as_bytes().get(end) == Some(&b'(') {
        return Some(&t[end..]);
    }
    t = t[end..].trim_start();
    t.starts_with('(').then_some(t)
}

/// Split `schema.table` / `"schema"."table"` / `table` from the start of `s`; remainder is key list / `INCLUDE` / `WHERE`.
fn split_qualified_table_prefix(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    let mut i = 0usize;
    loop {
        let rest = &s[i..];
        if rest.is_empty() {
            return None;
        }
        if rest.starts_with('"') {
            let end = rest[1..].find('"')?;
            i += 1 + end + 2;
        } else {
            let seg_end = rest
                .find(|c: char| c == '.' || c.is_whitespace() || c == '(')
                .unwrap_or(rest.len());
            if seg_end == 0 {
                return None;
            }
            i += seg_end;
        }
        let after = &s[i..];
        if after.starts_with('.') {
            i += 1;
            continue;
        }
        break;
    }
    Some((&s[..i], &s[i..]))
}

fn split_first_ident_token(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    if s.starts_with('"') {
        let end = s[1..].find('"')?;
        return Some((s[1..end + 1].to_string(), &s[end + 2..]));
    }
    let end = s
        .find(|c: char| c.is_whitespace() || c == '(')
        .unwrap_or(s.len());
    if end == 0 {
        return None;
    }
    Some((s[..end].to_string(), &s[end..]))
}

/// Parse a single-line `CREATE [UNIQUE] INDEX … ON …` from merged migration text.
///
/// Returns `(index_name, unqualified_table_name, statement_without_trailing_semicolon)` when the
/// line matches; `None` otherwise. Hand-written deltas may use **`USING`** before the key list.
fn try_parse_create_index_statement(line: &str) -> Option<(String, String, String)> {
    let stmt = line.trim().trim_end_matches(';').trim();
    let upper = stmt.to_ascii_uppercase();
    let rest = if upper.starts_with("CREATE UNIQUE INDEX ") {
        &stmt["CREATE UNIQUE INDEX ".len()..]
    } else if upper.starts_with("CREATE INDEX ") {
        &stmt["CREATE INDEX ".len()..]
    } else {
        return None;
    };
    let mut r = rest.trim_start();
    while r.len() >= 13 && r[..13].eq_ignore_ascii_case("CONCURRENTLY ") {
        r = r[13..].trim_start();
    }
    while r.len() >= 14 && r[..14].eq_ignore_ascii_case("IF NOT EXISTS ") {
        r = r[14..].trim_start();
    }
    let (idx_name, after_idx) = split_first_ident_token(r)?;
    let mut after = after_idx.trim_start();
    if after.len() < 3 || !after[..3].eq_ignore_ascii_case("ON ") {
        return None;
    }
    after = after[3..].trim_start();
    let (_qtable, after_tbl) = split_qualified_table_prefix(after)?;
    let after_keys = tail_after_qualified_table_for_create_index(after_tbl)?;
    if !after_keys.starts_with('(') {
        return None;
    }
    let table_unqual = _qtable
        .rsplit('.')
        .next()?
        .trim_matches('"')
        .to_string();
    Some((idx_name, table_unqual, stmt.to_string()))
}

/// `CREATE INDEX` / `CREATE UNIQUE INDEX` lines from a merged table baseline, keyed by index name.
///
/// Later lines in [`combined_old_section`] win when the same index name appears more than once.
#[must_use]
pub fn index_statements_for_table_from_merged_baseline(
    parts: &TableBaselineParts,
    table: &str,
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let combined = combined_old_section(parts);
    for line in combined.lines() {
        let Some((idx, tbl, stmt)) = try_parse_create_index_statement(line) else {
            continue;
        };
        if tbl == table {
            map.insert(idx, stmt);
        }
    }
    map
}

/// True if `dir` contains at least one `*_generated_from_entities.sql`.
#[must_use]
pub fn service_dir_has_generated_migrations(dir: &Path) -> bool {
    !list_generated_migration_paths_chronological(dir).is_empty()
}

fn parse_add_columns_from_alter_blob(blob: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for raw in blob.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("--") {
            continue;
        }
        let upper = line.to_ascii_uppercase();
        if !upper.starts_with("ALTER TABLE ") {
            continue;
        }
        let rest = line["ALTER TABLE ".len()..].trim_start();
        let Some(ws) = rest.find(char::is_whitespace) else {
            continue;
        };
        let after_table = rest[ws..].trim_start();
        let Some(rest2) = after_add_column_clause(after_table) else {
            continue;
        };
        let rest2 = rest2.trim_end_matches(';').trim();
        let Some(col_end) = rest2.find(char::is_whitespace) else {
            continue;
        };
        let col = rest2[..col_end].trim_matches('"');
        let def = rest2[col_end..].trim();
        if is_simple_ident(col) && !def.is_empty() {
            out.insert(col.to_string(), def.to_string());
        }
    }
    out
}

fn after_add_column_clause(s: &str) -> Option<&str> {
    let s = s.trim_start();
    const LONG: &str = "ADD COLUMN IF NOT EXISTS ";
    const SHORT: &str = "ADD COLUMN ";
    if s.len() >= LONG.len() && s[..LONG.len()].eq_ignore_ascii_case(LONG) {
        return Some(s[LONG.len()..].trim_start());
    }
    if s.len() >= SHORT.len() && s[..SHORT.len()].eq_ignore_ascii_case(SHORT) {
        return Some(s[SHORT.len()..].trim_start());
    }
    None
}

fn accumulated_from_single_file_content(content: &str) -> BTreeMap<String, TableBaselineParts> {
    let sections = extract_table_sections(content);
    let mut out = BTreeMap::new();
    for (name, sec) in sections {
        let mut parts = TableBaselineParts::default();
        if extract_create_and_tail(&sec).is_some() {
            parts.last_create_section = Some(sec);
        } else {
            parts.delta_section_fragments.push(sec);
        }
        out.insert(name, parts);
    }
    out
}

/// Build SQL for one service using merged history from every migration file in `service_dir`.
#[must_use]
pub fn build_service_migration_body_from_service_dir(
    service_dir: &Path,
    tables: &[(String, String)],
) -> String {
    let acc = accumulate_table_baselines_from_dir(service_dir);
    build_service_migration_body_from_accumulated(&acc, tables)
}

/// Build SQL from merged per-table baseline parts (see [`accumulate_table_baselines_from_dir`]).
#[must_use]
pub fn build_service_migration_body_from_accumulated(
    acc: &BTreeMap<String, TableBaselineParts>,
    tables: &[(String, String)],
) -> String {
    let mut out = String::new();

    for (table_name, new_sql) in tables {
        let Some(parts) = acc.get(table_name) else {
            out.push_str(&format!("-- Table: {table_name}\n{new_sql}\n\n"));
            continue;
        };
        if parts.last_create_section.is_none() && parts.delta_section_fragments.is_empty() {
            out.push_str(&format!("-- Table: {table_name}\n{new_sql}\n\n"));
            continue;
        }

        let combined = combined_old_section(parts);

        let Some((_, new_body, new_tail)) = extract_create_and_tail(new_sql) else {
            out.push_str(&format!("-- Table: {table_name}\n{new_sql}\n\n"));
            continue;
        };

        let mut old_cols = BTreeMap::new();
        if let Some(ref create_sec) = parts.last_create_section {
            if let Some((_, body, _)) = extract_create_and_tail(create_sec) {
                old_cols = parse_column_defs_from_create_body(&body);
            }
        }
        for frag in &parts.delta_section_fragments {
            for (c, d) in parse_add_columns_from_alter_blob(frag) {
                old_cols.insert(c, d);
            }
        }

        let new_cols = parse_column_defs_from_create_body(&new_body);

        let mut table_delta = String::new();
        for (col, def) in &new_cols {
            if old_cols.contains_key(col) {
                continue;
            }
            table_delta.push_str(&format!(
                "ALTER TABLE {table_name} ADD COLUMN IF NOT EXISTS {col} {def};\n"
            ));
        }

        let index_and_more = tail_lines_not_in_old(&combined, &new_tail);
        if table_delta.is_empty() && index_and_more.is_empty() {
            continue;
        }

        out.push_str(&format!("-- Table: {table_name}\n"));
        out.push_str(&table_delta);
        out.push_str(&index_and_more);
        if !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push('\n');
    }

    out
}

/// Split a generated migration file into `-- Table: name` sections (value = body after the header line).
#[must_use]
pub fn extract_table_sections(sql: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut buf = String::new();

    for line in sql.lines() {
        if let Some(rest) = line.strip_prefix("-- Table: ") {
            if let Some(name) = current.take() {
                map.insert(name, buf);
                buf = String::new();
            }
            current = Some(rest.trim().to_string());
            continue;
        }
        if current.is_some() {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    if let Some(name) = current {
        map.insert(name, buf);
    }
    map
}

/// Extract `(table_bare, create_body, tail_after_create)` from a single table section.
fn extract_create_and_tail(section: &str) -> Option<(String, String, String)> {
    let key = "CREATE TABLE IF NOT EXISTS ";
    let idx = section.find(key)?;
    let after = &section[idx + key.len()..];
    let after = after.trim_start();
    let open = after.find('(')?;
    let table_raw = after[..open].trim().trim_matches('"');
    let table_bare = table_raw
        .rsplit('.')
        .next()
        .unwrap_or(table_raw)
        .to_string();
    let from_paren = &after[open..];
    let (body, close_idx) = extract_paren_body(from_paren)?;
    let after_paren = from_paren[close_idx + 1..].trim_start();
    let tail = match after_paren.strip_prefix(';') {
        Some(rest) => rest.trim_start().to_string(),
        None => after_paren.to_string(),
    };
    Some((table_bare, body, tail))
}

/// `s` must start with `(`; returns inner body and index of closing `)` within `s`.
fn extract_paren_body(s: &str) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'(') {
        return None;
    }
    let mut depth = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((s[1..i].to_string(), i));
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

fn split_create_items(body: &str) -> Vec<String> {
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut out = Vec::new();
    for (i, c) in body.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(body[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    let tail = body[start..].trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

fn parse_column_defs_from_create_body(body: &str) -> BTreeMap<String, String> {
    let mut cols = BTreeMap::new();
    for item in split_create_items(body) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let upper = item.to_ascii_uppercase();
        if upper.starts_with("CONSTRAINT ")
            || upper.starts_with("UNIQUE(")
            || upper.starts_with("PRIMARY KEY")
            || upper.starts_with("FOREIGN KEY")
        {
            continue;
        }
        let Some(first) = item.split_whitespace().next() else {
            continue;
        };
        if !is_simple_ident(first) {
            continue;
        }
        let rest = item[first.len()..].trim_start();
        if rest.is_empty() {
            continue;
        }
        cols.insert(first.to_string(), rest.to_string());
    }
    cols
}

fn is_simple_ident(s: &str) -> bool {
    let mut it = s.chars();
    let Some(f) = it.next() else {
        return false;
    };
    if !(f.is_ascii_alphabetic() || f == '_') {
        return false;
    }
    it.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn index_line_with_if_not_exists(line: &str) -> String {
    let t = line.trim();
    let upper = t.to_ascii_uppercase();
    if upper.starts_with("CREATE UNIQUE INDEX ") {
        let rest = &t["CREATE UNIQUE INDEX ".len()..];
        format!("CREATE UNIQUE INDEX IF NOT EXISTS {rest}")
    } else if upper.starts_with("CREATE INDEX ") {
        let rest = &t["CREATE INDEX ".len()..];
        format!("CREATE INDEX IF NOT EXISTS {rest}")
    } else {
        t.to_string()
    }
}

fn non_empty_sql_lines(blob: &str) -> Vec<String> {
    blob.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("--"))
        .map(std::string::ToString::to_string)
        .collect()
}

fn tail_lines_not_in_old(old_section: &str, new_tail: &str) -> String {
    let old_set: std::collections::HashSet<String> = non_empty_sql_lines(old_section)
        .into_iter()
        .collect();
    let mut out = String::new();
    for line in non_empty_sql_lines(new_tail) {
        if old_set.contains(&line) {
            continue;
        }
        let upper = line.to_ascii_uppercase();
        let patched = if upper.starts_with("CREATE INDEX ") || upper.starts_with("CREATE UNIQUE INDEX ")
        {
            index_line_with_if_not_exists(&line)
        } else {
            line
        };
        out.push_str(&patched);
        if !patched.ends_with(';') {
            out.push(';');
        }
        out.push('\n');
    }
    out
}

/// Build SQL for one service: either full CREATE blobs (no baseline) or ALTER / new-table deltas.
///
/// `previous_file` is treated as a single migration document (tests and simple callers). For
/// multiple files on disk use [`build_service_migration_body_from_service_dir`].
#[must_use]
pub fn build_service_migration_body(
    previous_file: Option<&str>,
    tables: &[(String, String)],
) -> String {
    let acc = match previous_file {
        None => BTreeMap::new(),
        Some(content) => accumulated_from_single_file_content(content),
    };
    build_service_migration_body_from_accumulated(&acc, tables)
}

/// True if there is nothing to write (schema matches baseline).
#[must_use]
pub fn service_migration_is_empty(body: &str) -> bool {
    body.trim().is_empty()
}

/// Normalize per-table SQL for comparison (line endings, trailing whitespace).
#[must_use]
pub fn normalize_table_sql_blob(s: &str) -> String {
    s.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// True when each entity-generated table SQL matches the same table section in a **single**
/// migration document. Ignores file-level headers (`-- Version:`, `-- Generated:`, etc.): only
/// `-- Table: name` sections participate. For multiple files on disk, use
/// [`build_service_migration_body_from_service_dir`] and [`service_migration_is_empty`] instead.
#[must_use]
pub fn generated_tables_match_baseline(
    previous_file: &str,
    tables: &[(String, String)],
) -> bool {
    let old_sections = extract_table_sections(previous_file);
    if old_sections.len() != tables.len() {
        return false;
    }
    for (name, sql) in tables {
        let Some(old_sec) = old_sections.get(name) else {
            return false;
        };
        if normalize_table_sql_blob(old_sec) != normalize_table_sql_blob(sql) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const WIDGETS_CREATE: &str = r"CREATE TABLE IF NOT EXISTS widgets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL
);

CREATE INDEX idx_widgets_name ON widgets(name);
";

    #[test]
    fn delta_adds_column_and_index() {
        let old = format!(
            "-- Table: widgets\n{WIDGETS_CREATE}\n"
        );
        let new = r"CREATE TABLE IF NOT EXISTS widgets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    sku VARCHAR(50) NOT NULL DEFAULT ''
);

CREATE INDEX idx_widgets_name ON widgets(name);
CREATE INDEX idx_widgets_sku ON widgets(sku);
";
        let body = build_service_migration_body(Some(&old), &[("widgets".into(), new.into())]);
        assert!(body.contains("ADD COLUMN IF NOT EXISTS sku"));
        assert!(body.contains("CREATE INDEX IF NOT EXISTS idx_widgets_sku"));
        assert!(!body.contains("CREATE TABLE IF NOT EXISTS"));
    }

    #[test]
    fn no_previous_emits_full_create() {
        let new = WIDGETS_CREATE.to_string();
        let body = build_service_migration_body(None, &[("widgets".into(), new)]);
        assert!(body.contains("CREATE TABLE IF NOT EXISTS"));
    }

    #[test]
    fn identical_schema_produces_empty() {
        let s = format!("-- Table: widgets\n{WIDGETS_CREATE}\n");
        let body = build_service_migration_body(Some(&s), &[("widgets".into(), WIDGETS_CREATE.into())]);
        assert!(service_migration_is_empty(&body));
    }

    #[test]
    fn parses_inventory_style_create_block() {
        let section = r"CREATE TABLE IF NOT EXISTS categories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(50) NOT NULL UNIQUE
);

CREATE INDEX idx_categories_code ON categories(code);
COMMENT ON TABLE categories IS 'Product categories';
";
        let (table, body, tail) = extract_create_and_tail(section).expect("parse");
        assert_eq!(table, "categories");
        assert!(body.contains("id UUID"));
        assert!(tail.contains("CREATE INDEX"));
    }

    #[test]
    fn new_table_in_model_gets_full_create_when_others_exist() {
        let old = format!("-- Table: widgets\n{WIDGETS_CREATE}\n");
        let orders = r"CREATE TABLE IF NOT EXISTS orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid()
);

";
        let body = build_service_migration_body(
            Some(&old),
            &[
                ("widgets".into(), WIDGETS_CREATE.into()),
                ("orders".into(), orders.into()),
            ],
        );
        assert!(body.contains("CREATE TABLE IF NOT EXISTS orders"));
        assert!(!body.contains("ALTER TABLE widgets"));
    }

    #[test]
    fn find_latest_skips_non_numeric_prefix_files() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("20260101000002_generated_from_entities.sql");
        let bad = dir.path().join("manual_backup_generated_from_entities.sql");
        fs::write(&good, "-- x").unwrap();
        fs::write(&bad, "-- y").unwrap();
        assert_eq!(
            find_latest_generated_migration(dir.path()),
            Some(good)
        );
    }

    #[test]
    fn list_chronological_skips_non_numeric_prefix_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("20260101000001_generated_from_entities.sql"),
            "a",
        )
        .unwrap();
        fs::write(
            dir.path().join("not_a_timestamp_generated_from_entities.sql"),
            "b",
        )
        .unwrap();
        let paths = list_generated_migration_paths_chronological(dir.path());
        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn merged_chronological_full_then_alter_yields_empty_diff() {
        let dir = tempfile::tempdir().unwrap();
        let inv = dir.path().join("inventory");
        fs::create_dir_all(&inv).unwrap();
        let full = "-- Table: widgets\nCREATE TABLE IF NOT EXISTS widgets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL
);

CREATE INDEX idx_widgets_name ON widgets(name);
";
        let delta = "-- Table: widgets\nALTER TABLE widgets ADD COLUMN IF NOT EXISTS sku VARCHAR(50) NOT NULL DEFAULT '';\n\n";
        fs::write(
            inv.join("20260101000000_generated_from_entities.sql"),
            full,
        )
        .unwrap();
        fs::write(
            inv.join("20260101000001_generated_from_entities.sql"),
            delta,
        )
        .unwrap();
        let new_sql = r"CREATE TABLE IF NOT EXISTS widgets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    sku VARCHAR(50) NOT NULL DEFAULT ''
);

CREATE INDEX idx_widgets_name ON widgets(name);
";
        let body = build_service_migration_body_from_service_dir(
            &inv,
            &[("widgets".into(), new_sql.into())],
        );
        assert!(
            service_migration_is_empty(&body),
            "expected empty body, got: {body:?}"
        );
    }

    #[test]
    fn baseline_match_ignores_file_headers_and_trailing_space() {
        let prev = r"-- Migration: x
-- Version: 111
-- Generated: yesterday

-- Table: widgets
CREATE TABLE IF NOT EXISTS widgets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid()
);

";
        let new_sql = r"CREATE TABLE IF NOT EXISTS widgets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid()
);

";
        assert!(generated_tables_match_baseline(
            prev,
            &[("widgets".into(), new_sql.into())]
        ));
    }

    #[test]
    fn column_map_from_merged_baseline_merges_create_and_alter() {
        let mut parts = TableBaselineParts::default();
        parts.last_create_section = Some(
            r"CREATE TABLE IF NOT EXISTS widgets (
    id INTEGER PRIMARY KEY,
    name VARCHAR(255) NOT NULL
);"
            .to_string(),
        );
        parts.delta_section_fragments.push(
            "ALTER TABLE widgets ADD COLUMN IF NOT EXISTS sku VARCHAR(50) NOT NULL DEFAULT '';\n"
                .to_string(),
        );
        let m = column_map_from_merged_baseline(&parts);
        assert!(m.contains_key("id"));
        assert!(m.contains_key("name"));
        assert!(m.contains_key("sku"));
    }

    #[test]
    fn index_statements_for_table_from_merged_baseline_parses_create_index_lines() {
        let mut parts = TableBaselineParts::default();
        parts.last_create_section = Some(
            "CREATE TABLE IF NOT EXISTS widgets (id INT PRIMARY KEY);\n\
             CREATE INDEX idx_widgets_name ON widgets(name);\n"
                .to_string(),
        );
        let m = index_statements_for_table_from_merged_baseline(&parts, "widgets");
        assert_eq!(m.len(), 1);
        assert!(m.contains_key("idx_widgets_name"));
    }

    #[test]
    fn index_statements_merge_delta_if_not_exists_line() {
        let mut parts = TableBaselineParts::default();
        parts.last_create_section =
            Some("CREATE TABLE IF NOT EXISTS t (id INT PRIMARY KEY);\n".to_string());
        parts
            .delta_section_fragments
            .push("CREATE INDEX IF NOT EXISTS i ON t(id);\n".to_string());
        let m = index_statements_for_table_from_merged_baseline(&parts, "t");
        assert!(m.contains_key("i"));
    }
}
