//! `LifeguardPool` + `PooledLifeExecutor` against a real streaming replica (PRD read-replica).
//!
//! Skips when `TEST_REPLICA_URL` is unset (e.g. local testcontainers-only runs).

use lifeguard::test_helpers::TestDatabase;
use lifeguard::{LifeExecutor, LifeguardPool, PooledLifeExecutor};
use sea_query::{Value, Values};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn replica_url_or_skip() -> Option<String> {
    let ctx = crate::context::get_test_context();
    ctx.replica_pg_url.clone()
}

fn setup_schema_on_primary(executor: &lifeguard::MayPostgresExecutor) {
    executor
        .execute(
            "CREATE SCHEMA IF NOT EXISTS pool_replica_test",
            &[],
        )
        .expect("create schema");
    executor
        .execute(
            "DROP TABLE IF EXISTS pool_replica_test.t_pool_replica_smoke CASCADE",
            &[],
        )
        .expect("drop table");
    executor
        .execute(
            r#"
            CREATE TABLE pool_replica_test.t_pool_replica_smoke (
                id INTEGER PRIMARY KEY,
                note TEXT NOT NULL
            )
            "#,
            &[],
        )
        .expect("create table");
}

#[test]
fn pooled_pool_construct_write_read_with_replica() {
    let Some(replica_url) = replica_url_or_skip() else {
        return;
    };

    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();

    let mut db = TestDatabase::with_url(&primary_url);
    let setup_exec = db.executor().expect("primary executor");
    setup_schema_on_primary(&setup_exec);

    let pool = Arc::new(
        LifeguardPool::new(&primary_url, 1, vec![replica_url.clone()], 1)
            .expect("LifeguardPool::new with replica"),
    );
    let exec = PooledLifeExecutor::new(pool.clone());

    let insert_vals = Values(vec![
        Value::Int(Some(7)),
        Value::String(Some("via-pool".into())),
    ]);
    exec.execute_values(
        "INSERT INTO pool_replica_test.t_pool_replica_smoke (id, note) VALUES ($1, $2)",
        &insert_vals,
    )
    .expect("pooled insert");

    let lsn = crate::replication_sync::primary_current_wal_lsn(&primary_url).expect("primary lsn");
    crate::replication_sync::wait_replica_replayed_at_least(
        &replica_url,
        &lsn,
        Duration::from_secs(45),
        Duration::from_millis(50),
    )
    .expect("replica replay wait");

    assert!(
        crate::replication_sync::postgres_is_in_recovery(&replica_url).expect("is_in_recovery query"),
        "TEST_REPLICA_URL must be a standby (pg_is_in_recovery)"
    );

    // WalLagMonitor polls every 500ms; allow it to observe a healthy standby.
    thread::sleep(Duration::from_millis(700));
    let mut lag_ok = false;
    for _ in 0..30 {
        if !pool.is_replica_lagging() {
            lag_ok = true;
            break;
        }
        thread::sleep(Duration::from_millis(200));
    }
    assert!(
        lag_ok,
        "expected replica not lagging for pool read routing after warmup"
    );

    let read_vals = Values(vec![Value::Int(Some(7))]);
    let row = exec
        .query_one_values(
            "SELECT note FROM pool_replica_test.t_pool_replica_smoke WHERE id = $1",
            &read_vals,
        )
        .expect("pooled read");
    let note: String = row.get(0);
    assert_eq!(note, "via-pool");

    let rep = may_postgres::connect(&replica_url).expect("replica direct connect");
    let r2 = rep
        .query_one(
            "SELECT note FROM pool_replica_test.t_pool_replica_smoke WHERE id = 7",
            &[],
        )
        .expect("direct replica read");
    let note2: String = r2.get(0);
    assert_eq!(note2, "via-pool");
}

/// Lag fallback requires fault injection or a controllable slow replica; tracked for a follow-up.
#[test]
#[ignore = "R3.3: inject lag or error to assert primary fallback (future)"]
fn pooled_read_falls_back_when_replica_lagging() {
    let Some(replica_url) = replica_url_or_skip() else {
        return;
    };
    let _ = replica_url;
}
