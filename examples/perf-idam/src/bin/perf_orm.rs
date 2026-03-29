//! ORM performance harness: IDAM-shaped reads/writes via [`MayPostgresExecutor`].
//!
//! Environment:
//! - `PERF_DATABASE_URL` (preferred), else `DATABASE_URL`, else `TEST_DATABASE_URL`.
//! - `PERF_TENANT_COUNT` (default 10), `PERF_USER_ROWS`, `PERF_SESSION_ROWS` (default 5000 each).
//! - `PERF_WARMUP` (default 200), `PERF_ITERATIONS` (default 2000).
//! - `PERF_OUTPUT` — if set, write JSON to this path; otherwise stdout.
//!
//! Metrics include `connections: 1` until a Lifeguard pool exists (Epic 04).

use lifeguard::connection::connect;
use lifeguard::executor::MayPostgresExecutor;
use lifeguard::query::column::column_trait::ColumnTrait;
use lifeguard::{LifeExecutor, LifeModelTrait};
use perf_idam::perf_idam::{perf_session, perf_tenant, perf_user};
use serde::Serialize;
use std::env;
use std::fs;
use std::time::Instant;

const SCHEMA_SQL: &str = include_str!("../../migrations/schema.sql");

#[derive(Serialize)]
struct PerfReport {
    connections: u32,
    database_url_host: String,
    scale: Scale,
    warmup_iterations: usize,
    measured_iterations: usize,
    scenarios: Vec<ScenarioStats>,
}

#[derive(Serialize)]
struct Scale {
    tenants: usize,
    users: usize,
    sessions: usize,
}

#[derive(Serialize)]
struct ScenarioStats {
    name: &'static str,
    iterations: usize,
    mean_us: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    throughput_per_s: f64,
}

fn env_usize(key: &str, default: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn database_url() -> Result<String, String> {
    env::var("PERF_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .or_else(|_| env::var("TEST_DATABASE_URL"))
        .map_err(|_| {
            "Set PERF_DATABASE_URL, DATABASE_URL, or TEST_DATABASE_URL".to_string()
        })
}

fn redacted_host(url: &str) -> String {
    // Avoid printing credentials: keep scheme and host-ish prefix only.
    if let Some(after) = url.strip_prefix("postgres://") {
        if let Some(at) = after.find('@') {
            let rest = &after[at + 1..];
            return format!("postgres://***@{rest}");
        }
    }
    if let Some(after) = url.strip_prefix("postgresql://") {
        if let Some(at) = after.find('@') {
            let rest = &after[at + 1..];
            return format!("postgresql://***@{rest}");
        }
    }
    "postgres://***".to_string()
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let len = sorted.len();
    if len == 0 {
        return 0.0;
    }
    let idx = ((len as f64 - 1.0) * p / 100.0).round() as usize;
    sorted[idx.min(len - 1)]
}

fn stats(name: &'static str, samples: &mut [f64]) -> ScenarioStats {
    let iterations = samples.len();
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let sum: f64 = samples.iter().sum();
    let mean = if iterations > 0 {
        sum / iterations as f64
    } else {
        0.0
    };
    let p50 = percentile(samples, 50.0);
    let p95 = percentile(samples, 95.0);
    let p99 = percentile(samples, 99.0);
    let throughput_per_s = if mean > 0.0 {
        1_000_000.0 / mean
    } else {
        0.0
    };
    ScenarioStats {
        name,
        iterations,
        mean_us: mean,
        p50_us: p50,
        p95_us: p95,
        p99_us: p99,
        throughput_per_s,
    }
}

fn apply_schema(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    let mut buf = String::new();
    for line in SCHEMA_SQL.lines() {
        let t = line.trim_start();
        if t.starts_with("--") {
            continue;
        }
        buf.push_str(line);
        buf.push('\n');
    }
    for stmt in buf.split(';') {
        let s = stmt.trim();
        if s.is_empty() {
            continue;
        }
        let sql = format!("{s};");
        executor.execute(&sql, &[])?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = database_url()?;
    let host_display = redacted_host(&url);

    let tenant_n = env_usize("PERF_TENANT_COUNT", 10).max(1);
    let user_n = env_usize("PERF_USER_ROWS", 5000).max(1);
    let session_n = env_usize("PERF_SESSION_ROWS", 5000).max(1);
    let warmup = env_usize("PERF_WARMUP", 200);
    let iters = env_usize("PERF_ITERATIONS", 2000).max(1);

    let client = connect(&url)?;
    let executor = MayPostgresExecutor::new(client);

    apply_schema(&executor)?;

    // --- Seed tenants ---
    let mut tenant_ids = Vec::with_capacity(tenant_n);
    for i in 0..tenant_n {
        let tid = uuid::Uuid::new_v4();
        let mut tr = perf_tenant::PerfTenantRecord::new();
        tr.set_id(tid);
        tr.set_name(format!("tenant_{i}"));
        perf_tenant::Entity::insert(tr, &executor)?;
        tenant_ids.push(tid);
    }

    // --- Seed users (one session per user row up to session_n) ---
    let mut user_ids: Vec<uuid::Uuid> = Vec::with_capacity(user_n);
    let mut tenant_email_keys: Vec<(uuid::Uuid, String)> = Vec::with_capacity(user_n);
    let base = chrono::Utc::now().naive_utc();
    let expires = base + chrono::Duration::days(30);

    for i in 0..user_n {
        let uid = uuid::Uuid::new_v4();
        let tid = tenant_ids[i % tenant_n];
        let email = format!("user{i}@perf.example");
        let mut ur = perf_user::PerfUserRecord::new();
        ur.set_id(uid);
        ur.set_tenant_id(tid);
        ur.set_email(email.clone());
        ur.set_display_name(format!("User {i}"));
        ur.set_created_at(base);
        perf_user::Entity::insert(ur, &executor)?;
        user_ids.push(uid);
        tenant_email_keys.push((tid, email));
    }

    let mut token_fps: Vec<String> = Vec::with_capacity(session_n);
    let mut session_ids: Vec<uuid::Uuid> = Vec::with_capacity(session_n);

    for i in 0..session_n {
        let sid = uuid::Uuid::new_v4();
        let uid = user_ids[i % user_n];
        let fp = format!("tf_{i:012x}");
        let mut sr = perf_session::PerfSessionRecord::new();
        sr.set_id(sid);
        sr.set_user_id(uid);
        sr.set_token_fingerprint(fp.clone());
        sr.set_expires_at(expires);
        sr.set_last_seen_at(base);
        perf_session::Entity::insert(sr, &executor)?;
        token_fps.push(fp);
        session_ids.push(sid);
    }

    // --- Warmup ---
    for w in 0..warmup {
        let i = w % user_n;
        let _ = perf_user::Entity::find()
            .filter(perf_user::Column::Id.eq(user_ids[i]))
            .find_one(&executor)?;
        let (tid, ref em) = tenant_email_keys[i];
        let _ = perf_user::Entity::find()
            .filter(perf_user::Column::TenantId.eq(tid))
            .filter(perf_user::Column::Email.eq(em.as_str()))
            .find_one(&executor)?;
        let _ = perf_session::Entity::find()
            .filter(perf_session::Column::TokenFingerprint.eq(token_fps[i % session_n].as_str()))
            .find_one(&executor)?;
    }

    let mut scenarios = Vec::with_capacity(4);

    // 1) User by PK
    let mut samples = Vec::with_capacity(iters);
    for k in 0..iters {
        let i = (k * 2654435761usize) % user_n;
        let t0 = Instant::now();
        let _ = perf_user::Entity::find()
            .filter(perf_user::Column::Id.eq(user_ids[i]))
            .find_one(&executor)?;
        samples.push(t0.elapsed().as_micros() as f64);
    }
    scenarios.push(stats("user_by_pk", &mut samples));

    // 2) User by (tenant_id, email)
    let mut samples = Vec::with_capacity(iters);
    for k in 0..iters {
        let i = (k * 2654435761usize) % user_n;
        let (tid, ref em) = tenant_email_keys[i];
        let t0 = Instant::now();
        let _ = perf_user::Entity::find()
            .filter(perf_user::Column::TenantId.eq(tid))
            .filter(perf_user::Column::Email.eq(em.as_str()))
            .find_one(&executor)?;
        samples.push(t0.elapsed().as_micros() as f64);
    }
    scenarios.push(stats("user_by_tenant_and_email", &mut samples));

    // 3) Session by token_fingerprint
    let mut samples = Vec::with_capacity(iters);
    for k in 0..iters {
        let i = (k * 2654435761usize) % session_n;
        let t0 = Instant::now();
        let _ = perf_session::Entity::find()
            .filter(perf_session::Column::TokenFingerprint.eq(token_fps[i].as_str()))
            .find_one(&executor)?;
        samples.push(t0.elapsed().as_micros() as f64);
    }
    scenarios.push(stats("session_by_token_fingerprint", &mut samples));

    // 4) Update last_seen_at (session touch)
    let mut samples = Vec::with_capacity(iters);
    for k in 0..iters {
        let i = (k * 2654435761usize) % session_n;
        let sid = session_ids[i];
        let t0 = Instant::now();
        let model = perf_session::Entity::find()
            .filter(perf_session::Column::Id.eq(sid))
            .find_one(&executor)?
            .ok_or("session row missing")?;
        let mut rec = perf_session::PerfSessionRecord::from_model(&model);
        rec.set_last_seen_at(base + chrono::Duration::milliseconds(k as i64));
        let _ = perf_session::Entity::update(rec, &executor)?;
        samples.push(t0.elapsed().as_micros() as f64);
    }
    scenarios.push(stats("session_update_last_seen", &mut samples));

    let report = PerfReport {
        connections: 1,
        database_url_host: host_display,
        scale: Scale {
            tenants: tenant_n,
            users: user_n,
            sessions: session_n,
        },
        warmup_iterations: warmup,
        measured_iterations: iters,
        scenarios,
    };

    let json = serde_json::to_string_pretty(&report)?;
    if let Ok(path) = env::var("PERF_OUTPUT") {
        if !path.is_empty() {
            fs::write(path, json)?;
            return Ok(());
        }
    }
    println!("{json}");
    Ok(())
}
