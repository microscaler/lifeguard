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
//! migration column baseline is reported in [`IndexColumnDrift`], **unless** the same index name has
//! a `CREATE INDEX` line in the merged baseline — then [`IndexDefinitionTextDrift`] is used instead
//! of column-level drift for that index. **Access method:** non-**btree** indexes are reported in
//! [`IndexAccessMethodDrift`]. **T1:** when both sides name the index, [`normalize_index_statement_for_compare`]
//! compares normalized `CREATE INDEX` text (whitespace, `IF NOT EXISTS`, optional explicit **`USING btree`**).
//! **T2b (partial):** [`fetch_live_btree_index_key_opclasses`] reads **`pg_index` / `pg_opclass`**
//! for **btree** indexes and flags keys whose opclass is not the type’s default (`opcdefault`).
//! [`MigrationDbCompareReport::index_btree_nondefault_opclass_drifts`] lists those on **shared**
//! tables (primary-key indexes excluded). **T3 (partial):** when merged migration SQL lists only
//! **simple** btree key columns for an index name but **`pg_index.indkey`** has an **expression**
//! slot (`0`), [`MigrationDbCompareReport::index_expression_key_vs_simple_migration_drifts`]
//! reports structured drift (and **T1** text drift is **not** emitted for that index). **Not** fully
//! normalized: **collation**, **NULLS FIRST/LAST**, expression-key opclasses vs type defaults, or
//! hand-written migration opclasses vs live. See [`MigrationDbCompareReport`] and **`compare-schema`**
//! limits in `lifeguard-migrate/README.md`.

use lifeguard::LifeExecutor;
use lifeguard::LifeError;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use crate::generated_migration_diff::{
    accumulate_table_baselines_from_dir, column_map_from_merged_baseline,
    index_statements_for_table_from_merged_baseline,
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

/// Shared table + index name where normalized migration `CREATE INDEX` text ≠ live `pg_indexes.indexdef`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDefinitionTextDrift {
    pub table: String,
    pub index_name: String,
    pub normalized_migration: String,
    pub normalized_live: String,
}

/// Live index (non–primary-key) on a shared table with no matching `CREATE INDEX` line in the merged
/// baseline (and not solely explained by [`IndexColumnDrift`] or [`IndexAccessMethodDrift`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexOnlyInDatabaseDrift {
    pub table: String,
    pub index_name: String,
}

/// Index declared in merged migration text for a shared table but missing from live `pg_indexes`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexOnlyInMigrationDrift {
    pub table: String,
    pub index_name: String,
}

/// Live btree index uses at least one **expression** key (`pg_index.indkey` = `0`), but the merged
/// baseline’s `CREATE INDEX` line parses as **simple column** keys only (**T3** catalog vs parse).
///
/// For the same index, **T1** normalized text drift is suppressed in favor of this row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexExpressionKeyVsSimpleMigrationDrift {
    pub table: String,
    pub index_name: String,
    /// 1-based btree key ordinals where the live catalog reports an expression key.
    pub expression_key_ordinals: Vec<i32>,
    /// `pg_get_indexdef(index_oid, ordinal, false)` for each expression slot (same order as ordinals).
    pub live_expression_key_defs: Vec<String>,
    /// Column names from [`parse_pg_indexdef_simple_columns`] on the merged migration statement.
    pub migration_simple_key_columns: Vec<String>,
}

/// Btree index key uses a **non-default** operator class for the underlying column type (catalog **T2b**).
///
/// Expression keys (`pg_index` `indkey` slot `0`) are not reported here. If PostgreSQL has no
/// default btree opclass for the type, no drift is emitted for that key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexBtreeNonDefaultOpclassDrift {
    pub table: String,
    pub index_name: String,
    /// 1-based key column position in the btree index.
    pub key_ordinal: i32,
    pub column_name: Option<String>,
    /// Live opclass name (e.g. `text_pattern_ops` on btree `text`; `jsonb_path_ops` is GIN-only).
    pub opclass_name: String,
    /// Default btree opclass name for the column type when resolved.
    pub default_opclass_name: Option<String>,
}

/// One btree **key** column slot from [`fetch_live_btree_index_key_opclasses`] (catalog proof / tooling).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveBtreeIndexKeyOpclassRow {
    pub table_name: String,
    pub index_name: String,
    pub key_ordinal: i32,
    pub column_name: Option<String>,
    pub opclass_name: String,
    pub access_method: String,
    pub default_opclass_name: Option<String>,
    pub is_non_default_opclass: bool,
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
    /// **T3 (partial):** live btree index has expression key(s); merged migration lists simple columns only.
    pub index_expression_key_vs_simple_migration_drifts: Vec<IndexExpressionKeyVsSimpleMigrationDrift>,
    /// **T1:** normalized full `CREATE INDEX` text differs for the same index name.
    pub index_definition_text_drifts: Vec<IndexDefinitionTextDrift>,
    pub index_only_in_database: Vec<IndexOnlyInDatabaseDrift>,
    pub index_only_in_migration: Vec<IndexOnlyInMigrationDrift>,
    /// **T2b:** btree key uses non-default opclass for the column type (`pg_opclass` / `opcdefault`).
    pub index_btree_nondefault_opclass_drifts: Vec<IndexBtreeNonDefaultOpclassDrift>,
}

impl MigrationDbCompareReport {
    /// `true` when table sets differ, column names drift, an index’s parsed key / `INCLUDE`
    /// names reference a column missing from the merged migration map, a live index is not
    /// **`btree`** (implicit or explicit), **T1** / only-in-one-side index names differ, a btree
    /// key uses a **non-default** opclass (**T2b** catalog), or **T3** expression vs simple migration keys.
    #[must_use]
    pub fn has_drift(&self) -> bool {
        !self.only_in_database.is_empty()
            || !self.only_in_migrations.is_empty()
            || !self.column_drifts.is_empty()
            || !self.index_column_drifts.is_empty()
            || !self.index_access_method_drifts.is_empty()
            || !self.index_expression_key_vs_simple_migration_drifts.is_empty()
            || !self.index_definition_text_drifts.is_empty()
            || !self.index_only_in_database.is_empty()
            || !self.index_only_in_migration.is_empty()
            || !self.index_btree_nondefault_opclass_drifts.is_empty()
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

    let non_btree_keys: BTreeSet<(String, String)> = index_access_method_drifts
        .iter()
        .map(|d| (d.table.clone(), d.index_name.clone()))
        .collect();

    let expr_key_rows = fetch_live_btree_expression_index_key_slots(executor, schema)?;
    let mut expr_slots_by_index: BTreeMap<(String, String), Vec<(i32, String)>> = BTreeMap::new();
    for er in expr_key_rows {
        if !shared.contains(&er.table_name) {
            continue;
        }
        expr_slots_by_index
            .entry((er.table_name, er.index_name))
            .or_default()
            .push((er.key_ordinal, er.key_def));
    }
    for slots in expr_slots_by_index.values_mut() {
        slots.sort_by(|a, b| a.0.cmp(&b.0));
    }

    let mut index_definition_text_drifts = Vec::new();
    let mut index_expression_key_vs_simple_migration_drifts = Vec::new();
    let mut index_column_drifts = Vec::new();
    let mut index_only_in_database = Vec::new();

    for row in &index_rows {
        if !shared.contains(&row.table_name) {
            continue;
        }
        let Some(parts) = acc.get(row.table_name.as_str()) else {
            continue;
        };
        let index_by_name =
            index_statements_for_table_from_merged_baseline(parts, &row.table_name);
        let mig_stmt = index_by_name.get(&row.index_name);

        if let Some(mig) = mig_stmt {
            let idx_key = (row.table_name.clone(), row.index_name.clone());
            if let Some(slots) = expr_slots_by_index.get(&idx_key) {
                if !slots.is_empty() {
                    if let Some(mig_cols) = parse_pg_indexdef_simple_columns(mig) {
                        let ordinals: Vec<i32> = slots.iter().map(|(o, _)| *o).collect();
                        let defs: Vec<String> = slots.iter().map(|(_, d)| d.clone()).collect();
                        index_expression_key_vs_simple_migration_drifts.push(
                            IndexExpressionKeyVsSimpleMigrationDrift {
                                table: row.table_name.clone(),
                                index_name: row.index_name.clone(),
                                expression_key_ordinals: ordinals,
                                live_expression_key_defs: defs,
                                migration_simple_key_columns: mig_cols,
                            },
                        );
                        continue;
                    }
                }
            }
            let nm = normalize_index_statement_for_compare(mig);
            let nl = normalize_index_statement_for_compare(&row.indexdef);
            if nm != nl {
                index_definition_text_drifts.push(IndexDefinitionTextDrift {
                    table: row.table_name.clone(),
                    index_name: row.index_name.clone(),
                    normalized_migration: nm,
                    normalized_live: nl,
                });
            }
            continue;
        }

        let mig_map = column_map_from_merged_baseline(parts);
        let mig_names: BTreeSet<String> = mig_map.keys().cloned().collect();
        let Some(cols) = parse_pg_indexdef_simple_columns(&row.indexdef) else {
            let key = (row.table_name.clone(), row.index_name.clone());
            if !non_btree_keys.contains(&key) {
                index_only_in_database.push(IndexOnlyInDatabaseDrift {
                    table: row.table_name.clone(),
                    index_name: row.index_name.clone(),
                });
            }
            continue;
        };
        let mut all_cols = cols;
        if let Some(inc) = parse_pg_indexdef_include_columns(&row.indexdef) {
            all_cols.extend(inc);
        }
        let mut unknown: Vec<String> = all_cols
            .iter()
            .filter(|c| !mig_names.contains(*c))
            .cloned()
            .collect();
        unknown.sort();
        if !unknown.is_empty() {
            index_column_drifts.push(IndexColumnDrift {
                table: row.table_name.clone(),
                index_name: row.index_name.clone(),
                unknown_columns: unknown,
            });
            continue;
        }
        let key = (row.table_name.clone(), row.index_name.clone());
        if !non_btree_keys.contains(&key) {
            index_only_in_database.push(IndexOnlyInDatabaseDrift {
                table: row.table_name.clone(),
                index_name: row.index_name.clone(),
            });
        }
    }

    index_definition_text_drifts.sort_by(|a, b| {
        a.table
            .cmp(&b.table)
            .then_with(|| a.index_name.cmp(&b.index_name))
    });
    index_expression_key_vs_simple_migration_drifts.sort_by(|a, b| {
        a.table
            .cmp(&b.table)
            .then_with(|| a.index_name.cmp(&b.index_name))
    });
    index_column_drifts.sort_by(|a, b| {
        a.table
            .cmp(&b.table)
            .then_with(|| a.index_name.cmp(&b.index_name))
    });
    index_only_in_database.sort_by(|a, b| {
        a.table
            .cmp(&b.table)
            .then_with(|| a.index_name.cmp(&b.index_name))
    });

    let mut index_only_in_migration = Vec::new();
    for table in &shared {
        let Some(parts) = acc.get(table.as_str()) else {
            continue;
        };
        let mig_indexes = index_statements_for_table_from_merged_baseline(parts, table);
        let live_names: BTreeSet<String> = index_rows
            .iter()
            .filter(|r| r.table_name == *table)
            .map(|r| r.index_name.clone())
            .collect();
        for name in mig_indexes.keys() {
            if !live_names.contains(name) {
                index_only_in_migration.push(IndexOnlyInMigrationDrift {
                    table: table.clone(),
                    index_name: name.clone(),
                });
            }
        }
    }
    index_only_in_migration.sort_by(|a, b| {
        a.table
            .cmp(&b.table)
            .then_with(|| a.index_name.cmp(&b.index_name))
    });

    let opclass_rows = fetch_live_btree_index_key_opclasses(executor, schema)?;
    let mut index_btree_nondefault_opclass_drifts = Vec::new();
    for o in opclass_rows {
        if !o.is_non_default_opclass || !shared.contains(&o.table_name) {
            continue;
        }
        index_btree_nondefault_opclass_drifts.push(IndexBtreeNonDefaultOpclassDrift {
            table: o.table_name,
            index_name: o.index_name,
            key_ordinal: o.key_ordinal,
            column_name: o.column_name,
            opclass_name: o.opclass_name,
            default_opclass_name: o.default_opclass_name,
        });
    }
    index_btree_nondefault_opclass_drifts.sort_by(|a, b| {
        a.table
            .cmp(&b.table)
            .then_with(|| a.index_name.cmp(&b.index_name))
            .then_with(|| a.key_ordinal.cmp(&b.key_ordinal))
    });

    Ok(MigrationDbCompareReport {
        schema: schema.to_string(),
        generated_dir: generated_dir.to_path_buf(),
        only_in_database: only_in_db,
        only_in_migrations: only_mig,
        column_drifts,
        index_column_drifts,
        index_access_method_drifts,
        index_expression_key_vs_simple_migration_drifts,
        index_definition_text_drifts,
        index_only_in_database,
        index_only_in_migration,
        index_btree_nondefault_opclass_drifts,
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

/// Btree index **key** slots with operator class names from **`pg_index` / `pg_opclass`** (catalog **T2b** proof).
///
/// Restricted to **`pg_am.amname = 'btree'`**, valid indexes (`indisvalid`), non-primary (`NOT indisprimary`),
/// base tables in `schema`. Uses `indkey::int2[]` / `indclass::oid[]` (PostgreSQL 12+). Expression keys
/// (`indkey` slot `0`) return `column_name = None` and `is_non_default_opclass = false`.
#[must_use]
pub fn fetch_live_btree_index_key_opclasses(
    executor: &dyn LifeExecutor,
    schema: &str,
) -> Result<Vec<LiveBtreeIndexKeyOpclassRow>, LifeError> {
    let sql = r"
        SELECT
            t.relname::text AS tablename,
            ic.relname::text AS indexname,
            s.k::int AS key_ord,
            CASE
                WHEN (xi.indkey::int2[])[s.k] = 0::int2 THEN NULL
                ELSE a.attname::text
            END AS column_name,
            opc.opcname::text AS opclass_name,
            am.amname::text AS access_method,
            defopc.opcname::text AS default_opclass_name,
            CASE
                WHEN (xi.indkey::int2[])[s.k] = 0::int2 THEN false
                WHEN defopc.oid IS NULL THEN false
                ELSE opc.oid IS DISTINCT FROM defopc.oid
            END AS is_non_default
        FROM pg_index xi
        JOIN pg_class ic ON ic.oid = xi.indexrelid
        JOIN pg_class t ON t.oid = xi.indrelid
        JOIN pg_namespace n ON n.oid = t.relnamespace
        JOIN pg_am am ON am.oid = ic.relam
        CROSS JOIN LATERAL generate_subscripts(xi.indkey::int2[], 1) AS s(k)
        JOIN pg_opclass opc ON opc.oid = (xi.indclass::oid[])[s.k]
        LEFT JOIN pg_attribute a
            ON a.attrelid = xi.indrelid
            AND a.attnum = (xi.indkey::int2[])[s.k]
            AND (xi.indkey::int2[])[s.k] <> 0::int2
        LEFT JOIN LATERAL (
            SELECT oc.oid, oc.opcname
            FROM pg_opclass oc
            WHERE a.atttypid IS NOT NULL
                AND oc.opcintype = a.atttypid
                AND oc.opcmethod = (SELECT oid FROM pg_am WHERE amname = 'btree')
                AND oc.opcdefault
            LIMIT 1
        ) defopc ON true
        WHERE n.nspname = $1
            AND t.relkind IN ('r', 'p')
            AND ic.relkind = 'i'
            AND am.amname = 'btree'
            AND xi.indisvalid
            AND NOT xi.indisprimary
        ORDER BY t.relname, ic.relname, s.k
    ";
    let rows = executor.query_all(sql, &[&schema])?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let table_name: String = row.try_get(0).map_err(|e| {
            LifeError::Other(format!("compare-schema btree opclass tablename: {e}"))
        })?;
        let index_name: String = row.try_get(1).map_err(|e| {
            LifeError::Other(format!("compare-schema btree opclass indexname: {e}"))
        })?;
        let key_ordinal: i32 = row.try_get(2).map_err(|e| {
            LifeError::Other(format!("compare-schema btree opclass key_ord: {e}"))
        })?;
        let column_name: Option<String> = row.try_get(3).map_err(|e| {
            LifeError::Other(format!("compare-schema btree opclass column_name: {e}"))
        })?;
        let opclass_name: String = row.try_get(4).map_err(|e| {
            LifeError::Other(format!("compare-schema btree opclass opcname: {e}"))
        })?;
        let access_method: String = row.try_get(5).map_err(|e| {
            LifeError::Other(format!("compare-schema btree opclass amname: {e}"))
        })?;
        let default_opclass_name: Option<String> = row.try_get(6).map_err(|e| {
            LifeError::Other(format!("compare-schema btree opclass default_opcname: {e}"))
        })?;
        let is_non_default_opclass: bool = row.try_get(7).map_err(|e| {
            LifeError::Other(format!("compare-schema btree opclass is_non_default: {e}"))
        })?;
        out.push(LiveBtreeIndexKeyOpclassRow {
            table_name,
            index_name,
            key_ordinal,
            column_name,
            opclass_name,
            access_method,
            default_opclass_name,
            is_non_default_opclass,
        });
    }
    Ok(out)
}

/// One **expression** btree key slot (`pg_index.indkey` = `0`) from [`fetch_live_btree_expression_index_key_slots`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveBtreeExpressionKeyRow {
    pub table_name: String,
    pub index_name: String,
    pub key_ordinal: i32,
    pub key_def: String,
}

/// Btree index key slots where **`pg_index.indkey`** is zero — i.e. **expression** keys (catalog **T3**).
///
/// Uses `pg_get_indexdef(index_oid, key_ord, false)` for each slot. Restricted like
/// [`fetch_live_btree_index_key_opclasses`]: valid non-primary btree indexes on base tables in `schema`.
#[must_use]
pub fn fetch_live_btree_expression_index_key_slots(
    executor: &dyn LifeExecutor,
    schema: &str,
) -> Result<Vec<LiveBtreeExpressionKeyRow>, LifeError> {
    let sql = r"
        SELECT
            t.relname::text AS tablename,
            ic.relname::text AS indexname,
            s.k::int AS key_ord,
            pg_get_indexdef(ic.oid, s.k, false)::text AS key_def
        FROM pg_index xi
        JOIN pg_class ic ON ic.oid = xi.indexrelid
        JOIN pg_class t ON t.oid = xi.indrelid
        JOIN pg_namespace n ON n.oid = t.relnamespace
        JOIN pg_am am ON am.oid = ic.relam
        CROSS JOIN LATERAL generate_subscripts(xi.indkey::int2[], 1) AS s(k)
        WHERE n.nspname = $1
            AND t.relkind IN ('r', 'p')
            AND ic.relkind = 'i'
            AND am.amname = 'btree'
            AND xi.indisvalid
            AND NOT xi.indisprimary
            AND (xi.indkey::int2[])[s.k] = 0::int2
        ORDER BY t.relname, ic.relname, s.k
    ";
    let rows = executor.query_all(sql, &[&schema])?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let table_name: String = row.try_get(0).map_err(|e| {
            LifeError::Other(format!("compare-schema btree expr key tablename: {e}"))
        })?;
        let index_name: String = row.try_get(1).map_err(|e| {
            LifeError::Other(format!("compare-schema btree expr key indexname: {e}"))
        })?;
        let key_ordinal: i32 = row.try_get(2).map_err(|e| {
            LifeError::Other(format!("compare-schema btree expr key key_ord: {e}"))
        })?;
        let key_def: String = row.try_get(3).map_err(|e| {
            LifeError::Other(format!("compare-schema btree expr key key_def: {e}"))
        })?;
        out.push(LiveBtreeExpressionKeyRow {
            table_name,
            index_name,
            key_ordinal,
            key_def,
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

fn strip_create_index_prefix_options(mut r: &str) -> &str {
    r = r.trim_start();
    while r.len() >= 13 && r[..13].eq_ignore_ascii_case("CONCURRENTLY ") {
        r = r[13..].trim_start();
    }
    while r.len() >= 14 && r[..14].eq_ignore_ascii_case("IF NOT EXISTS ") {
        r = r[14..].trim_start();
    }
    r
}

fn collapse_ws_outside_quotes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_double = false;
    let mut in_single = false;
    let mut prev_ws = false;
    for ch in input.chars() {
        match ch {
            '"' if !in_single => {
                in_double = !in_double;
                out.push(ch);
                prev_ws = false;
            }
            '\'' if !in_double => {
                in_single = !in_single;
                out.push(ch);
                prev_ws = false;
            }
            c if c.is_whitespace() && !in_double && !in_single => {
                if !prev_ws {
                    out.push(' ');
                    prev_ws = true;
                }
            }
            c => {
                out.push(c);
                prev_ws = false;
            }
        }
    }
    out.trim().to_string()
}

fn strip_using_btree_after_on_table(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let Some(on_pos) = lower.find(" on ") else {
        return s.to_string();
    };
    let before_on = &s[..on_pos + 4];
    let tail = s[on_pos + 4..].trim_start();
    let Some(after_table) = skip_qualified_table(tail) else {
        return s.to_string();
    };
    let consumed = tail.len() - after_table.len();
    let table_part = &tail[..consumed];
    let mut rest = after_table.trim_start();
    const USING_BTREE: &str = "using btree";
    if rest.len() >= USING_BTREE.len()
        && rest[..USING_BTREE.len()].eq_ignore_ascii_case(USING_BTREE)
    {
        let after_using = rest[USING_BTREE.len()..].trim_start();
        if after_using.starts_with('(') {
            rest = after_using;
        }
    }
    format!("{}{}{}", before_on, table_part.trim_end(), rest)
}

/// Normalize a `CREATE [UNIQUE] INDEX` statement for **T1** string comparison (migration vs `pg_indexes.indexdef`).
#[must_use]
pub fn normalize_index_statement_for_compare(s: &str) -> String {
    let s = s.trim().trim_end_matches(';').trim();
    let upper = s.to_ascii_uppercase();
    let rebuilt = if upper.starts_with("CREATE UNIQUE INDEX ") {
        let r = strip_create_index_prefix_options(&s["CREATE UNIQUE INDEX ".len()..]);
        format!("CREATE UNIQUE INDEX {r}")
    } else if upper.starts_with("CREATE INDEX ") {
        let r = strip_create_index_prefix_options(&s["CREATE INDEX ".len()..]);
        format!("CREATE INDEX {r}")
    } else {
        s.to_string()
    };
    let c = collapse_ws_outside_quotes(rebuilt.trim());
    strip_using_btree_after_on_table(&c)
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
        if !self.index_expression_key_vs_simple_migration_drifts.is_empty() {
            writeln!(
                f,
                "  Live btree index uses expression key(s); merged migration lists simple columns only (pg_catalog T3; T1 suppressed for these indexes; shared tables only; primary key indexes skipped):"
            )?;
            for d in &self.index_expression_key_vs_simple_migration_drifts {
                let ord: Vec<String> = d
                    .expression_key_ordinals
                    .iter()
                    .map(|o| o.to_string())
                    .collect();
                let mig = d.migration_simple_key_columns.join(", ");
                writeln!(
                    f,
                    "    Table `{}` index `{}`: expression key position(s) [{}]; migration simple keys: {}; live expression(s): {}",
                    d.table,
                    d.index_name,
                    ord.join(", "),
                    mig,
                    d.live_expression_key_defs.join(" | ")
                )?;
            }
        }
        if !self.index_definition_text_drifts.is_empty() {
            writeln!(
                f,
                "  Index definition text differs (normalized CREATE INDEX vs live pg_indexes.indexdef; shared tables only; primary key indexes skipped):"
            )?;
            for d in &self.index_definition_text_drifts {
                writeln!(
                    f,
                    "    Table `{}` index `{}`:",
                    d.table, d.index_name
                )?;
                writeln!(f, "      migration: {}", d.normalized_migration)?;
                writeln!(f, "      live:      {}", d.normalized_live)?;
            }
        }
        if !self.index_only_in_database.is_empty() {
            writeln!(
                f,
                "  Indexes only in live database (not in merged migration baseline; shared tables only; primary key indexes skipped):"
            )?;
            for d in &self.index_only_in_database {
                writeln!(f, "    Table `{}` index `{}`", d.table, d.index_name)?;
            }
        }
        if !self.index_only_in_migration.is_empty() {
            writeln!(
                f,
                "  Indexes only in merged migration baseline (not in live database; shared tables only):"
            )?;
            for d in &self.index_only_in_migration {
                writeln!(f, "    Table `{}` index `{}`", d.table, d.index_name)?;
            }
        }
        if !self.index_btree_nondefault_opclass_drifts.is_empty() {
            writeln!(
                f,
                "  Btree index key uses non-default operator class (pg_catalog T2b; shared tables only; primary key indexes excluded):"
            )?;
            for d in &self.index_btree_nondefault_opclass_drifts {
                let col = d
                    .column_name
                    .as_deref()
                    .map_or("(expression key not evaluated)".to_string(), str::to_string);
                let def = d
                    .default_opclass_name
                    .as_deref()
                    .unwrap_or("?");
                writeln!(
                    f,
                    "    Table `{}` index `{}` key #{} column `{}`: opclass `{}` (default for type: `{}`)",
                    d.table,
                    d.index_name,
                    d.key_ordinal,
                    col,
                    d.opclass_name,
                    def
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
            index_expression_key_vs_simple_migration_drifts: vec![],
            index_definition_text_drifts: vec![],
            index_only_in_database: vec![],
            index_only_in_migration: vec![],
            index_btree_nondefault_opclass_drifts: vec![],
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
            index_expression_key_vs_simple_migration_drifts: vec![],
            index_definition_text_drifts: vec![],
            index_only_in_database: vec![],
            index_only_in_migration: vec![],
            index_btree_nondefault_opclass_drifts: vec![],
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
            index_expression_key_vs_simple_migration_drifts: vec![],
            index_definition_text_drifts: vec![],
            index_only_in_database: vec![],
            index_only_in_migration: vec![],
            index_btree_nondefault_opclass_drifts: vec![],
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
            index_expression_key_vs_simple_migration_drifts: vec![],
            index_definition_text_drifts: vec![],
            index_only_in_database: vec![],
            index_only_in_migration: vec![],
            index_btree_nondefault_opclass_drifts: vec![],
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
            index_expression_key_vs_simple_migration_drifts: vec![],
            index_definition_text_drifts: vec![],
            index_only_in_database: vec![],
            index_only_in_migration: vec![],
            index_btree_nondefault_opclass_drifts: vec![],
        };
        assert!(r.has_drift());
        assert!(r.to_string().contains("access method not btree"));
        assert!(r.to_string().contains("USING hash"));
    }

    #[test]
    fn normalize_index_statement_equates_if_not_exists_and_using_btree() {
        let mig = "CREATE INDEX IF NOT EXISTS i ON public.t(id);";
        let live = "CREATE INDEX i ON public.t USING btree (id)";
        assert_eq!(
            normalize_index_statement_for_compare(mig),
            normalize_index_statement_for_compare(live)
        );
    }

    #[test]
    fn has_drift_when_btree_nondefault_opclass_only() {
        let r = MigrationDbCompareReport {
            schema: "public".into(),
            generated_dir: Path::new("/x").to_path_buf(),
            only_in_database: vec![],
            only_in_migrations: vec![],
            column_drifts: vec![],
            index_column_drifts: vec![],
            index_access_method_drifts: vec![],
            index_expression_key_vs_simple_migration_drifts: vec![],
            index_definition_text_drifts: vec![],
            index_only_in_database: vec![],
            index_only_in_migration: vec![],
            index_btree_nondefault_opclass_drifts: vec![IndexBtreeNonDefaultOpclassDrift {
                table: "t".into(),
                index_name: "ix".into(),
                key_ordinal: 1,
                column_name: Some("body".into()),
                opclass_name: "text_pattern_ops".into(),
                default_opclass_name: Some("text_ops".into()),
            }],
        };
        assert!(r.has_drift());
        let s = r.to_string();
        assert!(s.contains("non-default operator class"));
        assert!(s.contains("text_pattern_ops"));
    }

    #[test]
    fn has_drift_when_expression_key_vs_simple_migration_only() {
        let r = MigrationDbCompareReport {
            schema: "public".into(),
            generated_dir: Path::new("/x").to_path_buf(),
            only_in_database: vec![],
            only_in_migrations: vec![],
            column_drifts: vec![],
            index_column_drifts: vec![],
            index_access_method_drifts: vec![],
            index_expression_key_vs_simple_migration_drifts: vec![
                IndexExpressionKeyVsSimpleMigrationDrift {
                    table: "t".into(),
                    index_name: "ix".into(),
                    expression_key_ordinals: vec![1],
                    live_expression_key_defs: vec!["lower((email))".into()],
                    migration_simple_key_columns: vec!["email".into()],
                },
            ],
            index_definition_text_drifts: vec![],
            index_only_in_database: vec![],
            index_only_in_migration: vec![],
            index_btree_nondefault_opclass_drifts: vec![],
        };
        assert!(r.has_drift());
        let s = r.to_string();
        assert!(s.contains("expression key"));
        assert!(s.contains("lower((email))"));
    }
}
