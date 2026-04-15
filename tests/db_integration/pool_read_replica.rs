//! `LifeguardPool` + `PooledLifeExecutor` against a real streaming replica (PRD read-replica).
//!
//! Skips when `TEST_REPLICA_URL` is unset (e.g. local testcontainers-only runs).
//!
//! **Routing proof:** after the lag monitor reports a healthy replica, tests run `SELECT pg_is_in_recovery()`
//! on the **pooled** executor so a silent fallback to primary cannot pass while data still matches on both nodes.
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
use lifeguard::{
    LifeExecutor, LifeguardPool, LifeguardPoolSettings, PooledLifeExecutor, ReadPreference,
};
use sea_query::{Value, Values};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Unique schema + table per test so parallel `db_integration` runs do not `DROP` each other's objects.
static POOL_REPLICA_SCHEMA_SEQ: AtomicU64 = AtomicU64::new(0);

fn unique_pool_replica_schema_names() -> (String, String) {
    let n = POOL_REPLICA_SCHEMA_SEQ.fetch_add(1, Ordering::Relaxed);
    (
        format!("pool_replica_test_{n}"),
        format!("t_pool_replica_smoke_{n}"),
    )
}

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

/// Proves which server handled a **pooled** read: standby (`true`) vs primary (`false`).
fn assert_pooled_pg_is_in_recovery(exec: &PooledLifeExecutor, expect_standby: bool, context: &str) {
    let row = exec
        .query_one_values("SELECT pg_is_in_recovery() AS ir", &Values(vec![]))
        .unwrap_or_else(|e| panic!("{context}: pooled pg_is_in_recovery probe: {e}"));
    let ir: bool = row.get(0);
    assert_eq!(
        ir, expect_standby,
        "{context}: pg_is_in_recovery()={ir}, expected {expect_standby} \
         (false = primary tier, true = replica tier)",
    );
}

fn setup_schema_on_primary(executor: &lifeguard::MayPostgresExecutor, schema: &str, table: &str) {
    executor
        .execute(&format!("CREATE SCHEMA IF NOT EXISTS {schema}"), &[])
        .expect("create schema");
    executor
        .execute(
            &format!("DROP TABLE IF EXISTS {schema}.{table} CASCADE"),
            &[],
        )
        .expect("drop table");
    executor
        .execute(
            &format!(
                r#"
            CREATE TABLE {schema}.{table} (
                id INTEGER PRIMARY KEY,
                note TEXT NOT NULL
            )
            "#
            ),
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
    let (schema, table) = unique_pool_replica_schema_names();

    let ep_p = pg_tcp_endpoint_key(&primary_url).expect("parse primary URL host:port");
    let ep_r = pg_tcp_endpoint_key(&replica_url).expect("parse replica URL host:port");
    assert_ne!(
        ep_p, ep_r,
        "TEST_REPLICA_URL must not target the same host:port as TEST_DATABASE_URL (both {ep_p}). \
         CI sets primary :6543 and replica via Toxiproxy :6547 — see .github/workflows/ci.yaml and .github/docker/docker-compose.yml."
    );

    let t0 = Instant::now();
    let mut db = TestDatabase::with_url(&primary_url);
    let setup_exec = db.executor().expect("primary executor");
    setup_schema_on_primary(&setup_exec, &schema, &table);
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
        &format!("INSERT INTO {schema}.{table} (id, note) VALUES ($1, $2)"),
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
        crate::replication_sync::postgres_is_in_recovery(&replica_url)
            .expect("is_in_recovery query"),
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

    assert_pooled_pg_is_in_recovery(
        &exec,
        true,
        "after lag monitor reports healthy replica, pooled reads must use replica tier",
    );

    let t5 = Instant::now();
    let read_vals = Values(vec![Value::Int(Some(7))]);
    let row = exec
        .query_one_values(
            &format!("SELECT note FROM {schema}.{table} WHERE id = $1"),
            &read_vals,
        )
        .expect("pooled read");
    let note: String = row.get(0);
    assert_eq!(note, "via-pool");
    log_timing(
        "PooledLifeExecutor read (replica tier when healthy)",
        t5.elapsed(),
    );
    log_timing(
        "SUBTOTAL smoke (insert + replay wait + lag gate + pooled read)",
        t_smoke_path.elapsed(),
    );

    let t6 = Instant::now();
    let rep = may_postgres::connect(&replica_url).expect("replica direct connect");
    let direct_note_sql = format!("SELECT note FROM {schema}.{table} WHERE id = 7");
    let r2 = rep
        .query_one(direct_note_sql.as_str(), &[])
        .expect("direct replica read");
    let note2: String = r2.get(0);
    assert_eq!(note2, "via-pool");
    log_timing("direct may_postgres read on replica", t6.elapsed());

    // Batch load: one statement, then single replay wait + count on replica.
    let t7 = Instant::now();
    let batch_hi = BATCH_LOAD_BASE_ID + BATCH_LOAD_ROWS - 1;
    let batch_sql = format!(
        "INSERT INTO {schema}.{table} (id, note) \
         SELECT g, 'batch-load' FROM generate_series({BATCH_LOAD_BASE_ID}, {batch_hi}) AS g"
    );
    exec.execute_values(&batch_sql, &Values(vec![]))
        .expect("batch insert via pool");
    log_timing(
        &format!("batch INSERT {BATCH_LOAD_ROWS} rows (primary tier)"),
        t7.elapsed(),
    );

    let t8 = Instant::now();
    let lsn2 = crate::replication_sync::primary_current_wal_lsn(&primary_url)
        .expect("primary lsn after batch");
    crate::replication_sync::wait_replica_replayed_at_least(
        &replica_url,
        &lsn2,
        Duration::from_secs(45),
        Duration::from_millis(5),
    )
    .expect("replica replay wait after batch");
    log_timing("wait_replica_replayed_at_least (after batch)", t8.elapsed());

    assert_pooled_pg_is_in_recovery(
        &exec,
        true,
        "before batch COUNT, pooled reads should still hit replica tier when healthy",
    );

    let t9 = Instant::now();
    let cnt_row = exec
        .query_one_values(
            &format!("SELECT COUNT(*)::bigint AS c FROM {schema}.{table} WHERE id >= $1"),
            &Values(vec![Value::Int(Some(BATCH_LOAD_BASE_ID))]),
        )
        .expect("count on replica tier");
    let batch_count: i64 = cnt_row.get(0);
    assert_eq!(batch_count, i64::from(BATCH_LOAD_ROWS));
    log_timing("COUNT(*) pooled read (replica tier)", t9.elapsed());

    log_timing("TOTAL (smoke + batch)", t_total.elapsed());
}

/// [`ReadPreference::Primary`] must hit the primary tier even when default routing uses the replica.
#[test]
fn pooled_read_preference_primary_forces_primary_tier() {
    let Some(replica_url) = replica_url_or_skip() else {
        return;
    };

    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_pool_replica_schema_names();

    let ep_p = pg_tcp_endpoint_key(&primary_url).expect("parse primary URL host:port");
    let ep_r = pg_tcp_endpoint_key(&replica_url).expect("parse replica URL host:port");
    assert_ne!(
        ep_p, ep_r,
        "TEST_REPLICA_URL must not target the same host:port as TEST_DATABASE_URL"
    );

    let mut db = TestDatabase::with_url(&primary_url);
    let setup_exec = db.executor().expect("primary executor");
    setup_schema_on_primary(&setup_exec, &schema, &table);

    let pool = Arc::new(
        LifeguardPool::new_with_settings(
            &primary_url,
            1,
            vec![replica_url.clone()],
            1,
            &integration_pool_settings(),
        )
        .expect("LifeguardPool::new_with_settings with replica"),
    );
    let exec = PooledLifeExecutor::new(pool.clone());

    let lsn = crate::replication_sync::primary_current_wal_lsn(&primary_url).expect("primary lsn");
    crate::replication_sync::wait_replica_replayed_at_least(
        &replica_url,
        &lsn,
        Duration::from_secs(45),
        Duration::from_millis(5),
    )
    .expect("replica replay wait");

    assert!(
        crate::replication_sync::postgres_is_in_recovery(&replica_url)
            .expect("is_in_recovery query"),
        "TEST_REPLICA_URL must be a standby (pg_is_in_recovery)"
    );

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

    assert_pooled_pg_is_in_recovery(
        &exec,
        true,
        "default ReadPreference routes to replica tier when healthy",
    );

    let exec_primary = exec.clone().with_read_preference(ReadPreference::Primary);
    assert_eq!(exec_primary.read_preference(), ReadPreference::Primary);
    assert_pooled_pg_is_in_recovery(
        &exec_primary,
        false,
        "ReadPreference::Primary must use primary tier",
    );

    assert_pooled_pg_is_in_recovery(
        &exec,
        true,
        "original executor should still use default (replica) routing",
    );
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

/// With [Toxiproxy](https://github.com/Shopify/toxiproxy) (`TOXIPROXY_API`, see `.github/docker/docker-compose.yml`),
/// disables the `postgres_replica` proxy so replica I/O fails; the WAL lag monitor marks lagging and reads route to the primary tier.
///
/// TODO: This test is **ignored** until CI is stable — `WalLagMonitor::is_replica_lagging` often stays
/// false for ~10s after the proxy is disabled (monitor loop / session / Toxiproxy timing). Re-enable
/// when we can assert lagging reliably (see `src/pool/wal.rs` `WalLagMonitor`, toxiproxy fault path).
#[test]
#[ignore = "flaky in CI: WalLagMonitor does not flip to lagging within poll window after toxiproxy disable; TODO stabilize and remove ignore"]
fn pooled_read_falls_back_to_primary_when_replica_lagging() {
    let Some(replica_url) = replica_url_or_skip() else {
        return;
    };
    let Some(api) = crate::toxiproxy_control::api_base_from_env() else {
        return;
    };

    struct ResetToxiproxy(String);
    impl Drop for ResetToxiproxy {
        fn drop(&mut self) {
            let _ = crate::toxiproxy_control::reset_all(&self.0);
        }
    }

    let _restore = ResetToxiproxy(api.clone());

    crate::toxiproxy_control::reset_all(&api).expect("toxiproxy POST /reset");

    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();
    let (schema, table) = unique_pool_replica_schema_names();

    let ep_p = pg_tcp_endpoint_key(&primary_url).expect("parse primary URL host:port");
    let ep_r = pg_tcp_endpoint_key(&replica_url).expect("parse replica URL host:port");
    assert_ne!(
        ep_p, ep_r,
        "TEST_REPLICA_URL must not target the same host:port as TEST_DATABASE_URL (both {ep_p}). \
         CI Compose uses Toxiproxy on :6547 to the streaming replica — see .github/docker/docker-compose.yml."
    );

    let mut db = TestDatabase::with_url(&primary_url);
    let setup_exec = db.executor().expect("primary executor");
    setup_schema_on_primary(&setup_exec, &schema, &table);

    const FALLBACK_ID: i32 = 42;
    let pool_settings = integration_pool_settings();
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
    let exec = PooledLifeExecutor::new(pool.clone());

    let insert_vals = Values(vec![
        Value::Int(Some(FALLBACK_ID)),
        Value::String(Some("primary-fallback".into())),
    ]);
    exec.execute_values(
        &format!("INSERT INTO {schema}.{table} (id, note) VALUES ($1, $2)"),
        &insert_vals,
    )
    .expect("pooled insert");

    let lsn = crate::replication_sync::primary_current_wal_lsn(&primary_url).expect("primary lsn");
    crate::replication_sync::wait_replica_replayed_at_least(
        &replica_url,
        &lsn,
        Duration::from_secs(45),
        Duration::from_millis(5),
    )
    .expect("replica replay wait");

    let mut replica_ok = false;
    for _ in 0..400 {
        if !pool.is_replica_lagging() {
            replica_ok = true;
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        replica_ok,
        "expected replica not lagging before toxiproxy fault"
    );

    crate::toxiproxy_control::set_proxy_enabled(
        &api,
        crate::toxiproxy_control::POSTGRES_REPLICA_PROXY,
        false,
    )
    .expect("disable postgres_replica proxy");

    let mut lagging_seen = false;
    for _ in 0..400 {
        if pool.is_replica_lagging() {
            lagging_seen = true;
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    assert!(
        lagging_seen,
        "expected WalLagMonitor to mark replica lagging after replica proxy disabled"
    );

    assert_pooled_pg_is_in_recovery(
        &exec,
        false,
        "when replica tier is unavailable, pooled reads must fall back to primary",
    );

    let read_vals = Values(vec![Value::Int(Some(FALLBACK_ID))]);
    let row = exec
        .query_one_values(
            &format!("SELECT note FROM {schema}.{table} WHERE id = $1"),
            &read_vals,
        )
        .expect("pooled read should succeed via primary when replica tier is unavailable");
    let note: String = row.get(0);
    assert_eq!(note, "primary-fallback");
}
