//! Delta SQL for entity-driven migrations.
//!
//! When a prior `*_generated_from_entities.sql` exists in the service directory, new runs
//! compare the previous `CREATE TABLE` column lists to the current generator output and emit
//! **`ALTER TABLE ... ADD COLUMN IF NOT EXISTS`** (and new index lines) instead of duplicating
//! full `CREATE TABLE` bodies for unchanged tables.

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
        let ts: u64 = prefix.parse().ok()?;
        match best {
            None => best = Some((ts, ent.path())),
            Some((t0, _)) if ts > t0 => best = Some((ts, ent.path())),
            _ => {}
        }
    }
    best.map(|(_, p)| p)
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
#[must_use]
pub fn build_service_migration_body(
    previous_file: Option<&str>,
    tables: &[(String, String)],
) -> String {
    let old_sections = previous_file
        .map(extract_table_sections)
        .unwrap_or_default();

    let mut out = String::new();

    for (table_name, new_sql) in tables {
        if let Some(old_sec) = old_sections.get(table_name) {
            let Some((_, old_body, old_tail)) = extract_create_and_tail(old_sec) else {
                out.push_str(&format!("-- Table: {table_name}\n{new_sql}\n\n"));
                continue;
            };
            let Some((_, new_body, new_tail)) = extract_create_and_tail(new_sql) else {
                out.push_str(&format!("-- Table: {table_name}\n{new_sql}\n\n"));
                continue;
            };

            let old_cols = parse_column_defs_from_create_body(&old_body);
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

            let index_and_more = tail_lines_not_in_old(old_sec, &new_tail);
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
        } else {
            out.push_str(&format!("-- Table: {table_name}\n{new_sql}\n\n"));
        }
    }

    out
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

/// True when each entity-generated table SQL matches the same table section in the latest
/// migration file. Ignores file-level headers (`-- Version:`, `-- Generated:`, etc.): only
/// `-- Table: name` sections participate. Use this as a final guard so a new timestamped file
/// is not written when the only drift is metadata or a diff false-positive.
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
}
