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

    let ctx = crate::context::get_test_context();
    let primary_url = ctx.pg_url.clone();

    let ep_p = pg_tcp_endpoint_key(&primary_url).expect("parse primary URL host:port");
    let ep_r = pg_tcp_endpoint_key(&replica_url).expect("parse replica URL host:port");
    assert_ne!(
        ep_p, ep_r,
        "TEST_REPLICA_URL must not target the same host:port as TEST_DATABASE_URL (both {ep_p}). \
         CI sets primary :5432 and replica :5433 — see .github/workflows/ci.yaml and .github/docker/docker-compose.yml."
    );

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
