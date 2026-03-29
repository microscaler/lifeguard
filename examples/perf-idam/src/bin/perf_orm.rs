//! ORM performance harness: IDAM-shaped reads/writes via [`lifeguard::PooledLifeExecutor`]
//! (round-robin over a [`lifeguard::LifeguardPool`]).
//!
//! Environment:
//! - `PERF_DATABASE_URL` or `TEST_DATABASE_URL` (in that order). Generic `DATABASE_URL` is **not** read:
//!   this binary runs destructive DDL (`DROP` / `CREATE` on `perf_*` tables).
//! - `PERF_RESET` — must be truthy (`1`, `true`, `yes`, `on`) before any schema reset; avoids accidents.
//! - `PERF_POOL_SIZE` (default 8) — concurrent `may_postgres::Client` slots in the pool.
//! - `PERF_TENANT_COUNT` (default 10), `PERF_USER_ROWS`, `PERF_SESSION_ROWS` (default 5000 each).
//! - `PERF_WARMUP` (default 200), `PERF_ITERATIONS` (default 2000).
//! - `PERF_OUTPUT` — if set, write JSON to this path; otherwise stdout.
//!
//! The JSON report includes `connections` equal to `PERF_POOL_SIZE` for downstream dashboards.

use lifeguard::query::column::column_trait::ColumnTrait;
use lifeguard::{LifeExecutor, LifeModelTrait, LifeguardPool, PooledLifeExecutor};
use perf_idam::perf_idam::{perf_session, perf_tenant, perf_user};
use serde::Serialize;
use std::env;
use std::fs;
use std::sync::Arc;
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

fn resolve_database_url_values(
    perf_database_url: Option<String>,
    test_database_url: Option<String>,
) -> Result<String, String> {
    if let Some(url) = perf_database_url.filter(|s| !s.trim().is_empty()) {
        return Ok(url);
    }
    if let Some(url) = test_database_url.filter(|s| !s.trim().is_empty()) {
        return Ok(url);
    }
    Err(
        "Set PERF_DATABASE_URL or TEST_DATABASE_URL. Generic DATABASE_URL is not accepted: perf-orm applies destructive DDL to perf_* tables.".to_string(),
    )
}

fn database_url() -> Result<String, String> {
    resolve_database_url_values(
        env::var("PERF_DATABASE_URL").ok(),
        env::var("TEST_DATABASE_URL").ok(),
    )
}

/// `PERF_RESET` must be one of `1`, `true`, `yes`, `on` (ASCII case-insensitive).
fn perf_reset_acknowledged_from_var(raw: Option<String>) -> bool {
    let Some(s) = raw else {
        return false;
    };
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn perf_reset_acknowledged() -> bool {
    perf_reset_acknowledged_from_var(env::var("PERF_RESET").ok())
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

/// `find_one` must return a row; otherwise the harness would record timings for broken lookups.
fn require_row<T>(context: &'static str, row: Option<T>) -> Result<T, Box<dyn std::error::Error>> {
    row.ok_or_else(|| format!("{context}: expected row, got None").into())
}

/// Strip whole-line `--` comments (harness `schema.sql` style). Inline `--` is not removed.
fn schema_sql_without_line_comments(sql: &str) -> String {
    sql.lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Split on `;` only **outside** single-quoted literals. `''` is the escaped quote inside a string.
///
/// Sufficient for DDL used in this harness. **Not** a full SQL lexer: dollar-quoted strings (`$$`),
/// standard-conforming-strings edge cases, and procedural bodies are unsupported—keep `schema.sql` as plain DDL.
fn split_postgres_statements(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut in_quote = false;

    while i < bytes.len() {
        let b = bytes[i];
        if in_quote {
            if b == b'\'' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                in_quote = false;
            }
            i += 1;
            continue;
        }
        match b {
            b'\'' => {
                in_quote = true;
                i += 1;
            }
            b';' => {
                let part = sql[start..i].trim();
                if !part.is_empty() {
                    out.push(part.to_string());
                }
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    let tail = sql[start..].trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

fn apply_schema(executor: &impl LifeExecutor) -> Result<(), lifeguard::executor::LifeError> {
    let script = schema_sql_without_line_comments(SCHEMA_SQL);
    for stmt in split_postgres_statements(&script) {
        let sql = format!("{stmt};");
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
    let pool_size = env_usize("PERF_POOL_SIZE", 8).max(1);
    let warmup = env_usize("PERF_WARMUP", 200);
    let iters = env_usize("PERF_ITERATIONS", 2000).max(1);

    if !perf_reset_acknowledged() {
        return Err(
            "Refusing to run: set PERF_RESET=1 (or true/yes/on) after confirming a disposable database. \
             This binary DROPs/recreates perf_* tables. Only PERF_DATABASE_URL and TEST_DATABASE_URL are used (not DATABASE_URL)."
                .into(),
        );
    }

    let pool = Arc::new(LifeguardPool::new(&url, pool_size)?);
    let executor = PooledLifeExecutor::new(pool);

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
    // Distinct `expires_at` per row so equality on `NaiveDateTime` returns a single session (selective).
    let mut session_expires_at: Vec<chrono::NaiveDateTime> = Vec::with_capacity(session_n);

    for i in 0..session_n {
        let sid = uuid::Uuid::new_v4();
        let uid = user_ids[i % user_n];
        let fp = format!("tf_{i:012x}");
        let expires_at = base + chrono::Duration::seconds((i as i64).saturating_add(1));
        let mut sr = perf_session::PerfSessionRecord::new();
        sr.set_id(sid);
        sr.set_user_id(uid);
        sr.set_token_fingerprint(fp.clone());
        sr.set_expires_at(expires_at);
        sr.set_last_seen_at(base);
        perf_session::Entity::insert(sr, &executor)?;
        token_fps.push(fp);
        session_ids.push(sid);
        session_expires_at.push(expires_at);
    }

    // --- Warmup ---
    for w in 0..warmup {
        let i = w % user_n;
        require_row(
            "warmup user_by_pk",
            perf_user::Entity::find()
                .filter(perf_user::Column::Id.eq(user_ids[i]))
                .find_one(&executor)?,
        )?;
        let (tid, ref em) = tenant_email_keys[i];
        require_row(
            "warmup user_by_tenant_and_email",
            perf_user::Entity::find()
                .filter(perf_user::Column::TenantId.eq(tid))
                .filter(perf_user::Column::Email.eq(em.as_str()))
                .find_one(&executor)?,
        )?;
        require_row(
            "warmup session_by_token_fingerprint",
            perf_session::Entity::find()
                .filter(perf_session::Column::TokenFingerprint.eq(token_fps[i % session_n].as_str()))
                .find_one(&executor)?,
        )?;
        require_row(
            "warmup session_by_expires_at",
            perf_session::Entity::find()
                .filter(perf_session::Column::ExpiresAt.eq(session_expires_at[w % session_n]))
                .find_one(&executor)?,
        )?;
    }

    let mut scenarios = Vec::with_capacity(5);

    // 1) User by PK
    let mut samples = Vec::with_capacity(iters);
    for k in 0..iters {
        let i = (k * 2654435761usize) % user_n;
        let t0 = Instant::now();
        require_row(
            "scenario user_by_pk",
            perf_user::Entity::find()
                .filter(perf_user::Column::Id.eq(user_ids[i]))
                .find_one(&executor)?,
        )?;
        samples.push(t0.elapsed().as_micros() as f64);
    }
    scenarios.push(stats("user_by_pk", &mut samples));

    // 2) User by (tenant_id, email)
    let mut samples = Vec::with_capacity(iters);
    for k in 0..iters {
        let i = (k * 2654435761usize) % user_n;
        let (tid, ref em) = tenant_email_keys[i];
        let t0 = Instant::now();
        require_row(
            "scenario user_by_tenant_and_email",
            perf_user::Entity::find()
                .filter(perf_user::Column::TenantId.eq(tid))
                .filter(perf_user::Column::Email.eq(em.as_str()))
                .find_one(&executor)?,
        )?;
        samples.push(t0.elapsed().as_micros() as f64);
    }
    scenarios.push(stats("user_by_tenant_and_email", &mut samples));

    // 3) Session by token_fingerprint
    let mut samples = Vec::with_capacity(iters);
    for k in 0..iters {
        let i = (k * 2654435761usize) % session_n;
        let t0 = Instant::now();
        require_row(
            "scenario session_by_token_fingerprint",
            perf_session::Entity::find()
                .filter(perf_session::Column::TokenFingerprint.eq(token_fps[i].as_str()))
                .find_one(&executor)?,
        )?;
        samples.push(t0.elapsed().as_micros() as f64);
    }
    scenarios.push(stats("session_by_token_fingerprint", &mut samples));

    // 4) Session by expires_at — exercises `NaiveDateTime` in a WHERE bind (read path).
    let mut samples = Vec::with_capacity(iters);
    for k in 0..iters {
        let i = (k * 2654435761usize) % session_n;
        let exp = session_expires_at[i];
        let t0 = Instant::now();
        require_row(
            "scenario session_by_expires_at",
            perf_session::Entity::find()
                .filter(perf_session::Column::ExpiresAt.eq(exp))
                .find_one(&executor)?,
        )?;
        samples.push(t0.elapsed().as_micros() as f64);
    }
    scenarios.push(stats("session_by_expires_at", &mut samples));

    // 5) Update last_seen_at (session touch)
    let mut samples = Vec::with_capacity(iters);
    for k in 0..iters {
        let i = (k * 2654435761usize) % session_n;
        let sid = session_ids[i];
        let t0 = Instant::now();
        let model = require_row(
            "scenario session_update_last_seen",
            perf_session::Entity::find()
                .filter(perf_session::Column::Id.eq(sid))
                .find_one(&executor)?,
        )?;
        let mut rec = perf_session::PerfSessionRecord::from_model(&model);
        rec.set_last_seen_at(base + chrono::Duration::milliseconds(k as i64));
        let _ = perf_session::Entity::update(rec, &executor)?;
        samples.push(t0.elapsed().as_micros() as f64);
    }
    scenarios.push(stats("session_update_last_seen", &mut samples));

    let report = PerfReport {
        connections: pool_size as u32,
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

#[cfg(test)]
mod guard_tests {
    use super::{perf_reset_acknowledged_from_var, resolve_database_url_values};

    #[test]
    fn database_url_prefers_perf_over_test() {
        let r = resolve_database_url_values(
            Some("postgres://a".into()),
            Some("postgres://b".into()),
        );
        assert_eq!(r.unwrap(), "postgres://a");
    }

    #[test]
    fn database_url_falls_back_to_test_only() {
        let r = resolve_database_url_values(None, Some("postgres://b".into()));
        assert_eq!(r.unwrap(), "postgres://b");
    }

    #[test]
    fn database_url_errors_when_missing() {
        assert!(resolve_database_url_values(None, None).is_err());
    }

    #[test]
    fn database_url_skips_empty_perf() {
        let r = resolve_database_url_values(Some("  ".into()), Some("postgres://b".into()));
        assert_eq!(r.unwrap(), "postgres://b");
    }

    #[test]
    fn perf_reset_truthy_values() {
        assert!(perf_reset_acknowledged_from_var(Some("1".into())));
        assert!(perf_reset_acknowledged_from_var(Some("true".into())));
        assert!(perf_reset_acknowledged_from_var(Some("TRUE".into())));
        assert!(perf_reset_acknowledged_from_var(Some(" yes ".into())));
        assert!(perf_reset_acknowledged_from_var(Some("on".into())));
    }

    #[test]
    fn perf_reset_falsy_or_missing() {
        assert!(!perf_reset_acknowledged_from_var(None));
        assert!(!perf_reset_acknowledged_from_var(Some("".into())));
        assert!(!perf_reset_acknowledged_from_var(Some("0".into())));
        assert!(!perf_reset_acknowledged_from_var(Some("no".into())));
    }

    #[test]
    fn require_row_some_ok() {
        assert_eq!(super::require_row("ctx", Some(7_i32)).unwrap(), 7);
    }

    #[test]
    fn require_row_none_err() {
        let e = super::require_row::<i32>("warmup x", None).unwrap_err();
        assert!(
            e.to_string().contains("warmup x"),
            "message should include context: {e}"
        );
    }
}

#[cfg(test)]
mod split_tests {
    use super::{schema_sql_without_line_comments, split_postgres_statements, SCHEMA_SQL};

    #[test]
    fn splits_two_simple_statements() {
        let s = "SELECT 1; SELECT 2";
        let v = split_postgres_statements(s);
        assert_eq!(v, vec!["SELECT 1", "SELECT 2"]);
    }

    #[test]
    fn does_not_split_on_semicolon_inside_single_quoted_literal() {
        let s = "INSERT INTO t VALUES ('a;b'); DELETE FROM t";
        let v = split_postgres_statements(s);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], "INSERT INTO t VALUES ('a;b')");
        assert_eq!(v[1], "DELETE FROM t");
    }

    #[test]
    fn doubled_quote_inside_string() {
        let s = "SELECT '''x;y'''; SELECT 2";
        let v = split_postgres_statements(s);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], "SELECT '''x;y'''");
        assert_eq!(v[1], "SELECT 2");
    }

    #[test]
    fn bundled_schema_yields_expected_statement_count() {
        let script = schema_sql_without_line_comments(SCHEMA_SQL);
        let v = split_postgres_statements(&script);
        assert_eq!(
            v.len(),
            7,
            "expected DROP×3 + CREATE TABLE×3 + CREATE INDEX×1; update count if schema.sql changes"
        );
    }
}
