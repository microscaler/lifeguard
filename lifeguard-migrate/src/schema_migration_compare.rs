//! Compare **live PostgreSQL** table names to merged **`*_generated_from_entities.sql`** baselines.
//!
//! Uses [`crate::generated_migration_diff::accumulate_table_baselines_from_dir`] (`-- Table:` sections)
//! vs `information_schema.tables` for DBA confidence (PRD Phase A).

use lifeguard::LifeExecutor;
use lifeguard::LifeError;
use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

use crate::generated_migration_diff::accumulate_table_baselines_from_dir;

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
}

impl MigrationDbCompareReport {
    /// `true` when the two sets differ (either direction).
    #[must_use]
    pub fn has_drift(&self) -> bool {
        !self.only_in_database.is_empty() || !self.only_in_migrations.is_empty()
    }
}

/// Compare merged generated migration table names to live `information_schema` base tables.
pub fn compare_generated_dir_to_live_db(
    executor: &dyn LifeExecutor,
    schema: &str,
    generated_dir: &Path,
) -> Result<MigrationDbCompareReport, LifeError> {
    let on_disk = table_names_from_generated_migrations_dir(generated_dir);
    let live = fetch_live_base_table_names(executor, schema)?;
    let mut only_in_db: Vec<String> = live.difference(&on_disk).cloned().collect();
    let mut only_mig: Vec<String> = on_disk.difference(&live).cloned().collect();
    only_in_db.sort();
    only_mig.sort();
    Ok(MigrationDbCompareReport {
        schema: schema.to_string(),
        generated_dir: generated_dir.to_path_buf(),
        only_in_database: only_in_db,
        only_in_migrations: only_mig,
    })
}

impl fmt::Display for MigrationDbCompareReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Schema/migration table reconciliation (PostgreSQL schema `{}`)",
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
                "  Tables match: live DB and merged `*_generated_from_entities.sql` baselines list the same table names."
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
        }
        if !self.only_in_migrations.is_empty() {
            writeln!(
                f,
                "  Tables only in merged migration files (not in live DB):"
            )?;
            for t in &self.only_in_migrations {
                writeln!(f, "    - {t}")?;
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
        };
        assert!(!r.has_drift());
        let s = r.to_string();
        assert!(s.contains("Tables match"));
    }

    #[test]
    fn has_drift_when_only_in_db() {
        let r = MigrationDbCompareReport {
            schema: "public".into(),
            generated_dir: Path::new("/x").to_path_buf(),
            only_in_database: vec!["orphan".into()],
            only_in_migrations: vec![],
        };
        assert!(r.has_drift());
    }
}
