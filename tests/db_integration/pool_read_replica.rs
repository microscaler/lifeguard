//! `LifeguardPool` + `PooledLifeExecutor` against a real streaming replica (PRD read-replica).
//!
//! Skips when `TEST_REPLICA_URL` is unset (e.g. local testcontainers-only runs).
//!
//! ## Timing (`LIFEGUARD_POOL_TEST_TIMING=1`)
//!
//! Set this env var to print phase timings to **stderr** (setup, pool open, insert, replay wait,
//! lag-monitor readiness, pooled read, direct read, optional batch load).
//!
//! ## Sub‑500ms on localhost?
//!
//! Physical replication on the same host (Docker bridge) is usually **well under a millisecond**
//! of WAL apply latency. What dominated the old test was **not** the network:
//!
//! - A fixed **`thread::sleep(700ms)`** waiting for the lag monitor (removed).
//! - `WalLagMonitor` default **500ms** poll interval — configurable via
//!   `LifeguardPoolSettings::wal_lag_poll_interval` (these tests use **25ms** so routing flips to
//!   “healthy” quickly).
//!
//! Remaining variance: first poll happens after `poll_interval` from thread start, TCP connect
//! latency, and CI runner load. Cross‑AZ or WAN replicas are **outside** this crate’s control.

use lifeguard::test_helpers::TestDatabase;
use lifeguard::{LifeExecutor, LifeguardPool, LifeguardPoolSettings, PooledLifeExecutor};
use sea_query::{Value, Values};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const BATCH_LOAD_ROWS: i32 = 64;
const BATCH_LOAD_BASE_ID: i32 = 10_000;

fn replica_url_or_skip() -> Option<String> {
    let ctx = crate::context::get_test_context();
    ctx.replica_pg_url.clone()
}

fn timing_enabled() -> bool {
    std::env::var("LIFEGUARD_POOL_TEST_TIMING")
        .map(|s| {
            let t = s.trim();
            !t.is_empty() && t != "0" && !t.eq_ignore_ascii_case("false")
        })
        .unwrap_or(false)
}

fn log_timing(label: &str, elapsed: Duration) {
    if timing_enabled() {
        eprintln!(
            "[pool_read_replica] {label}: {}.{:03}s",
            elapsed.as_secs(),
            elapsed.subsec_millis()
        );
    }
}

/// Tight integration settings: fast WAL poll so read routing does not wait ~500ms per observation.
fn integration_pool_settings() -> LifeguardPoolSettings {
    LifeguardPoolSettings {
        wal_lag_poll_interval: Duration::from_millis(25),
        ..LifeguardPoolSettings::default()
    }
}

/// Normalized `host:port` so `localhost` and `127.0.0.1` match CI vs local runbooks.
fn pg_tcp_endpoint_key(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("postgres://")
        .or_else(|| url.strip_prefix("postgresql://"))?;
    let after_at = rest.rsplit('@').next()?;
    let host_port = after_at.split('/').next()?;

    if let Some(inner) = host_port.strip_prefix('[') {
        let (addr, port_s) = inner.split_once("]:")?;
        let host = match addr {
            "::1" | "localhost" => "127.0.0.1",
            h => h,
        };
        let port: u16 = port_s.parse().ok()?;
        return Some(format!("{host}:{port}"));
    }

    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) => match p.parse::<u16>() {
            Ok(port) => (h, port),
            Err(_) => (host_port, 5432),
        },
        None => (host_port, 5432),
    };

    let host_norm = match host.to_ascii_lowercase().as_str() {
        "localhost" | "::1" => "127.0.0.1".to_string(),
        h => h.to_string(),
    };
    Some(format!("{host_norm}:{port}"))
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

    let t_total = Instant::now();
    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();

    let ep_p = pg_tcp_endpoint_key(&primary_url).expect("parse primary URL host:port");
    let ep_r = pg_tcp_endpoint_key(&replica_url).expect("parse replica URL host:port");
    assert_ne!(
        ep_p, ep_r,
        "TEST_REPLICA_URL must not target the same host:port as TEST_DATABASE_URL (both {ep_p}). \
         CI sets primary :6543 and replica :6544 — see .github/workflows/ci.yaml and .github/docker/docker-compose.yml."
    );

    let t0 = Instant::now();
    let mut db = TestDatabase::with_url(&primary_url);
    let setup_exec = db.executor().expect("primary executor");
    setup_schema_on_primary(&setup_exec);
    log_timing("setup_schema (primary)", t0.elapsed());

    let pool_settings = integration_pool_settings();
    let t1 = Instant::now();
    let pool = Arc::new(
        LifeguardPool::new_with_settings(
            &primary_url,
            1,
            vec![replica_url.clone()],
            1,
            &pool_settings,
        )
        .expect("LifeguardPool::new_with_settings with replica"),
    );
    log_timing("LifeguardPool::new_with_settings", t1.elapsed());
    let exec = PooledLifeExecutor::new(pool.clone());

    let t_smoke_path = Instant::now();
    let t2 = Instant::now();
    let insert_vals = Values(vec![
        Value::Int(Some(7)),
        Value::String(Some("via-pool".into())),
    ]);
    exec.execute_values(
        "INSERT INTO pool_replica_test.t_pool_replica_smoke (id, note) VALUES ($1, $2)",
        &insert_vals,
    )
    .expect("pooled insert");
    log_timing("pooled INSERT (primary tier)", t2.elapsed());

    let t3 = Instant::now();
    let lsn = crate::replication_sync::primary_current_wal_lsn(&primary_url).expect("primary lsn");
    crate::replication_sync::wait_replica_replayed_at_least(
        &replica_url,
        &lsn,
        Duration::from_secs(45),
        Duration::from_millis(5),
    )
    .expect("replica replay wait");
    log_timing("wait_replica_replayed_at_least", t3.elapsed());

    assert!(
        crate::replication_sync::postgres_is_in_recovery(&replica_url).expect("is_in_recovery query"),
        "TEST_REPLICA_URL must be a standby (pg_is_in_recovery)"
    );

    let t4 = Instant::now();
    let mut lag_ok = false;
    for _ in 0..400 {
        if !pool.is_replica_lagging() {
            lag_ok = true;
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        lag_ok,
        "expected replica not lagging for pool read routing after warmup"
    );
    log_timing("wait_until !is_replica_lagging (poll 5ms)", t4.elapsed());

    let t5 = Instant::now();
    let read_vals = Values(vec![Value::Int(Some(7))]);
    let row = exec
        .query_one_values(
            "SELECT note FROM pool_replica_test.t_pool_replica_smoke WHERE id = $1",
            &read_vals,
        )
        .expect("pooled read");
    let note: String = row.get(0);
    assert_eq!(note, "via-pool");
    log_timing("PooledLifeExecutor read (replica tier when healthy)", t5.elapsed());
    log_timing(
        "SUBTOTAL smoke (insert + replay wait + lag gate + pooled read)",
        t_smoke_path.elapsed(),
    );

    let t6 = Instant::now();
    let rep = may_postgres::connect(&replica_url).expect("replica direct connect");
    let r2 = rep
        .query_one(
            "SELECT note FROM pool_replica_test.t_pool_replica_smoke WHERE id = 7",
            &[],
        )
        .expect("direct replica read");
    let note2: String = r2.get(0);
    assert_eq!(note2, "via-pool");
    log_timing("direct may_postgres read on replica", t6.elapsed());

    // Batch load: one statement, then single replay wait + count on replica.
    let t7 = Instant::now();
    let batch_hi = BATCH_LOAD_BASE_ID + BATCH_LOAD_ROWS - 1;
    let batch_sql = format!(
        "INSERT INTO pool_replica_test.t_pool_replica_smoke (id, note) \
         SELECT g, 'batch-load' FROM generate_series({BATCH_LOAD_BASE_ID}, {batch_hi}) AS g"
    );
    exec.execute_values(&batch_sql, &Values(vec![]))
        .expect("batch insert via pool");
    log_timing(&format!("batch INSERT {BATCH_LOAD_ROWS} rows (primary tier)"), t7.elapsed());

    let t8 = Instant::now();
    let lsn2 =
        crate::replication_sync::primary_current_wal_lsn(&primary_url).expect("primary lsn after batch");
    crate::replication_sync::wait_replica_replayed_at_least(
        &replica_url,
        &lsn2,
        Duration::from_secs(45),
        Duration::from_millis(5),
    )
    .expect("replica replay wait after batch");
    log_timing("wait_replica_replayed_at_least (after batch)", t8.elapsed());

    let t9 = Instant::now();
    let cnt_row = exec
        .query_one_values(
            "SELECT COUNT(*)::bigint AS c FROM pool_replica_test.t_pool_replica_smoke WHERE id >= $1",
            &Values(vec![Value::Int(Some(BATCH_LOAD_BASE_ID))]),
        )
        .expect("count on replica tier");
    let batch_count: i64 = cnt_row.get(0);
    assert_eq!(batch_count, i64::from(BATCH_LOAD_ROWS));
    log_timing("COUNT(*) pooled read (replica tier)", t9.elapsed());

    log_timing("TOTAL (smoke + batch)", t_total.elapsed());
}

#[cfg(test)]
mod pg_endpoint_key_tests {
    use super::pg_tcp_endpoint_key;

    #[test]
    fn normalizes_localhost_and_default_port() {
        assert_eq!(
            pg_tcp_endpoint_key("postgres://u:p@localhost:5432/db").as_deref(),
            Some("127.0.0.1:5432")
        );
        assert_eq!(
            pg_tcp_endpoint_key("postgresql://u:p@127.0.0.1:5433/postgres").as_deref(),
            Some("127.0.0.1:5433")
        );
        assert_eq!(
            pg_tcp_endpoint_key("postgres://u:p@localhost/postgres").as_deref(),
            Some("127.0.0.1:5432")
        );
    }
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
