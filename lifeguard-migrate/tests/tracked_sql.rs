//! Tracked SQL migrations against a live PostgreSQL (`TEST_DATABASE_URL`; skipped when unset).
//!
//! Every test works in its own schema, so tests can run in parallel against one database.

use lifeguard_migrate::tracked::{
    self, Action, Baseline, FileKind, Options, PlannedFile, TrackedError,
};
use may_postgres::Client;
use std::path::Path;
use std::time::Duration;

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
}

struct Db {
    client: Client,
    schema: String,
}

impl Db {
    fn new(name: &str) -> Option<Self> {
        let Some(url) = url() else {
            eprintln!("TEST_DATABASE_URL unset: skipping {name}");
            return None;
        };
        let client = lifeguard::connect(&url).expect("connect");
        let schema = format!("tracked_{name}_{}", std::process::id());
        client
            .batch_execute(&format!(
                "DROP SCHEMA IF EXISTS {schema} CASCADE; CREATE SCHEMA {schema}"
            ))
            .expect("schema");
        Some(Self { client, schema })
    }

    fn opts(&self, release: &str) -> Options {
        let mut o = Options::new(release);
        o.table = format!("{}.schema_migrations", self.schema);
        o.preamble = Some(format!("SET search_path TO {}", self.schema));
        o.lock_timeout = Duration::from_secs(5);
        o
    }

    fn scalar_i64(&self, sql: &str) -> i64 {
        self.client
            .query_one(sql.replace("$S", &self.schema).as_str(), &[])
            .expect(sql)
            .get(0)
    }

    fn text(&self, sql: &str) -> Option<String> {
        self.client
            .query_opt(sql.replace("$S", &self.schema).as_str(), &[])
            .expect(sql)
            .map(|r| r.get(0))
    }

    fn run(&self, dir: &Path, plan: &str, opts: &Options) -> Result<tracked::Report, TrackedError> {
        let files = tracked::load_entries(&tracked::parse_plan(plan)?, dir)?;
        tracked::run(&self.client, &files, opts, None, &mut |_| {})
    }
}

impl Drop for Db {
    fn drop(&mut self) {
        let _ = self
            .client
            .batch_execute(&format!("DROP SCHEMA IF EXISTS {} CASCADE", self.schema));
    }
}

fn write(dir: &Path, name: &str, sql: &str) {
    let p = dir.join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, sql).unwrap();
}

#[test]
fn applies_once_then_skips_and_records_runs() {
    let Some(db) = Db::new("once") else { return };
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "m/001_items.sql", "CREATE TABLE items (id int PRIMARY KEY, name text);\nINSERT INTO items VALUES (1, 'a; b');");
    write(
        dir.path(),
        "m/002_more.sql",
        "INSERT INTO items VALUES (2, 'c');",
    );
    let plan = "m/001_items.sql\nm/002_more.sql\n";

    let r1 = db.run(dir.path(), plan, &db.opts("v1")).expect("first run");
    assert_eq!(r1.count(Action::Apply), 2);
    assert_eq!(db.scalar_i64("SELECT count(*) FROM $S.items"), 2);

    let r2 = db
        .run(dir.path(), plan, &db.opts("v2"))
        .expect("second run");
    assert_eq!(r2.count(Action::Skip), 2, "nothing re-applied");
    assert_eq!(db.scalar_i64("SELECT count(*) FROM $S.items"), 2);

    // Tracking rows keep the release that applied each file; one run row per run.
    assert_eq!(
        db.text("SELECT release FROM $S.schema_migrations WHERE file = 'm/001_items.sql'")
            .as_deref(),
        Some("v1")
    );
    assert_eq!(db.scalar_i64("SELECT count(*) FROM $S.schema_migrations WHERE status = 'applied' AND duration_ms IS NOT NULL"), 2);
    assert_eq!(
        db.scalar_i64("SELECT count(*) FROM $S.schema_migrations_runs WHERE status = 'succeeded'"),
        2
    );
    assert_eq!(
        db.scalar_i64("SELECT applied::bigint FROM $S.schema_migrations_runs WHERE release = 'v2'"),
        0
    );
    assert_eq!(
        db.scalar_i64("SELECT skipped::bigint FROM $S.schema_migrations_runs WHERE release = 'v2'"),
        2
    );
}

/// The incident this exists for: an old file re-created a narrower view on every run and broke
/// the release that widened it. With tracking, the old file never runs again.
#[test]
fn an_old_view_file_is_not_re_run() {
    let Some(db) = Db::new("view") else { return };
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "001_t.sql", "CREATE TABLE t (a int, b int);");
    write(
        dir.path(),
        "002_v.sql",
        "CREATE OR REPLACE VIEW v AS SELECT a FROM t;",
    );
    db.run(dir.path(), "001_t.sql\n002_v.sql", &db.opts("r1"))
        .expect("r1");

    write(
        dir.path(),
        "003_v_wider.sql",
        "DROP VIEW v; CREATE VIEW v AS SELECT a, b FROM t;",
    );
    let plan = "001_t.sql\n002_v.sql\n003_v_wider.sql";
    db.run(dir.path(), plan, &db.opts("r2"))
        .expect("r2 widens the view");
    // Re-applying 002 here would fail: "cannot drop columns from view".
    let r3 = db
        .run(dir.path(), plan, &db.opts("r3"))
        .expect("r3 runs nothing");
    assert_eq!(r3.count(Action::Skip), 3);
    assert_eq!(
        db.scalar_i64("SELECT count(*) FROM information_schema.columns WHERE table_schema = '$S' AND table_name = 'v'"),
        2
    );
}

#[test]
fn an_edited_versioned_file_fails_before_anything_is_applied() {
    let Some(db) = Db::new("edited") else { return };
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "001.sql", "CREATE TABLE e (a int);");
    db.run(dir.path(), "001.sql", &db.opts("r1")).expect("r1");

    write(dir.path(), "001.sql", "CREATE TABLE e (a int, b int);");
    write(dir.path(), "002.sql", "CREATE TABLE f (a int);");
    let err = db
        .run(dir.path(), "001.sql\n002.sql", &db.opts("r2"))
        .expect_err("edited file must fail the run");
    match &err {
        TrackedError::ChecksumMismatch { files, .. } => {
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].id, "001.sql");
        }
        other => panic!("expected ChecksumMismatch, got {other}"),
    }
    assert!(err.to_string().contains("001.sql"), "{err}");
    assert_eq!(
        db.scalar_i64("SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = '$S' AND c.relname = 'f'"),
        0,
        "002 was not applied"
    );
    assert_eq!(
        db.text("SELECT status FROM $S.schema_migrations_runs WHERE release = 'r2'")
            .as_deref(),
        Some("failed")
    );
    assert_eq!(
        db.text("SELECT failed_file FROM $S.schema_migrations_runs WHERE release = 'r2'")
            .as_deref(),
        Some("001.sql")
    );
}

#[test]
fn repeatable_files_rerun_when_they_change() {
    let Some(db) = Db::new("repeat") else { return };
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "001_t.sql", "CREATE TABLE t (a int, b int);");
    write(
        dir.path(),
        "views/v.sql",
        "-- lifeguard:repeatable\nCREATE OR REPLACE VIEW v AS SELECT a FROM t;",
    );
    write(dir.path(), "seed.sql", "INSERT INTO t VALUES (1, 1);");
    let plan = "001_t.sql\nviews/v.sql\nrepeatable seed.sql";
    let r1 = db.run(dir.path(), plan, &db.opts("r1")).expect("r1");
    assert_eq!(r1.count(Action::Apply), 3);
    assert_eq!(
        db.text("SELECT kind FROM $S.schema_migrations WHERE file = 'seed.sql'")
            .as_deref(),
        Some("repeatable")
    );

    let r2 = db.run(dir.path(), plan, &db.opts("r2")).expect("r2");
    assert_eq!(
        r2.count(Action::Skip),
        3,
        "unchanged repeatables are not re-run"
    );
    assert_eq!(db.scalar_i64("SELECT count(*) FROM $S.t"), 1);

    write(
        dir.path(),
        "views/v.sql",
        "-- lifeguard:repeatable\nCREATE OR REPLACE VIEW v AS SELECT a, b FROM t;",
    );
    let r3 = db.run(dir.path(), plan, &db.opts("r3")).expect("r3");
    assert_eq!(r3.count(Action::Reapply), 1);
    assert_eq!(
        db.text("SELECT release FROM $S.schema_migrations WHERE file = 'views/v.sql'")
            .as_deref(),
        Some("r3")
    );
    assert_eq!(
        db.scalar_i64("SELECT count(*) FROM information_schema.columns WHERE table_schema = '$S' AND table_name = 'v'"),
        2
    );
}

#[test]
fn a_failing_file_is_recorded_and_retried_after_a_fix() {
    let Some(db) = Db::new("failing") else { return };
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "001.sql", "CREATE TABLE ok1 (a int);");
    write(
        dir.path(),
        "002.sql",
        "CREATE TABLE ok2 (a int);\n\nSELECT * FROM missing_table;",
    );
    write(dir.path(), "003.sql", "CREATE TABLE ok3 (a int);");
    let plan = "001.sql\n002.sql\n003.sql";
    let err = db
        .run(dir.path(), plan, &db.opts("r1"))
        .expect_err("002 fails");
    match &err {
        TrackedError::Statement {
            file,
            index,
            line,
            error,
            ..
        } => {
            assert_eq!(file, "002.sql");
            assert_eq!(*index, 2);
            assert_eq!(*line, 3);
            assert!(error.contains("42P01"), "{error}");
            // `SELECT * FROM missing_table`: the server points at the table name.
            assert!(error.contains("(position: 15)"), "{error}");
        }
        other => panic!("expected Statement, got {other}"),
    }
    assert!(!err.to_string().contains('\n'), "one log line: {err}");
    assert_eq!(
        db.text("SELECT status FROM $S.schema_migrations WHERE file = '002.sql'")
            .as_deref(),
        Some("failed")
    );
    assert!(db
        .text("SELECT file FROM $S.schema_migrations WHERE file = '003.sql'")
        .is_none());
    assert_eq!(
        db.text("SELECT status FROM $S.schema_migrations_runs")
            .as_deref(),
        Some("failed")
    );

    // A failed file may be edited: it was never applied.
    write(
        dir.path(),
        "002.sql",
        "CREATE TABLE IF NOT EXISTS ok2 (a int);",
    );
    let r2 = db
        .run(dir.path(), plan, &db.opts("r2"))
        .expect("r2 after the fix");
    assert_eq!(r2.count(Action::Apply), 2);
    assert_eq!(r2.count(Action::Skip), 1);
}

#[test]
fn baseline_records_existing_files_without_running_them() {
    let Some(db) = Db::new("baseline") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    // Already applied by the old runner; would fail if run again.
    let old = "SELECT 1 / 0;";
    write(dir.path(), "001_old.sql", old);
    write(dir.path(), "002_old.sql", old);
    write(dir.path(), "003_new.sql", "CREATE TABLE fresh (a int);");
    // The old runner's ledger: file and sha256.
    db.client
        .batch_execute(&format!(
            "CREATE TABLE {s}.old_ledger (file text, sha256 text);
             INSERT INTO {s}.old_ledger VALUES ('001_old.sql', '{c}'), ('002_old.sql', NULL), ('gone.sql', 'x');",
            s = db.schema,
            c = tracked::checksum(old.as_bytes())
        ))
        .unwrap();
    let mut opts = db.opts("first");
    opts.baseline = Baseline::Query(format!("SELECT file, sha256 FROM {}.old_ledger", db.schema));

    // Dry run first: reports, writes nothing.
    let mut dry = opts.clone();
    dry.dry_run = true;
    let plan = "001_old.sql\n002_old.sql\n003_new.sql";
    let d = db.run(dir.path(), plan, &dry).expect("dry run");
    assert_eq!(d.count(Action::Baseline), 2);
    assert_eq!(d.count(Action::Apply), 1);
    assert!(d.run_id.is_none());
    assert_eq!(
        db.scalar_i64(&format!(
            "SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = '{}' AND c.relname = 'schema_migrations'",
            db.schema
        )),
        0,
        "dry run creates nothing"
    );

    let r = db.run(dir.path(), plan, &opts).expect("baseline run");
    assert_eq!(r.count(Action::Baseline), 2);
    assert_eq!(r.count(Action::Apply), 1);
    assert_eq!(
        db.scalar_i64("SELECT count(*) FROM $S.schema_migrations WHERE status = 'baselined' AND release = 'first'"),
        2
    );
    assert_eq!(
        db.scalar_i64("SELECT baselined::bigint FROM $S.schema_migrations_runs"),
        2
    );

    // The baseline is only taken on an empty table; the next run just skips.
    let r2 = db.run(dir.path(), plan, &opts).expect("second run");
    assert_eq!(r2.count(Action::Skip), 3);
}

#[test]
fn autocommit_statements_and_single_transaction_files() {
    let Some(db) = Db::new("txn") else { return };
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "001.sql",
        "CREATE TABLE c (a int);\nCREATE INDEX CONCURRENTLY c_a ON c (a);",
    );
    write(
        dir.path(),
        "002.sql",
        "-- lifeguard:single-transaction\nCREATE TABLE half (a int);\nSELECT * FROM missing_table;",
    );
    let err = db
        .run(dir.path(), "001.sql\n002.sql", &db.opts("r1"))
        .expect_err("002 fails");
    assert!(
        matches!(err, TrackedError::Statement { ref file, .. } if file == "002.sql"),
        "{err}"
    );
    assert_eq!(
        db.scalar_i64(
            "SELECT count(*) FROM pg_indexes WHERE schemaname = '$S' AND indexname = 'c_a'"
        ),
        1,
        "CONCURRENTLY works outside a transaction"
    );
    assert_eq!(
        db.scalar_i64("SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = '$S' AND c.relname = 'half'"),
        0,
        "the single-transaction file rolled back"
    );
}

#[test]
fn a_second_runner_waits_for_the_lock() {
    let Some(db) = Db::new("lock") else { return };
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "001.sql", "SELECT 1;");
    let mut opts = db.opts("r1");
    opts.lock_timeout = Duration::from_secs(1);
    let other = lifeguard::connect(&url().unwrap()).expect("connect");
    other
        .query_one(
            "SELECT pg_advisory_lock(hashtext($1)::bigint)",
            &[&opts.table],
        )
        .expect("hold the lock");
    let err = db.run(dir.path(), "001.sql", &opts).expect_err("lock held");
    assert!(matches!(err, TrackedError::LockTimeout(_)), "{err}");
    other
        .query_one(
            "SELECT pg_advisory_unlock(hashtext($1)::bigint)",
            &[&opts.table],
        )
        .unwrap();
    db.run(dir.path(), "001.sql", &opts)
        .expect("runs once the lock is free");
}

#[test]
fn plan_kinds_and_filters() {
    let Some(db) = Db::new("filter") else { return };
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "001.sql", "CREATE TABLE owner_marker (who text);\nINSERT INTO owner_marker VALUES ('ROLE_PLACEHOLDER');");
    let files: Vec<PlannedFile> =
        tracked::load_entries(&tracked::parse_plan("001.sql").unwrap(), dir.path()).unwrap();
    assert_eq!(files[0].kind, FileKind::Versioned);
    let filter = |f: &PlannedFile| -> Result<String, String> {
        Ok(f.sql.replace("ROLE_PLACEHOLDER", "app_owner"))
    };
    tracked::run(
        &db.client,
        &files,
        &db.opts("r1"),
        Some(&filter),
        &mut |_| {},
    )
    .expect("run");
    assert_eq!(
        db.text("SELECT who FROM $S.owner_marker").as_deref(),
        Some("app_owner")
    );
    // The checksum is of the file as written, not the rewritten text.
    assert_eq!(
        db.text("SELECT checksum FROM $S.schema_migrations")
            .as_deref(),
        Some(files[0].checksum.as_str())
    );
}
