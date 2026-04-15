//! Optional live-PostgreSQL smoke for [`lifeguard_migrate::schema_migration_compare`] + `compare-schema` CLI.
//!
//! Uses a **dedicated schema** per run so we only compare the scratch table against merged migration
//! baselines — not the entire shared `public` schema (which holds many integration-test tables in CI).

use std::fs;

use lifeguard::{connect, LifeExecutor, MayPostgresExecutor};
use lifeguard_migrate::schema_migration_compare::{
    compare_generated_dir_to_live_db, fetch_live_btree_expression_index_key_slots,
    fetch_live_btree_index_key_opclasses,
};
use uuid::Uuid;

const FILE: &str = "20990101000000_generated_from_entities.sql";
const TABLE: &str = "smoke_t";
const EXTRA_COL: &str = "extra_col";
const HASH_IDX_COL: &str = "x";
/// T2b catalog smoke: btree index with non-default opclass (`text_pattern_ops`; default is `text_ops`).
/// Note: `jsonb_path_ops` is **GIN-only** and cannot be used with `USING btree`.
const OPCLASS_TABLE: &str = "smoke_opclass_t";
const OPCLASS_IDX: &str = "idx_smoke_opclass_body";
/// T3 catalog smoke: live expression btree key vs merged simple-column `CREATE INDEX`.
const EXPR_TABLE: &str = "smoke_expr_t";
const EXPR_IDX: &str = "idx_smoke_expr_email";
/// T2b: migration names same non-default opclass as live — expect no opclass drift.
const OPCLASS_MATCH_TABLE: &str = "smoke_opclass_match_t";
const OPCLASS_MATCH_IDX: &str = "idx_opclass_match_body";
/// T3 v2: migration + live both expression index, normalized slots match — no T1 / slot mismatch.
const T3V2_TABLE: &str = "smoke_t3v2_t";
const T3V2_IDX: &str = "idx_t3v2_lower_email";
/// Ordering / collation: explicit DESC in migration vs live ASC.
const ORD_TABLE: &str = "smoke_ord_t";
const ORD_IDX: &str = "idx_ord_id";

fn postgres_url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .or_else(|_| std::env::var("LIFEGUARD_DATABASE_URL"))
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// Unique schema so parallel tests and shared DBs do not see unrelated `public` tables.
fn scratch_schema_name() -> String {
    format!("lg_cmp_{id}", id = Uuid::new_v4().simple())
}

#[test]
fn compare_generated_dir_matches_live_table_set() {
    let Some(url) = postgres_url() else {
        eprintln!(
            "compare_generated_dir_matches_live_table_set: skipped (set TEST_DATABASE_URL, DATABASE_URL, or LIFEGUARD_DATABASE_URL)"
        );
        return;
    };

    let schema = scratch_schema_name();
    let dir = tempfile::tempdir().expect("tempdir");
    let sql = format!(
        "-- Table: {TABLE}\n\
         CREATE TABLE IF NOT EXISTS {TABLE} (id INTEGER PRIMARY KEY);\n"
    );
    fs::write(dir.path().join(FILE), sql).expect("write generated sql");

    let client = connect(&url).expect("connect");
    let executor = MayPostgresExecutor::new(client);

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();
    executor
        .execute(&format!("CREATE SCHEMA {schema}"), &[])
        .expect("create schema");
    executor
        .execute(
            &format!("CREATE TABLE {schema}.{TABLE} (id INTEGER PRIMARY KEY)"),
            &[],
        )
        .expect("create scratch table");

    let report = compare_generated_dir_to_live_db(&executor, &schema, dir.path())
        .expect("compare_generated_dir_to_live_db");

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();

    assert!(
        !report.has_drift(),
        "expected no drift; only_in_db={:?} only_mig={:?}",
        report.only_in_database,
        report.only_in_migrations
    );
}

#[test]
fn compare_reports_column_and_index_drift_when_live_has_extra_indexed_column() {
    let Some(url) = postgres_url() else {
        eprintln!(
            "compare_reports_column_and_index_drift_when_live_has_extra_indexed_column: skipped (no DB URL)"
        );
        return;
    };

    let schema = scratch_schema_name();
    let dir = tempfile::tempdir().expect("tempdir");
    let sql = format!(
        "-- Table: {TABLE}\n\
         CREATE TABLE IF NOT EXISTS {TABLE} (id INTEGER PRIMARY KEY);\n"
    );
    fs::write(dir.path().join(FILE), sql).expect("write generated sql");

    let client = connect(&url).expect("connect");
    let executor = MayPostgresExecutor::new(client);

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();
    executor
        .execute(&format!("CREATE SCHEMA {schema}"), &[])
        .expect("create schema");
    executor
        .execute(
            &format!(
                "CREATE TABLE {schema}.{TABLE} (id INTEGER NOT NULL PRIMARY KEY, {EXTRA_COL} INTEGER NOT NULL)"
            ),
            &[],
        )
        .expect("create scratch table");
    executor
        .execute(
            &format!("CREATE INDEX {TABLE}_extra_idx ON {schema}.{TABLE} ({EXTRA_COL})"),
            &[],
        )
        .expect("create index on extra column");

    let report = compare_generated_dir_to_live_db(&executor, &schema, dir.path())
        .expect("compare_generated_dir_to_live_db");

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();

    assert!(report.has_drift(), "expected drift");
    let col = report
        .column_drifts
        .iter()
        .find(|d| d.table == TABLE)
        .expect("column drift for smoke table");
    assert_eq!(col.only_in_database, vec![EXTRA_COL.to_string()]);
    assert!(col.only_in_migrations.is_empty());
    let ix = report
        .index_column_drifts
        .iter()
        .find(|d| d.table == TABLE)
        .expect("index drift for smoke table");
    assert_eq!(ix.unknown_columns, vec![EXTRA_COL.to_string()]);
}

#[test]
fn compare_reports_access_method_drift_when_live_uses_non_btree_index() {
    let Some(url) = postgres_url() else {
        eprintln!(
            "compare_reports_access_method_drift_when_live_uses_non_btree_index: skipped (no DB URL)"
        );
        return;
    };

    let schema = scratch_schema_name();
    let dir = tempfile::tempdir().expect("tempdir");
    let sql = format!(
        "-- Table: {TABLE}\n\
         CREATE TABLE IF NOT EXISTS {TABLE} (id INTEGER PRIMARY KEY, {HASH_IDX_COL} INTEGER NOT NULL);\n"
    );
    fs::write(dir.path().join(FILE), sql).expect("write generated sql");

    let client = connect(&url).expect("connect");
    let executor = MayPostgresExecutor::new(client);

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();
    executor
        .execute(&format!("CREATE SCHEMA {schema}"), &[])
        .expect("create schema");
    executor
        .execute(
            &format!(
                "CREATE TABLE {schema}.{TABLE} (id INTEGER NOT NULL PRIMARY KEY, {HASH_IDX_COL} INTEGER NOT NULL)"
            ),
            &[],
        )
        .expect("create scratch table");
    executor
        .execute(
            &format!(
                "CREATE INDEX {TABLE}_hash_idx ON {schema}.{TABLE} USING hash ({HASH_IDX_COL})"
            ),
            &[],
        )
        .expect("create hash index");

    let report = compare_generated_dir_to_live_db(&executor, &schema, dir.path())
        .expect("compare_generated_dir_to_live_db");

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();

    assert!(report.has_drift(), "expected access-method drift");
    assert!(
        report.column_drifts.is_empty(),
        "no column drift when merged baseline lists all columns: {:?}",
        report.column_drifts
    );
    assert!(
        report.index_column_drifts.is_empty(),
        "x is in merged baseline: {:?}",
        report.index_column_drifts
    );
    let am = report
        .index_access_method_drifts
        .iter()
        .find(|d| d.table == TABLE && d.index_name == format!("{TABLE}_hash_idx"))
        .expect("access method drift for hash index");
    assert_eq!(am.access_method, "hash");
}

#[test]
fn fetch_live_btree_index_key_opclasses_lists_text_pattern_ops() {
    let Some(url) = postgres_url() else {
        eprintln!(
            "fetch_live_btree_index_key_opclasses_lists_text_pattern_ops: skipped (no DB URL)"
        );
        return;
    };

    let schema = scratch_schema_name();
    let client = connect(&url).expect("connect");
    let executor = MayPostgresExecutor::new(client);

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();
    executor
        .execute(&format!("CREATE SCHEMA {schema}"), &[])
        .expect("create schema");
    executor
        .execute(
            &format!(
                "CREATE TABLE {schema}.{OPCLASS_TABLE} (id INTEGER NOT NULL PRIMARY KEY, body TEXT NOT NULL)"
            ),
            &[],
        )
        .expect("create table");
    executor
        .execute(
            &format!(
                "CREATE INDEX {OPCLASS_IDX} ON {schema}.{OPCLASS_TABLE} USING btree (body text_pattern_ops)"
            ),
            &[],
        )
        .expect("create text_pattern_ops btree index");

    let rows = fetch_live_btree_index_key_opclasses(&executor, &schema).expect("catalog query");
    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();

    let hit = rows.iter().find(|r| {
        r.table_name == OPCLASS_TABLE
            && r.index_name == OPCLASS_IDX
            && r.opclass_name == "text_pattern_ops"
    });
    let Some(hit) = hit else {
        panic!("expected catalog row for {OPCLASS_TABLE}/{OPCLASS_IDX}, got: {rows:?}");
    };
    assert!(
        hit.is_non_default_opclass,
        "text_pattern_ops should differ from default text_ops: {hit:?}"
    );
    assert_eq!(hit.column_name.as_deref(), Some("body"));
    assert_eq!(hit.default_opclass_name.as_deref(), Some("text_ops"));
}

#[test]
fn compare_reports_btree_non_default_opclass_when_live_uses_text_pattern_ops() {
    let Some(url) = postgres_url() else {
        eprintln!(
            "compare_reports_btree_non_default_opclass_when_live_uses_text_pattern_ops: skipped (no DB URL)"
        );
        return;
    };

    let schema = scratch_schema_name();
    let dir = tempfile::tempdir().expect("tempdir");
    let sql = format!(
        "-- Table: {OPCLASS_TABLE}\n\
         CREATE TABLE IF NOT EXISTS {OPCLASS_TABLE} (id INTEGER PRIMARY KEY, body TEXT NOT NULL);\n\n\
         CREATE INDEX {OPCLASS_IDX} ON {OPCLASS_TABLE} (body);\n"
    );
    fs::write(dir.path().join(FILE), sql).expect("write generated sql");

    let client = connect(&url).expect("connect");
    let executor = MayPostgresExecutor::new(client);

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();
    executor
        .execute(&format!("CREATE SCHEMA {schema}"), &[])
        .expect("create schema");
    executor
        .execute(
            &format!(
                "CREATE TABLE {schema}.{OPCLASS_TABLE} (id INTEGER NOT NULL PRIMARY KEY, body TEXT NOT NULL)"
            ),
            &[],
        )
        .expect("create table");
    executor
        .execute(
            &format!(
                "CREATE INDEX {OPCLASS_IDX} ON {schema}.{OPCLASS_TABLE} USING btree (body text_pattern_ops)"
            ),
            &[],
        )
        .expect("create text_pattern_ops btree index");

    let report = compare_generated_dir_to_live_db(&executor, &schema, dir.path())
        .expect("compare_generated_dir_to_live_db");

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();

    let d = report
        .index_btree_nondefault_opclass_drifts
        .iter()
        .find(|x| x.table == OPCLASS_TABLE && x.index_name == OPCLASS_IDX)
        .expect("expected T2b opclass drift");
    assert_eq!(d.opclass_name, "text_pattern_ops");
    assert_eq!(d.column_name.as_deref(), Some("body"));
    assert_eq!(d.default_opclass_name.as_deref(), Some("text_ops"));
    assert!(d.migration_explicit_opclass.is_none());
    assert!(report.has_drift(), "opclass drift should set has_drift");
}

#[test]
fn compare_no_opclass_drift_when_migration_explicit_matches_live() {
    let Some(url) = postgres_url() else {
        eprintln!(
            "compare_no_opclass_drift_when_migration_explicit_matches_live: skipped (no DB URL)"
        );
        return;
    };

    let schema = scratch_schema_name();
    let dir = tempfile::tempdir().expect("tempdir");
    let sql = format!(
        "-- Table: {OPCLASS_MATCH_TABLE}\n\
         CREATE TABLE IF NOT EXISTS {OPCLASS_MATCH_TABLE} (id INTEGER PRIMARY KEY, body TEXT NOT NULL);\n\n\
         CREATE INDEX {OPCLASS_MATCH_IDX} ON {OPCLASS_MATCH_TABLE} (body text_pattern_ops);\n"
    );
    fs::write(dir.path().join(FILE), sql).expect("write generated sql");

    let client = connect(&url).expect("connect");
    let executor = MayPostgresExecutor::new(client);

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();
    executor
        .execute(&format!("CREATE SCHEMA {schema}"), &[])
        .expect("create schema");
    executor
        .execute(
            &format!(
                "CREATE TABLE {schema}.{OPCLASS_MATCH_TABLE} (id INTEGER NOT NULL PRIMARY KEY, body TEXT NOT NULL)"
            ),
            &[],
        )
        .expect("create table");
    executor
        .execute(
            &format!(
                "CREATE INDEX {OPCLASS_MATCH_IDX} ON {schema}.{OPCLASS_MATCH_TABLE} USING btree (body text_pattern_ops)"
            ),
            &[],
        )
        .expect("create index");

    let report = compare_generated_dir_to_live_db(&executor, &schema, dir.path())
        .expect("compare_generated_dir_to_live_db");

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();

    assert!(
        report
            .index_btree_nondefault_opclass_drifts
            .iter()
            .all(|d| d.table != OPCLASS_MATCH_TABLE),
        "unexpected opclass drift: {:?}",
        report.index_btree_nondefault_opclass_drifts
    );
    assert!(
        !report.has_drift(),
        "expected no drift when migration opclass matches live; got {:?}",
        report.index_definition_text_drifts
    );
}

#[test]
fn compare_t3_v2_skips_t1_when_expression_indexdefs_normalize_equal() {
    let Some(url) = postgres_url() else {
        eprintln!(
            "compare_t3_v2_skips_t1_when_expression_indexdefs_normalize_equal: skipped (no DB URL)"
        );
        return;
    };

    let schema = scratch_schema_name();
    let dir = tempfile::tempdir().expect("tempdir");
    let sql = format!(
        "-- Table: {T3V2_TABLE}\n\
         CREATE TABLE IF NOT EXISTS {T3V2_TABLE} (id INTEGER PRIMARY KEY, email TEXT NOT NULL);\n\n\
         CREATE INDEX {T3V2_IDX} ON {T3V2_TABLE} ((lower(email)));\n"
    );
    fs::write(dir.path().join(FILE), sql).expect("write generated sql");

    let client = connect(&url).expect("connect");
    let executor = MayPostgresExecutor::new(client);

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();
    executor
        .execute(&format!("CREATE SCHEMA {schema}"), &[])
        .expect("create schema");
    executor
        .execute(
            &format!(
                "CREATE TABLE {schema}.{T3V2_TABLE} (id INTEGER NOT NULL PRIMARY KEY, email TEXT NOT NULL)"
            ),
            &[],
        )
        .expect("create table");
    executor
        .execute(
            &format!("CREATE INDEX {T3V2_IDX} ON {schema}.{T3V2_TABLE} ((lower(email)))"),
            &[],
        )
        .expect("create expression index");

    let report = compare_generated_dir_to_live_db(&executor, &schema, dir.path())
        .expect("compare_generated_dir_to_live_db");

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();

    assert!(
        report.index_key_normalized_slots_mismatch_drifts.is_empty(),
        "unexpected slot mismatch: {:?}",
        report.index_key_normalized_slots_mismatch_drifts
    );
    assert!(
        report.index_definition_text_drifts.is_empty(),
        "T3 v2 should suppress T1 when slots match; got {:?}",
        report.index_definition_text_drifts
    );
    assert!(!report.has_drift(), "expected no drift");
}

#[test]
fn compare_reports_ordering_drift_when_migration_desc_not_live_asc() {
    let Some(url) = postgres_url() else {
        eprintln!(
            "compare_reports_ordering_drift_when_migration_desc_not_live_asc: skipped (no DB URL)"
        );
        return;
    };

    let schema = scratch_schema_name();
    let dir = tempfile::tempdir().expect("tempdir");
    let sql = format!(
        "-- Table: {ORD_TABLE}\n\
         CREATE TABLE IF NOT EXISTS {ORD_TABLE} (id INTEGER PRIMARY KEY);\n\n\
         CREATE INDEX {ORD_IDX} ON {ORD_TABLE} (id DESC);\n"
    );
    fs::write(dir.path().join(FILE), sql).expect("write generated sql");

    let client = connect(&url).expect("connect");
    let executor = MayPostgresExecutor::new(client);

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();
    executor
        .execute(&format!("CREATE SCHEMA {schema}"), &[])
        .expect("create schema");
    executor
        .execute(
            &format!("CREATE TABLE {schema}.{ORD_TABLE} (id INTEGER NOT NULL PRIMARY KEY)"),
            &[],
        )
        .expect("create table");
    executor
        .execute(
            &format!("CREATE INDEX {ORD_IDX} ON {schema}.{ORD_TABLE} (id)"),
            &[],
        )
        .expect("create asc index");

    let report = compare_generated_dir_to_live_db(&executor, &schema, dir.path())
        .expect("compare_generated_dir_to_live_db");

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();

    let od = report
        .index_btree_key_ordering_collation_drifts
        .iter()
        .find(|d| d.table == ORD_TABLE && d.index_name == ORD_IDX)
        .expect("ordering/collation drift");
    assert!(
        od.detail.to_ascii_lowercase().contains("desc")
            || od.detail.to_ascii_lowercase().contains("asc"),
        "detail: {}",
        od.detail
    );
    assert!(report.has_drift());
}

#[test]
fn fetch_live_btree_expression_index_key_slots_lists_lower_email() {
    let Some(url) = postgres_url() else {
        eprintln!(
            "fetch_live_btree_expression_index_key_slots_lists_lower_email: skipped (no DB URL)"
        );
        return;
    };

    let schema = scratch_schema_name();
    let client = connect(&url).expect("connect");
    let executor = MayPostgresExecutor::new(client);

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();
    executor
        .execute(&format!("CREATE SCHEMA {schema}"), &[])
        .expect("create schema");
    executor
        .execute(
            &format!(
                "CREATE TABLE {schema}.{EXPR_TABLE} (id INTEGER NOT NULL PRIMARY KEY, email TEXT NOT NULL)"
            ),
            &[],
        )
        .expect("create table");
    executor
        .execute(
            &format!("CREATE INDEX {EXPR_IDX} ON {schema}.{EXPR_TABLE} ((lower(email)))"),
            &[],
        )
        .expect("create expression index");

    let rows = fetch_live_btree_expression_index_key_slots(&executor, &schema).expect("catalog");
    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();

    let hit = rows
        .iter()
        .find(|r| r.table_name == EXPR_TABLE && r.index_name == EXPR_IDX);
    let Some(hit) = hit else {
        panic!("expected expression key row for {EXPR_TABLE}/{EXPR_IDX}, got: {rows:?}");
    };
    assert_eq!(hit.key_ordinal, 1);
    assert!(
        hit.key_def.to_ascii_lowercase().contains("lower"),
        "unexpected pg_get_indexdef fragment: {:?}",
        hit.key_def
    );
}

#[test]
fn compare_reports_expression_key_when_migration_lists_simple_columns_only() {
    let Some(url) = postgres_url() else {
        eprintln!(
            "compare_reports_expression_key_when_migration_lists_simple_columns_only: skipped (no DB URL)"
        );
        return;
    };

    let schema = scratch_schema_name();
    let dir = tempfile::tempdir().expect("tempdir");
    let sql = format!(
        "-- Table: {EXPR_TABLE}\n\
         CREATE TABLE IF NOT EXISTS {EXPR_TABLE} (id INTEGER PRIMARY KEY, email TEXT NOT NULL);\n\n\
         CREATE INDEX {EXPR_IDX} ON {EXPR_TABLE} (email);\n"
    );
    fs::write(dir.path().join(FILE), sql).expect("write generated sql");

    let client = connect(&url).expect("connect");
    let executor = MayPostgresExecutor::new(client);

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();
    executor
        .execute(&format!("CREATE SCHEMA {schema}"), &[])
        .expect("create schema");
    executor
        .execute(
            &format!(
                "CREATE TABLE {schema}.{EXPR_TABLE} (id INTEGER NOT NULL PRIMARY KEY, email TEXT NOT NULL)"
            ),
            &[],
        )
        .expect("create table");
    executor
        .execute(
            &format!("CREATE INDEX {EXPR_IDX} ON {schema}.{EXPR_TABLE} ((lower(email)))"),
            &[],
        )
        .expect("create expression index");

    let report = compare_generated_dir_to_live_db(&executor, &schema, dir.path())
        .expect("compare_generated_dir_to_live_db");

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();

    let d = report
        .index_expression_key_vs_simple_migration_drifts
        .iter()
        .find(|x| x.table == EXPR_TABLE && x.index_name == EXPR_IDX)
        .expect("expected T3 expression-key drift");
    assert_eq!(d.migration_simple_key_columns, vec!["email".to_string()]);
    assert_eq!(d.expression_key_ordinals, vec![1]);
    assert!(
        !d.live_expression_key_defs.is_empty()
            && d.live_expression_key_defs[0]
                .to_ascii_lowercase()
                .contains("lower"),
        "live defs: {:?}",
        d.live_expression_key_defs
    );
    assert!(
        report
            .index_definition_text_drifts
            .iter()
            .all(|t| t.index_name != EXPR_IDX),
        "T1 should be suppressed when T3 fires: {:?}",
        report.index_definition_text_drifts
    );
    assert!(report.has_drift(), "T3 drift should set has_drift");
}

#[test]
fn compare_schema_cli_succeeds_when_no_drift() {
    let Some(url) = postgres_url() else {
        eprintln!("compare_schema_cli_succeeds_when_no_drift: skipped (no DB URL)");
        return;
    };

    let Some(bin) = std::env::var_os("CARGO_BIN_EXE_lifeguard-migrate") else {
        eprintln!("compare_schema_cli_succeeds_when_no_drift: skipped (no CARGO_BIN_EXE_lifeguard-migrate)");
        return;
    };

    let schema = scratch_schema_name();
    let dir = tempfile::tempdir().expect("tempdir");
    let sql = format!(
        "-- Table: {TABLE}\n\
         CREATE TABLE IF NOT EXISTS {TABLE} (id INTEGER PRIMARY KEY);\n"
    );
    fs::write(dir.path().join(FILE), sql).expect("write generated sql");

    let client = connect(&url).expect("connect");
    let executor = MayPostgresExecutor::new(client);
    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();
    executor
        .execute(&format!("CREATE SCHEMA {schema}"), &[])
        .expect("create schema");
    executor
        .execute(
            &format!("CREATE TABLE {schema}.{TABLE} (id INTEGER PRIMARY KEY)"),
            &[],
        )
        .expect("create scratch table");

    let out = std::process::Command::new(&bin)
        .args([
            "compare-schema",
            "--database-url",
            &url,
            "--schema",
            &schema,
            "--generated-dir",
            dir.path().to_str().expect("utf8 temp path"),
        ])
        .output()
        .expect("spawn compare-schema");

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();

    assert!(
        out.status.success(),
        "compare-schema failed: stderr={stderr}",
        stderr = String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("No drift"), "unexpected output: {stdout}");
}

#[test]
fn compare_schema_cli_fails_when_live_has_extra_indexed_column() {
    let Some(url) = postgres_url() else {
        eprintln!(
            "compare_schema_cli_fails_when_live_has_extra_indexed_column: skipped (no DB URL)"
        );
        return;
    };

    let Some(bin) = std::env::var_os("CARGO_BIN_EXE_lifeguard-migrate") else {
        eprintln!(
            "compare_schema_cli_fails_when_live_has_extra_indexed_column: skipped (no CARGO_BIN_EXE_lifeguard-migrate)"
        );
        return;
    };

    let schema = scratch_schema_name();
    let dir = tempfile::tempdir().expect("tempdir");
    let sql = format!(
        "-- Table: {TABLE}\n\
         CREATE TABLE IF NOT EXISTS {TABLE} (id INTEGER PRIMARY KEY);\n"
    );
    fs::write(dir.path().join(FILE), sql).expect("write generated sql");

    let client = connect(&url).expect("connect");
    let executor = MayPostgresExecutor::new(client);
    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();
    executor
        .execute(&format!("CREATE SCHEMA {schema}"), &[])
        .expect("create schema");
    executor
        .execute(
            &format!(
                "CREATE TABLE {schema}.{TABLE} (id INTEGER NOT NULL PRIMARY KEY, {EXTRA_COL} INTEGER NOT NULL)"
            ),
            &[],
        )
        .expect("create scratch table");
    executor
        .execute(
            &format!("CREATE INDEX {TABLE}_extra_idx ON {schema}.{TABLE} ({EXTRA_COL})"),
            &[],
        )
        .expect("create index");

    let out = std::process::Command::new(&bin)
        .args([
            "compare-schema",
            "--database-url",
            &url,
            "--schema",
            &schema,
            "--generated-dir",
            dir.path().to_str().expect("utf8 temp path"),
        ])
        .output()
        .expect("spawn compare-schema");

    executor
        .execute(&format!("DROP SCHEMA IF EXISTS {schema} CASCADE"), &[])
        .ok();

    assert!(
        !out.status.success(),
        "expected non-zero exit; stderr={stderr}",
        stderr = String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Column name differences") && stdout.contains(EXTRA_COL),
        "expected column drift in stdout: {stdout}"
    );
    assert!(
        stdout.contains("Index key / INCLUDE columns") && stdout.contains(EXTRA_COL),
        "expected index drift in stdout: {stdout}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("index keys") || stderr.contains("column names"),
        "expected summary error on stderr: {stderr}"
    );
}
