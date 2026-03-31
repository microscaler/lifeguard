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
}

impl MigrationDbCompareReport {
    /// `true` when table sets differ or any shared table has column name drift.
    #[must_use]
    pub fn has_drift(&self) -> bool {
        !self.only_in_database.is_empty()
            || !self.only_in_migrations.is_empty()
            || !self.column_drifts.is_empty()
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

    Ok(MigrationDbCompareReport {
        schema: schema.to_string(),
        generated_dir: generated_dir.to_path_buf(),
        only_in_database: only_in_db,
        only_in_migrations: only_mig,
        column_drifts,
    })
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
        };
        assert!(r.has_drift());
        let s = r.to_string();
        assert!(s.contains("Column name differences"));
        assert!(s.contains("extra"));
    }
}
