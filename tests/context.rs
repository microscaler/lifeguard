//! Shared Postgres (and optional Redis) URLs for `tests/*.rs` integration binaries.
//!
//! When **`TEST_REPLICA_URL`** is set (non-empty), it must point at a streaming standby for the
//! same cluster as **`TEST_DATABASE_URL`** (primary). Used by read-replica pool tests; unset
//! locally skips those tests. See `docs/planning/PRD_READ_REPLICA_TESTING.md`.
//!
//! When **`TEST_DATABASE_URL`** is unset, we start Docker containers via testcontainers.
//! We intentionally do **not** read `DATABASE_URL` here: integration helpers such as
//! [`clean_db`] issue destructive SQL (`DROP TABLE ... CASCADE`), and `DATABASE_URL` is commonly
//! set to a developer or app database. Require an explicit test URL instead.
//!
//! Container IDs are registered for removal on process exit using `ctor::dtor` because Rust does
//! not reliably run `Drop` for `static` items at shutdown—`Box::leak` was leaving dozens of
//! Postgres/Redis containers behind after `cargo nextest` runs.

use std::env;
use std::mem;
use std::process::Command;
use std::sync::Mutex;
use testcontainers::clients;
use testcontainers_modules::{postgres::Postgres, redis::Redis};

pub struct LifeguardTestContext {
    /// Primary (read-write) connection string.
    pub pg_url: String,
    pub redis_url: String,
    /// Standby URL when `TEST_REPLICA_URL` is set; `None` if unset or empty.
    pub replica_pg_url: Option<String>,
}

static DOCKER_CONTAINER_IDS: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn record_container_ids_for_shutdown(ids: Vec<String>) {
    let mut guard = DOCKER_CONTAINER_IDS
        .lock()
        .expect("docker container id registry poisoned");
    guard.extend(ids);
}

/// Runs when the integration test **binary** exits (each `tests/*.rs` crate is its own process).
#[ctor::dtor]
fn remove_testcontainer_sidecars() {
    let ids: Vec<String> = match DOCKER_CONTAINER_IDS.lock() {
        Ok(mut g) => mem::take(&mut *g),
        Err(e) => mem::take(&mut *e.into_inner()),
    };
    for id in ids {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        let status = Command::new("docker").args(["rm", "-f", id]).status();
        if let Err(e) = status {
            eprintln!("lifeguard tests: failed to remove docker container {id:?}: {e}");
        }
    }
}

fn non_empty_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|s| !s.trim().is_empty())
}

/// Only **`TEST_DATABASE_URL`** (see module docs — no `DATABASE_URL` fallback).
fn postgres_url_from_env() -> Option<String> {
    non_empty_env("TEST_DATABASE_URL")
}

fn redis_url_from_env() -> Option<String> {
    non_empty_env("TEST_REDIS_URL").or_else(|| non_empty_env("REDIS_URL"))
}

fn replica_url_from_env() -> Option<String> {
    non_empty_env("TEST_REPLICA_URL")
}

/// Start Postgres and Redis via testcontainers; register IDs for `remove_testcontainer_sidecars`.
fn start_pg_and_redis_containers() -> (String, String) {
    let mut ids = Vec::with_capacity(2);

    let cli_pg = clients::Cli::default();
    let pg = cli_pg.run(Postgres::default());
    let pg_port = pg.get_host_port_ipv4(5432);
    ids.push(pg.id().to_string());
    mem::forget(pg);
    mem::forget(cli_pg);

    let cli_redis = clients::Cli::default();
    let redis = cli_redis.run(Redis);
    let redis_port = redis.get_host_port_ipv4(6379);
    ids.push(redis.id().to_string());
    mem::forget(redis);
    mem::forget(cli_redis);

    record_container_ids_for_shutdown(ids);

    let pg_url = format!("postgres://postgres:postgres@127.0.0.1:{pg_port}/postgres");
    let redis_url = format!("redis://127.0.0.1:{redis_port}");

    (pg_url, redis_url)
}

pub static TEST_CONTEXT: std::sync::LazyLock<LifeguardTestContext> = std::sync::LazyLock::new(|| {
    if let Some(pg_url) = postgres_url_from_env() {
        let redis_url = redis_url_from_env()
            .unwrap_or_else(|| "redis://127.0.0.1:6379".to_string());
        let replica_pg_url = replica_url_from_env();
        LifeguardTestContext {
            pg_url,
            redis_url,
            replica_pg_url,
        }
    } else {
        let (pg_url, redis_url) = start_pg_and_redis_containers();
        LifeguardTestContext {
            pg_url,
            redis_url,
            replica_pg_url: None,
        }
    }
});

#[must_use] pub fn get_test_context() -> &'static LifeguardTestContext {
    &TEST_CONTEXT
}

/// Drops tables with `CASCADE`. Only use with a URL from [`get_test_context`] (i.e.
/// `TEST_DATABASE_URL` or an isolated testcontainer), never a production `DATABASE_URL`.
pub fn clean_db(pg_url: &str, tables: &[&str]) {
    if let Ok(client) = may_postgres::connect(pg_url) {
        for table in tables {
            let _ = client.execute(format!("DROP TABLE IF EXISTS {table} CASCADE;").as_str(), &[]);
        }
    }
}
