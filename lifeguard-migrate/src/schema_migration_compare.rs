//! Compare **live PostgreSQL** to merged **`*_generated_from_entities.sql`** baselines.
//!
//! - **Table names:** [`accumulate_table_baselines_from_dir`] (`-- Table:` sections) vs
//!   `information_schema.tables`.
//! - **Column names:** for tables present in **both** baselines, compare `information_schema.columns`
//!   to column lines from merged `CREATE TABLE` + `ADD COLUMN` fragments (see
//!   [`crate::generated_migration_diff::column_map_from_merged_baseline`]).
//!
//! Does **not** compare SQL type text literally (PG `data_type` vs migration `INTEGER` spelling);
//! name-level reconciliation is the Phase A column diff scope.
//!
//! **Index keys + `INCLUDE` (PRD §5.7a):** for shared tables, rows from
//! [`pg_indexes`](https://www.postgresql.org/docs/current/view-pg-indexes.html) are parsed for
//! **simple** btree-style key columns and optional **`INCLUDE (…)`** column names when the
//! `indexdef` shape matches what the parser understands. Any such name missing from the merged
//! migration column baseline is reported in [`IndexColumnDrift`]. **Access method:** non-**btree**
//! indexes are reported in [`IndexAccessMethodDrift`]. **Not** compared: full `CREATE INDEX` text
//! equality, btree **opclass** variants (`jsonb_path_ops`, …), **collation**, **NULLS FIRST/LAST**,
//! or expression keys when parsing fails. See [`MigrationDbCompareReport`] and **`compare-schema`**
//! limits in `lifeguard-migrate/README.md`.

use lifeguard::LifeExecutor;
use lifeguard::LifeError;
use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use crate::generated_migration_diff::{
    accumulate_table_baselines_from_dir, column_map_from_merged_baseline,
};

/// Table names from merged `*_generated_from_entities.sql` in `dir` (from `-- Table:` headers).
#[must_use]
pub fn table_names_from_generated_migrations_dir(dir: &Path) -> BTreeSet<String> {
    accumulate_table_baselines_from_dir(dir)
        .into_keys()
        .collect()
}

/// `BASE TABLE` names in `information_schema.tables` for `schema` (e.g. `public`).
pub fn fetch_live_base_table_names(
    executor: &dyn LifeExecutor,
    schema: &str,
) -> Result<BTreeSet<String>, LifeError> {
    let sql = r"
        SELECT table_name::text
        FROM information_schema.tables
        WHERE table_schema = $1 AND table_type = 'BASE TABLE'
        ORDER BY table_name
    ";
    let rows = executor.query_all(sql, &[&schema])?;
    let mut set = BTreeSet::new();
    for row in rows {
        let name: String = row
            .try_get(0)
            .map_err(|e| LifeError::Other(format!("compare-schema table_name: {e}")))?;
        set.insert(name);
    }
    Ok(set)
}

/// Column names for one table in `information_schema.columns` (ordered for stable diffs).
pub fn fetch_live_table_column_names(
    executor: &dyn LifeExecutor,
    schema: &str,
    table: &str,
) -> Result<BTreeSet<String>, LifeError> {
    let sql = r"
        SELECT column_name::text
        FROM information_schema.columns
        WHERE table_schema = $1 AND table_name = $2
        ORDER BY ordinal_position
    ";
    let rows = executor.query_all(sql, &[&schema, &table])?;
    let mut set = BTreeSet::new();
    for row in rows {
        let name: String = row
            .try_get(0)
            .map_err(|e| LifeError::Other(format!("compare-schema column_name: {e}")))?;
        set.insert(name);
    }
    Ok(set)
}

/// One row from [`pg_indexes`](https://www.postgresql.org/docs/current/view-pg-indexes.html).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveIndexRow {
    pub table_name: String,
    pub index_name: String,
    pub indexdef: String,
}

/// Live index uses a **non-btree** access method; merged entity-driven SQL assumes **btree**-style
/// indexes only (see `lifeguard_migrate::sql_generator`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexAccessMethodDrift {
    pub table: String,
    pub index_name: String,
    /// Lowercase PostgreSQL access method (`hash`, `gin`, `gist`, …).
    pub access_method: String,
}

/// Indexes on a shared table where **parsed** btree key and/or **`INCLUDE`** column names are not
/// all present in the merged migration column map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexColumnDrift {
    pub table: String,
    pub index_name: String,
    /// Key and `INCLUDE` column names from `pg_indexes.indexdef` that are absent from the merged
    /// migration baseline (see module docs for what parsing covers).
    pub unknown_columns: Vec<String>,
}

/// Column-level drift for a single table that exists in both the live DB and merged migrations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumnDrift {
    pub table: String,
    /// Columns present in `information_schema` but not parsed from the merged baseline.
    pub only_in_database: Vec<String>,
    /// Columns parsed from the merged baseline but not in `information_schema`.
    pub only_in_migrations: Vec<String>,
}

/// Result of [`compare_generated_dir_to_live_db`].
#[derive(Debug, Clone)]
pub struct MigrationDbCompareReport {
    /// PostgreSQL schema (namespace) used for the live query.
    pub schema: String,
    /// Directory scanned for `*_generated_from_entities.sql`.
    pub generated_dir: std::path::PathBuf,
    /// Present in the database but not in any merged `-- Table:` baseline.
    pub only_in_database: Vec<String>,
    /// Present in merged migration baselines but not as a base table in the database.
    pub only_in_migrations: Vec<String>,
    /// Tables in both baselines where **column name** sets differ.
    pub column_drifts: Vec<TableColumnDrift>,
    /// Shared tables where a live index’s **parsed** key / `INCLUDE` names reference a column absent
    /// from the merged baseline.
    pub index_column_drifts: Vec<IndexColumnDrift>,
    /// Shared tables where a live index uses a non-**btree** access method (entity migrations are
    /// btree-oriented; PRD §5.7a / **T2** partial).
    pub index_access_method_drifts: Vec<IndexAccessMethodDrift>,
}

impl MigrationDbCompareReport {
    /// `true` when table sets differ, column names drift, an index’s parsed key / `INCLUDE`
    /// names reference a column missing from the merged migration map, or a live index is not
    /// **`btree`** (implicit or explicit).
    #[must_use]
    pub fn has_drift(&self) -> bool {
        !self.only_in_database.is_empty()
            || !self.only_in_migrations.is_empty()
            || !self.column_drifts.is_empty()
            || !self.index_column_drifts.is_empty()
            || !self.index_access_method_drifts.is_empty()
    }
}

/// Compare merged generated migration baselines to live `information_schema` (tables + column names).
pub fn compare_generated_dir_to_live_db(
    executor: &dyn LifeExecutor,
    schema: &str,
    generated_dir: &Path,
) -> Result<MigrationDbCompareReport, LifeError> {
    let acc = accumulate_table_baselines_from_dir(generated_dir);
    let on_disk: BTreeSet<String> = acc.keys().cloned().collect();
    let live = fetch_live_base_table_names(executor, schema)?;
    let mut only_in_db: Vec<String> = live.difference(&on_disk).cloned().collect();
    let mut only_mig: Vec<String> = on_disk.difference(&live).cloned().collect();
    only_in_db.sort();
    only_mig.sort();

    let mut column_drifts = Vec::new();
    for table in on_disk.intersection(&live) {
        let Some(parts) = acc.get(table.as_str()) else {
            continue;
        };
        let mig_map = column_map_from_merged_baseline(parts);
        let mig_names: BTreeSet<String> = mig_map.keys().cloned().collect();
        let live_names = fetch_live_table_column_names(executor, schema, table)?;
        let mut only_col_db: Vec<String> = live_names.difference(&mig_names).cloned().collect();
        let mut only_col_mig: Vec<String> = mig_names.difference(&live_names).cloned().collect();
        only_col_db.sort();
        only_col_mig.sort();
        if !only_col_db.is_empty() || !only_col_mig.is_empty() {
            column_drifts.push(TableColumnDrift {
                table: table.clone(),
                only_in_database: only_col_db,
                only_in_migrations: only_col_mig,
            });
        }
    }
    column_drifts.sort_by(|a, b| a.table.cmp(&b.table));

    let shared: BTreeSet<String> = on_disk.intersection(&live).cloned().collect();
    let index_rows = fetch_live_pg_indexes(executor, schema)?;
    let mut index_access_method_drifts = Vec::new();
    for row in &index_rows {
        if !shared.contains(&row.table_name) {
            continue;
        }
        if let Some(method) = parse_pg_indexdef_access_method(&row.indexdef) {
            if method != "btree" {
                index_access_method_drifts.push(IndexAccessMethodDrift {
                    table: row.table_name.clone(),
                    index_name: row.index_name.clone(),
                    access_method: method,
                });
            }
        }
    }
    index_access_method_drifts.sort_by(|a, b| {
        a.table
            .cmp(&b.table)
            .then_with(|| a.index_name.cmp(&b.index_name))
    });

    let mut index_column_drifts = Vec::new();
    for row in index_rows {
        if !shared.contains(&row.table_name) {
            continue;
        }
        let Some(parts) = acc.get(row.table_name.as_str()) else {
            continue;
        };
        let mig_map = column_map_from_merged_baseline(parts);
        let mig_names: BTreeSet<String> = mig_map.keys().cloned().collect();
        let Some(cols) = parse_pg_indexdef_simple_columns(&row.indexdef) else {
            continue;
        };
        let mut all_cols = cols;
        if let Some(inc) = parse_pg_indexdef_include_columns(&row.indexdef) {
            all_cols.extend(inc);
        }
        let mut unknown: Vec<String> = all_cols
            .into_iter()
            .filter(|c| !mig_names.contains(c))
            .collect();
        if unknown.is_empty() {
            continue;
        }
        unknown.sort();
        index_column_drifts.push(IndexColumnDrift {
            table: row.table_name,
            index_name: row.index_name,
            unknown_columns: unknown,
        });
    }
    index_column_drifts.sort_by(|a, b| {
        a.table
            .cmp(&b.table)
            .then_with(|| a.index_name.cmp(&b.index_name))
    });

    Ok(MigrationDbCompareReport {
        schema: schema.to_string(),
        generated_dir: generated_dir.to_path_buf(),
        only_in_database: only_in_db,
        only_in_migrations: only_mig,
        column_drifts,
        index_column_drifts,
        index_access_method_drifts,
    })
}

/// Non-primary indexes in `schema` from `pg_indexes` (includes unique indexes; expression indexes kept for parse skip).
pub fn fetch_live_pg_indexes(
    executor: &dyn LifeExecutor,
    schema: &str,
) -> Result<Vec<LiveIndexRow>, LifeError> {
    let sql = r"
        SELECT tablename::text, indexname::text, indexdef::text
        FROM pg_indexes
        WHERE schemaname = $1
        ORDER BY tablename, indexname
    ";
    let rows = executor.query_all(sql, &[&schema])?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let table_name: String = row
            .try_get(0)
            .map_err(|e| LifeError::Other(format!("compare-schema pg_indexes tablename: {e}")))?;
        let index_name: String = row
            .try_get(1)
            .map_err(|e| LifeError::Other(format!("compare-schema pg_indexes indexname: {e}")))?;
        let indexdef: String = row
            .try_get(2)
            .map_err(|e| LifeError::Other(format!("compare-schema pg_indexes indexdef: {e}")))?;
        if index_name.ends_with("_pkey") {
            continue;
        }
        out.push(LiveIndexRow {
            table_name,
            index_name,
            indexdef,
        });
    }
    Ok(out)
}

fn after_on_clause(def: &str) -> Option<&str> {
    let lower = def.to_ascii_lowercase();
    let i = lower.find(" on ")? + 4;
    Some(def[i..].trim_start())
}

fn skip_qualified_table(s: &str) -> Option<&str> {
    let mut s = s.trim_start();
    loop {
        if s.is_empty() {
            return None;
        }
        if s.starts_with('"') {
            let rest = &s[1..];
            let end = rest.find('"')?;
            s = rest[end + 1..].trim_start();
        } else {
            let mut cut = s.len();
            let mut broke = false;
            for (idx, ch) in s.char_indices() {
                if ch.is_whitespace() || ch == '(' {
                    cut = idx;
                    broke = true;
                    break;
                }
                if ch == '.' {
                    cut = idx + 1;
                    broke = true;
                    break;
                }
            }
            if !broke {
                cut = s.len();
            }
            if cut == 0 {
                return None;
            }
            if s.as_bytes().get(cut.saturating_sub(1)) == Some(&b'.') {
                s = s[cut..].trim_start();
                continue;
            }
            s = s[cut..].trim_start();
            break;
        }
        if s.starts_with('.') {
            s = s[1..].trim_start();
            continue;
        }
        break;
    }
    Some(s)
}

fn skip_using_method(s: &str) -> Option<&str> {
    let s = s.trim_start();
    if s.len() >= 6 && s[..6].eq_ignore_ascii_case("using ") {
        let rest = s[6..].trim_start();
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '(')
            .unwrap_or(rest.len());
        Some(rest[end..].trim_start())
    } else {
        Some(s)
    }
}

/// PostgreSQL access method for `CREATE INDEX` / `pg_indexes.indexdef`: `btree` (implicit when the
/// key list follows the table name directly), or the first identifier after **`USING`** (`hash`,
/// `gin`, `gist`, …). Returns [`None`] if the `ON …` tail cannot be interpreted.
#[must_use]
pub fn parse_pg_indexdef_access_method(indexdef: &str) -> Option<String> {
    let mut tail = after_on_clause(indexdef)?;
    if tail.len() >= 5 && tail[..5].eq_ignore_ascii_case("only ") {
        tail = tail[5..].trim_start();
    }
    tail = skip_qualified_table(tail)?;
    let tail = tail.trim_start();
    if tail.starts_with('(') {
        return Some("btree".to_string());
    }
    if tail.len() >= 6 && tail[..6].eq_ignore_ascii_case("using ") {
        let rest = tail[6..].trim_start();
        let first = rest.split_whitespace().next()?;
        let method = if let Some(dot) = first.rfind('.') {
            &first[dot + 1..]
        } else {
            first
        };
        return Some(method.to_ascii_lowercase());
    }
    None
}

fn balanced_paren_group(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if !s.starts_with('(') {
        return None;
    }
    let mut depth = 0i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&s[1..i], &s[i + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

fn simple_key_columns_from_inner(inner: &str) -> Option<Vec<String>> {
    let inner = inner.trim();
    if inner.is_empty() {
        return None;
    }
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut cols = Vec::new();
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                if let Some(c) = first_simple_index_column(&inner[start..i]) {
                    cols.push(c);
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    if let Some(c) = first_simple_index_column(&inner[start..]) {
        cols.push(c);
    }
    if cols.is_empty() {
        None
    } else {
        Some(cols)
    }
}

fn first_simple_index_column(seg: &str) -> Option<String> {
    let seg = seg.trim();
    if seg.is_empty() {
        return None;
    }
    if seg.starts_with('(') {
        return None;
    }
    if seg.contains('(') && !seg.starts_with('"') {
        return None;
    }
    let lower = seg.to_ascii_lowercase();
    if let Some(pos) = lower.find(" collate ") {
        return first_simple_index_column(&seg[..pos]);
    }
    let seg = seg
        .split_whitespace()
        .next()
        .unwrap_or(seg)
        .trim_end_matches(',');
    if seg.starts_with('(') {
        return None;
    }
    if seg.starts_with('"') {
        let rest = &seg[1..];
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }
    Some(seg.trim_matches('"').to_string())
}

/// Parse simple btree-style index key columns from `pg_indexes.indexdef`. Returns `None` for
/// expression indexes or unrecognised shapes.
#[must_use]
pub fn parse_pg_indexdef_simple_columns(indexdef: &str) -> Option<Vec<String>> {
    let mut tail = after_on_clause(indexdef)?;
    if tail.len() >= 5 && tail[..5].eq_ignore_ascii_case("only ") {
        tail = tail[5..].trim_start();
    }
    tail = skip_qualified_table(tail)?;
    tail = skip_using_method(tail)?;
    let (key_inner, _rest) = balanced_paren_group(tail)?;
    simple_key_columns_from_inner(key_inner)
}

/// PostgreSQL **`INCLUDE`** column list after the btree key, if present (`INCLUDE (a, b)`).
#[must_use]
pub fn parse_pg_indexdef_include_columns(indexdef: &str) -> Option<Vec<String>> {
    let mut tail = after_on_clause(indexdef)?;
    if tail.len() >= 5 && tail[..5].eq_ignore_ascii_case("only ") {
        tail = tail[5..].trim_start();
    }
    tail = skip_qualified_table(tail)?;
    tail = skip_using_method(tail)?;
    let (_, rest) = balanced_paren_group(tail)?;
    let mut r = rest.trim_start();
    if r.len() < 8 || !r[..8].eq_ignore_ascii_case("include ") {
        return Some(Vec::new());
    }
    r = r[8..].trim_start();
    let (inc_inner, _) = balanced_paren_group(r)?;
    simple_key_columns_from_inner(inc_inner)
}

impl fmt::Display for MigrationDbCompareReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Schema/migration reconciliation (PostgreSQL schema `{}`)",
            self.schema
        )?;
        writeln!(
            f,
            "  Generated migrations dir: {}",
            self.generated_dir.display()
        )?;
        writeln!(f)?;
        if !self.has_drift() {
            writeln!(
                f,
                "  No drift: table names align, and column names match for tables present in both the database and merged migration baselines."
            )?;
            return Ok(());
        }
        if !self.only_in_database.is_empty() {
            writeln!(
                f,
                "  Tables only in database (not in merged migration baseline):"
            )?;
            for t in &self.only_in_database {
                writeln!(f, "    - {t}")?;
            }
            writeln!(f)?;
        }
        if !self.only_in_migrations.is_empty() {
            writeln!(
                f,
                "  Tables only in merged migration files (not in live DB):"
            )?;
            for t in &self.only_in_migrations {
                writeln!(f, "    - {t}")?;
            }
            writeln!(f)?;
        }
        if !self.column_drifts.is_empty() {
            writeln!(
                f,
                "  Column name differences (tables in both live DB and merged migrations):"
            )?;
            for d in &self.column_drifts {
                writeln!(f, "    Table `{}`:", d.table)?;
                if !d.only_in_database.is_empty() {
                    writeln!(
                        f,
                        "      Columns only in database (not in merged baseline): {}",
                        d.only_in_database.join(", ")
                    )?;
                }
                if !d.only_in_migrations.is_empty() {
                    writeln!(
                        f,
                        "      Columns only in merged baseline (not in database): {}",
                        d.only_in_migrations.join(", ")
                    )?;
                }
            }
        }
        if !self.index_column_drifts.is_empty() {
            writeln!(
                f,
                "  Index key / INCLUDE columns not in merged migration baseline (shared tables only; primary key indexes skipped):"
            )?;
            for d in &self.index_column_drifts {
                writeln!(
                    f,
                    "    Table `{}` index `{}`: unknown columns: {}",
                    d.table,
                    d.index_name,
                    d.unknown_columns.join(", ")
                )?;
            }
        }
        if !self.index_access_method_drifts.is_empty() {
            writeln!(
                f,
                "  Index access method not btree (entity migrations assume btree; shared tables only; primary key indexes skipped):"
            )?;
            for d in &self.index_access_method_drifts {
                writeln!(
                    f,
                    "    Table `{}` index `{}`: USING {}",
                    d.table, d.index_name, d.access_method
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_display_ok_when_no_drift() {
        let r = MigrationDbCompareReport {
            schema: "public".into(),
            generated_dir: Path::new("/tmp/x").to_path_buf(),
            only_in_database: vec![],
            only_in_migrations: vec![],
            column_drifts: vec![],
            index_column_drifts: vec![],
            index_access_method_drifts: vec![],
        };
        assert!(!r.has_drift());
        let s = r.to_string();
        assert!(s.contains("No drift"));
    }

    #[test]
    fn has_drift_when_only_in_db() {
        let r = MigrationDbCompareReport {
            schema: "public".into(),
            generated_dir: Path::new("/x").to_path_buf(),
            only_in_database: vec!["orphan".into()],
            only_in_migrations: vec![],
            column_drifts: vec![],
            index_column_drifts: vec![],
            index_access_method_drifts: vec![],
        };
        assert!(r.has_drift());
    }

    #[test]
    fn has_drift_when_column_drift_only() {
        let r = MigrationDbCompareReport {
            schema: "public".into(),
            generated_dir: Path::new("/x").to_path_buf(),
            only_in_database: vec![],
            only_in_migrations: vec![],
            column_drifts: vec![TableColumnDrift {
                table: "t".into(),
                only_in_database: vec!["extra".into()],
                only_in_migrations: vec![],
            }],
            index_column_drifts: vec![],
            index_access_method_drifts: vec![],
        };
        assert!(r.has_drift());
        let s = r.to_string();
        assert!(s.contains("Column name differences"));
        assert!(s.contains("extra"));
    }

    #[test]
    fn has_drift_when_index_unknown_column() {
        let r = MigrationDbCompareReport {
            schema: "public".into(),
            generated_dir: Path::new("/x").to_path_buf(),
            only_in_database: vec![],
            only_in_migrations: vec![],
            column_drifts: vec![],
            index_column_drifts: vec![IndexColumnDrift {
                table: "t".into(),
                index_name: "ix".into(),
                unknown_columns: vec!["ghost".into()],
            }],
            index_access_method_drifts: vec![],
        };
        assert!(r.has_drift());
        assert!(r.to_string().contains("Index key / INCLUDE columns"));
    }

    #[test]
    fn parse_pg_indexdef_simple_columns_examples() {
        let def = "CREATE INDEX ix ON public.widgets USING btree (id)";
        assert_eq!(
            parse_pg_indexdef_simple_columns(def),
            Some(vec!["id".to_string()])
        );
        let def2 = "CREATE UNIQUE INDEX u ON ONLY myschema.items USING hash (a, b)";
        assert_eq!(
            parse_pg_indexdef_simple_columns(def2),
            Some(vec!["a".to_string(), "b".to_string()])
        );
        assert!(parse_pg_indexdef_simple_columns("CREATE INDEX x ON t (lower(y))").is_none());
    }

    #[test]
    fn parse_pg_indexdef_include_columns_examples() {
        let def = "CREATE INDEX ix ON public.widgets USING btree (id) INCLUDE (name, sku)";
        assert_eq!(
            parse_pg_indexdef_include_columns(def),
            Some(vec!["name".to_string(), "sku".to_string()])
        );
        assert_eq!(
            parse_pg_indexdef_include_columns("CREATE INDEX ix ON t (id)"),
            Some(vec![])
        );
    }

    #[test]
    fn parse_pg_indexdef_access_method_examples() {
        assert_eq!(
            parse_pg_indexdef_access_method("CREATE INDEX ix ON public.widgets USING btree (id)"),
            Some("btree".to_string())
        );
        assert_eq!(
            parse_pg_indexdef_access_method("CREATE INDEX ix ON t (id)"),
            Some("btree".to_string())
        );
        assert_eq!(
            parse_pg_indexdef_access_method(
                "CREATE UNIQUE INDEX u ON ONLY myschema.items USING hash (a, b)"
            ),
            Some("hash".to_string())
        );
        assert_eq!(
            parse_pg_indexdef_access_method("CREATE INDEX ix ON t USING gin (j)"),
            Some("gin".to_string())
        );
    }

    #[test]
    fn has_drift_when_index_access_method_not_btree() {
        let r = MigrationDbCompareReport {
            schema: "public".into(),
            generated_dir: Path::new("/x").to_path_buf(),
            only_in_database: vec![],
            only_in_migrations: vec![],
            column_drifts: vec![],
            index_column_drifts: vec![],
            index_access_method_drifts: vec![IndexAccessMethodDrift {
                table: "t".into(),
                index_name: "ix".into(),
                access_method: "hash".into(),
            }],
        };
        assert!(r.has_drift());
        assert!(r.to_string().contains("access method not btree"));
        assert!(r.to_string().contains("USING hash"));
    }
}
