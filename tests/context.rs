//! Shared Postgres (and optional Redis) URLs for `tests/*.rs` integration binaries.
//!
//! When `DATABASE_URL` / `TEST_DATABASE_URL` is unset, we start Docker containers via
//! testcontainers. Container IDs are registered for removal on process exit using `ctor::dtor`
//! because Rust does not reliably run `Drop` for `static` items at shutdown—`Box::leak` was
//! leaving dozens of Postgres/Redis containers behind after `cargo nextest` runs.

use std::env;
use std::mem;
use std::process::Command;
use std::sync::Mutex;
use testcontainers::clients;
use testcontainers_modules::{postgres::Postgres, redis::Redis};

pub struct LifeguardTestContext {
    pub pg_url: String,
    pub redis_url: String,
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

/// Prefer the same URL sources as `TestDatabase::get_connection_string` so `just nt`
/// (`DATABASE_URL`) and CI Postgres services are used instead of spawning containers per test binary.
fn postgres_url_from_env() -> Option<String> {
    non_empty_env("TEST_DATABASE_URL").or_else(|| non_empty_env("DATABASE_URL"))
}

fn redis_url_from_env() -> Option<String> {
    non_empty_env("TEST_REDIS_URL").or_else(|| non_empty_env("REDIS_URL"))
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
        LifeguardTestContext { pg_url, redis_url }
    } else {
        let (pg_url, redis_url) = start_pg_and_redis_containers();
        LifeguardTestContext { pg_url, redis_url }
    }
});

#[must_use] pub fn get_test_context() -> &'static LifeguardTestContext {
    &TEST_CONTEXT
}

// Helper to clean the database before a test if needed
pub fn clean_db(pg_url: &str, tables: &[&str]) {
    if let Ok(client) = may_postgres::connect(pg_url) {
        for table in tables {
            let _ = client.execute(format!("DROP TABLE IF EXISTS {table} CASCADE;").as_str(), &[]);
        }
    }
}
