//! Order SQL migrations using foreign-key edges parsed from SQL and Rust sources.

use crate::dependency_ordering::{extract_foreign_key_table, topological_sort, validate_foreign_key_references, TableInfo};
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
    let re = Regex::new(
        r"(?i)REFERENCES\s+([a-zA-Z_][a-zA-Z0-9_]*(?:\.[a-zA-Z_][a-zA-Z0-9_]*)?)\s*\(",
    )
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

/// First `CREATE TABLE IF NOT EXISTS name` in a file (PostgreSQL).
pub fn extract_created_table_from_migration_sql(sql: &str) -> Option<String> {
    let re = Regex::new(
        r"(?i)CREATE\s+TABLE\s+IF\s+NOT\s+EXISTS\s+([a-zA-Z_][a-zA-Z0-9_]*(?:\.[a-zA-Z_][a-zA-Z0-9_]*)?)\s*\(",
    )
    .expect("CREATE TABLE regex");
    re.captures(sql)
        .and_then(|c| c.get(1))
        .map(|m| normalize_sql_table_name(m.as_str()))
}

/// Tables targeted by top-level `ALTER TABLE name` (additive migrations).
pub fn extract_alter_table_targets_from_migration_sql(sql: &str) -> Vec<String> {
    let re = Regex::new(r"(?i)ALTER\s+TABLE\s+([a-zA-Z_][a-zA-Z0-9_]*(?:\.[a-zA-Z_][a-zA-Z0-9_]*)?)\s+")
        .expect("regex");
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
            .filter(|d| names.contains(d))
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

/// Build `apply_order.txt` under `migrations_dir`: one relative path per line (FK order).
///
/// Files that only `ALTER TABLE` depend on the file that `CREATE TABLE`s the same table.
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
        created: Option<String>,
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
        let created = extract_created_table_from_migration_sql(&content);
        let alters = extract_alter_table_targets_from_migration_sql(&content);
        let refs = extract_referenced_tables_from_migration_sql(&content);
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
        if let Some(ref t) = m.created {
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
        if m.created.is_none() {
            for t in &m.alters {
                if let Some(creator) = table_creator.get(t) {
                    if *creator != m.rel {
                        d.insert(creator.clone());
                    }
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
        queue.sort();
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
    writeln!(f, "# Auto-generated FK-safe apply order. Regenerate with: cargo run -p hauliage_migrator")
        .map_err(|e| e.to_string())?;
    for rel in ordered {
        writeln!(f, "{}", rel).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_parsing() {
        let sql = "job_id VARCHAR REFERENCES telemetry_locations(job_id)";
        assert_eq!(
            extract_referenced_tables_from_migration_sql(sql),
            vec!["telemetry_locations".to_string()]
        );
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
}
