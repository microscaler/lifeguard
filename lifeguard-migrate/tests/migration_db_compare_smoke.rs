//! Optional live-PostgreSQL smoke for [`lifeguard_migrate::schema_migration_compare`] + `compare-schema` CLI.
//!
//! Uses a **dedicated schema** per run so we only compare the scratch table against merged migration
//! baselines — not the entire shared `public` schema (which holds many integration-test tables in CI).

use std::fs;

use lifeguard::{connect, LifeExecutor, MayPostgresExecutor};
use lifeguard_migrate::schema_migration_compare::compare_generated_dir_to_live_db;
use uuid::Uuid;

const FILE: &str = "20990101000000_generated_from_entities.sql";
const TABLE: &str = "smoke_t";

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
        .execute(
            &format!("DROP SCHEMA IF EXISTS {schema} CASCADE"),
            &[],
        )
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
        .execute(
            &format!("DROP SCHEMA IF EXISTS {schema} CASCADE"),
            &[],
        )
        .ok();

    assert!(
        !report.has_drift(),
        "expected no drift; only_in_db={:?} only_mig={:?}",
        report.only_in_database,
        report.only_in_migrations
    );
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
        .execute(
            &format!("DROP SCHEMA IF EXISTS {schema} CASCADE"),
            &[],
        )
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
        .execute(
            &format!("DROP SCHEMA IF EXISTS {schema} CASCADE"),
            &[],
        )
        .ok();

    assert!(
        out.status.success(),
        "compare-schema failed: stderr={stderr}",
        stderr = String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("No drift"),
        "unexpected output: {stdout}"
    );
}
