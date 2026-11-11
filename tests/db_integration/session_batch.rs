//! Pipelined statement batches (`StatementBatch`) and the pipelined session transaction.
//!
//! Runs as the non-superuser `rls_test_role` (created by `rls_integration`'s setup) so the RLS
//! policy applies. Covers: results and RLS filtering, isolation between consecutive users on one
//! connection, an error in the middle of a pipeline, rollback, the deferred `BEGIN`, the
//! unawaited read-only `COMMIT`, and the `LIFEGUARD_RLS_PIPELINE` off path.

use lifeguard::pool::session_batch::set_pipeline_override;
use lifeguard::{
    LifeError, LifeExecutor, LifeguardPool, PooledLifeExecutor, SessionContext, StatementBatch,
};
use sea_query::{Value, Values};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const ORG_A: &str = "650e8400-e29b-41d4-a716-446655440001";
const ORG_B: &str = "650e8400-e29b-41d4-a716-446655440002";
const ROLE_PASSWORD: &str = "rls_test_role_pw";

pub(crate) fn role_url(url: &str) -> String {
    let (scheme, rest) = url.split_once("://").expect("postgres url scheme");
    let after_user = rest.rsplit_once('@').map_or(rest, |(_, after)| after);
    format!("{scheme}://rls_test_role:{ROLE_PASSWORD}@{after_user}")
}

pub(crate) fn ctx(org: &str) -> SessionContext {
    SessionContext {
        tenant_id: "acme".to_string(),
        subject_id: uuid::Uuid::new_v4(),
        organization_id: uuid::Uuid::parse_str(org).expect("org uuid"),
        session_id: format!("batch-{}", uuid::Uuid::new_v4()),
        roles: vec!["member".to_string()],
        permissions: vec![],
        user_type: Some("member".to_string()),
        org_type: Some("tenant".to_string()),
    }
}

/// A table with an org policy, 2 rows for A and 3 for B; returns `schema.table`.
pub(crate) fn fixture() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let t = format!("batch_rls_{pid}_{n}.items");
    let ctx = crate::context::get_test_context();
    let c = may_postgres::connect(&ctx.pg_url).expect("connect");
    c.batch_execute(&format!(
        "CREATE SCHEMA IF NOT EXISTS batch_rls_{pid}_{n};
         CREATE TABLE {t} (id SERIAL PRIMARY KEY, org TEXT NOT NULL, qty INT NOT NULL CHECK (qty >= 0));
         GRANT USAGE ON SCHEMA batch_rls_{pid}_{n} TO rls_test_role;
         GRANT SELECT, INSERT, UPDATE, DELETE ON {t} TO rls_test_role;
         GRANT USAGE ON ALL SEQUENCES IN SCHEMA batch_rls_{pid}_{n} TO rls_test_role;
         ALTER TABLE {t} ENABLE ROW LEVEL SECURITY;
         CREATE POLICY org_rows ON {t}
           USING (org = NULLIF(current_setting('app.organization_id', true), ''))
           WITH CHECK (org = NULLIF(current_setting('app.organization_id', true), ''));
         INSERT INTO {t} (org, qty) VALUES ('{ORG_A}', 1), ('{ORG_A}', 2),
           ('{ORG_B}', 10), ('{ORG_B}', 20), ('{ORG_B}', 30);"
    ))
    .expect("fixture");
    t
}

fn pool(slots: usize) -> Arc<LifeguardPool> {
    let ctx = crate::context::get_test_context();
    Arc::new(LifeguardPool::new(&role_url(&ctx.pg_url), slots, Vec::new(), 0).expect("pool"))
}

fn no_params() -> Values {
    Values(Vec::new())
}

fn sum_count(pool: &LifeguardPool, t: &str, org: &str) -> (i64, i64) {
    let mut b = StatementBatch::new();
    let n = b
        .query_opt(&format!("SELECT count(*)::int8 FROM {t}"), &no_params())
        .expect("q");
    let s = b
        .query_opt(
            &format!("SELECT COALESCE(sum(qty), 0)::int8 FROM {t}"),
            &no_params(),
        )
        .expect("q");
    let mut out = pool.with_session_batch(&ctx(org), b).expect("batch");
    let n: i64 = out.take(n).expect("row").get(0);
    let s: i64 = out.take(s).expect("row").get(0);
    (n, s)
}

/// Superuser view of the table (bypasses RLS).
pub(crate) fn total_rows(t: &str) -> i64 {
    let ctx = crate::context::get_test_context();
    let c = may_postgres::connect(&ctx.pg_url).expect("connect");
    c.query_one(format!("SELECT count(*)::int8 FROM {t}").as_str(), &[])
        .expect("count")
        .get(0)
}

/// The organization GUC as seen outside any unit (must be empty: nothing session-scoped).
fn org_outside_any_unit(exec: &PooledLifeExecutor) -> String {
    exec.query_one_values(
        "SELECT COALESCE(current_setting('app.organization_id', true), '')",
        &no_params(),
    )
    .expect("guc")
    .get(0)
}

#[test]
fn batch_returns_every_result_under_the_callers_policy() {
    let t = fixture();
    let p = pool(2);
    assert_eq!(sum_count(&p, &t, ORG_A), (2, 3));
    assert_eq!(sum_count(&p, &t, ORG_B), (3, 60));

    // Mixed kinds: a write, a read of it, a multi-row read, in one unit.
    let mut b = StatementBatch::new();
    let w = b
        .execute(
            &format!("INSERT INTO {t} (org, qty) VALUES ($1, 5)"),
            &Values(vec![Value::from(ORG_A)]),
        )
        .expect("q");
    let n = b
        .query_opt(&format!("SELECT count(*)::int8 FROM {t}"), &no_params())
        .expect("q");
    let all = b
        .query_all(&format!("SELECT qty FROM {t} ORDER BY qty"), &no_params())
        .expect("q");
    let mut out = p.with_session_batch(&ctx(ORG_A), b).expect("batch");
    assert_eq!(out.take(w), 1);
    let n: i64 = out.take(n).expect("row").get(0);
    assert_eq!(n, 3, "the read sees the earlier write of the same unit");
    let qty: Vec<i32> = out.take(all).iter().map(|r| r.get(0)).collect();
    assert_eq!(qty, vec![1, 2, 5]);
}

#[test]
fn consecutive_users_on_one_connection_never_see_each_others_context() {
    let t = fixture();
    let p = pool(1); // every unit on the same connection
    let plain = PooledLifeExecutor::new(Arc::clone(&p));
    for i in 0..40 {
        let (org, want) = if i % 2 == 0 { (ORG_A, 2) } else { (ORG_B, 3) };
        let mut b = StatementBatch::new();
        let g = b
            .query_opt(
                "SELECT current_setting('app.organization_id', true)",
                &no_params(),
            )
            .expect("q");
        let n = b
            .query_opt(&format!("SELECT count(*)::int8 FROM {t}"), &no_params())
            .expect("q");
        let mut out = p.with_session_batch(&ctx(org), b).expect("batch");
        let g: String = out.take(g).expect("row").get(0);
        let n: i64 = out.take(n).expect("row").get(0);
        assert_eq!(g, org, "iteration {i}");
        assert_eq!(n, want, "iteration {i}");
        // Between users, outside any unit: no context at all.
        assert_eq!(
            org_outside_any_unit(&plain),
            "",
            "iteration {i}: context leaked"
        );
    }
}

#[test]
fn an_error_mid_pipeline_is_reported_and_nothing_commits() {
    let t = fixture();
    let p = pool(1);
    let before = total_rows(&t);
    let mut b = StatementBatch::new();
    let _ = b
        .execute(
            &format!("INSERT INTO {t} (org, qty) VALUES ($1, 7)"),
            &Values(vec![Value::from(ORG_A)]),
        )
        .expect("q");
    let _ = b.query_opt("SELECT 1 / 0", &no_params()).expect("q");
    let _ = b
        .execute(
            &format!("INSERT INTO {t} (org, qty) VALUES ($1, 8)"),
            &Values(vec![Value::from(ORG_A)]),
        )
        .expect("q");
    let err = match p.with_session_batch(&ctx(ORG_A), b) {
        Ok(_) => panic!("division by zero must fail the batch"),
        Err(e) => e,
    };
    match &err {
        LifeError::PostgresError(pe) => assert_eq!(
            pe.code().map(|c| c.code().to_string()).as_deref(),
            Some("22012"),
            "the first real error, not the 25P02 that follows it: {err}"
        ),
        other => panic!("unexpected error {other}"),
    }
    assert_eq!(total_rows(&t), before, "neither insert committed");
    // The connection is reusable and its context is clean.
    assert_eq!(sum_count(&p, &t, ORG_B), (3, 60));
    let plain = PooledLifeExecutor::new(Arc::clone(&p));
    assert_eq!(org_outside_any_unit(&plain), "");
}

#[test]
fn a_policy_violation_rolls_the_whole_unit_back() {
    let t = fixture();
    let p = pool(1);
    let before = total_rows(&t);
    let mut b = StatementBatch::new();
    let _ = b
        .execute(
            &format!("INSERT INTO {t} (org, qty) VALUES ($1, 1)"),
            &Values(vec![Value::from(ORG_A)]),
        )
        .expect("q");
    // WITH CHECK rejects a row for another org (42501).
    let _ = b
        .execute(
            &format!("INSERT INTO {t} (org, qty) VALUES ($1, 1)"),
            &Values(vec![Value::from(ORG_B)]),
        )
        .expect("q");
    assert!(p.with_session_batch(&ctx(ORG_A), b).is_err());
    assert_eq!(total_rows(&t), before);
}

#[test]
fn session_transaction_batches_writes_and_rolls_back_on_error() {
    let t = fixture();
    let p = pool(1);
    let before = total_rows(&t);
    // A unit of work: N writes in one round trip inside the transaction, then the closure fails.
    let r: Result<(), LifeError> = p.with_session_transaction(&ctx(ORG_A), |tx| {
        let mut b = StatementBatch::new();
        for q in 0..5 {
            let _ = b.execute(
                &format!("INSERT INTO {t} (org, qty) VALUES ($1, $2)"),
                &Values(vec![Value::from(ORG_A), Value::from(q)]),
            )?;
        }
        let out = tx.run_batch(b)?;
        assert_eq!(out.len(), 5);
        Err(LifeError::Other("application says no".into()))
    });
    assert!(r.is_err());
    assert_eq!(total_rows(&t), before, "rolled back");

    // Same unit, committed.
    p.with_session_transaction(&ctx(ORG_A), |tx| {
        let mut b = StatementBatch::new();
        for q in 0..5 {
            let _ = b.execute(
                &format!("INSERT INTO {t} (org, qty) VALUES ($1, $2)"),
                &Values(vec![Value::from(ORG_A), Value::from(q)]),
            )?;
        }
        tx.run_batch(b).map(|_| ())
    })
    .expect("commit");
    assert_eq!(total_rows(&t), before + 5);
    assert_eq!(sum_count(&p, &t, ORG_A).0, 7);
}

#[test]
fn deferred_begin_carries_the_context_and_an_empty_transaction_costs_nothing() {
    let t = fixture();
    let p = pool(1);
    // No statement at all: commit is a no-op, not an error.
    p.with_session_transaction(&ctx(ORG_A), |_tx| Ok(()))
        .expect("empty transaction");
    // The first statement carries BEGIN and the context.
    let n: i64 = p
        .with_session_transaction(&ctx(ORG_B), |tx| {
            tx.query_one_values(&format!("SELECT count(*)::int8 FROM {t}"), &no_params())
                .map(|r| r.get(0))
        })
        .expect("tx");
    assert_eq!(n, 3);
    // A failing first statement: the error is the statement's, and the slot is clean after.
    let r: Result<i64, LifeError> = p.with_session_transaction(&ctx(ORG_A), |tx| {
        tx.query_one_values("SELECT (1 / 0)::int8", &no_params())
            .map(|r| r.get(0))
    });
    assert!(r.is_err());
    assert_eq!(sum_count(&p, &t, ORG_A), (2, 3));
}

#[test]
fn read_only_transaction_commit_is_not_awaited_and_leaves_the_slot_clean() {
    let t = fixture();
    let p = pool(1);
    let plain = PooledLifeExecutor::new(Arc::clone(&p));
    for i in 0..20 {
        let org = if i % 2 == 0 { ORG_A } else { ORG_B };
        let (g, n): (String, i64) = p
            .with_session_read_transaction(&ctx(org), |tx| {
                let g: String = tx
                    .query_one_values(
                        "SELECT current_setting('app.organization_id', true)",
                        &no_params(),
                    )?
                    .get(0);
                let n: i64 = tx
                    .query_one_values(&format!("SELECT count(*)::int8 FROM {t}"), &no_params())?
                    .get(0);
                Ok((g, n))
            })
            .expect("read tx");
        assert_eq!(g, org);
        assert_eq!(n, if org == ORG_A { 2 } else { 3 });
        // Straight after the unawaited COMMIT, on the same connection: clean, and committed.
        assert_eq!(org_outside_any_unit(&plain), "", "iteration {i}");
    }
    // A write inside a read-only transaction is refused (25006).
    let r: Result<u64, LifeError> = p.with_session_read_transaction(&ctx(ORG_A), |tx| {
        tx.execute_values(
            &format!("INSERT INTO {t} (org, qty) VALUES ($1, 1)"),
            &Values(vec![Value::from(ORG_A)]),
        )
    });
    assert!(r.is_err());
    assert_eq!(sum_count(&p, &t, ORG_A), (2, 3));
}

#[test]
fn pipeline_off_gives_the_same_results() {
    let t = fixture();
    let p = pool(1);
    set_pipeline_override(Some(false));
    let off = (sum_count(&p, &t, ORG_A), sum_count(&p, &t, ORG_B));
    let mut b = StatementBatch::new();
    let _ = b.query_opt("SELECT 1 / 0", &no_params()).expect("q");
    let off_err = p.with_session_batch(&ctx(ORG_A), b).is_err();
    let tx_off: i64 = p
        .with_session_read_transaction(&ctx(ORG_B), |tx| {
            tx.query_one_values(&format!("SELECT count(*)::int8 FROM {t}"), &no_params())
                .map(|r| r.get(0))
        })
        .expect("tx");
    set_pipeline_override(None);
    let on = (sum_count(&p, &t, ORG_A), sum_count(&p, &t, ORG_B));
    assert_eq!(off, on);
    assert_eq!(on, ((2, 3), (3, 60)));
    assert!(off_err);
    assert_eq!(tx_off, 3);
}

#[test]
fn batch_carries_an_isolation_level_on_its_begin() {
    let t = fixture();
    let p = pool(1);
    let mut b = StatementBatch::new();
    let lvl = b
        .query_opt(
            "SELECT current_setting('transaction_isolation')",
            &no_params(),
        )
        .expect("q");
    let g = b
        .query_opt(
            "SELECT current_setting('app.organization_id', true)",
            &no_params(),
        )
        .expect("q");
    let n = b
        .query_opt(&format!("SELECT count(*)::int8 FROM {t}"), &no_params())
        .expect("q");
    let mut out = p
        .with_session_batch_isolated(&ctx(ORG_B), lifeguard::IsolationLevel::RepeatableRead, b)
        .expect("batch");
    let lvl: String = out.take(lvl).expect("row").get(0);
    let g: String = out.take(g).expect("row").get(0);
    let n: i64 = out.take(n).expect("row").get(0);
    assert_eq!(lvl, "repeatable read");
    assert_eq!(g, ORG_B);
    assert_eq!(n, 3);
    // Plain batch: the server default.
    let mut b = StatementBatch::new();
    let lvl = b
        .query_opt(
            "SELECT current_setting('transaction_isolation')",
            &no_params(),
        )
        .expect("q");
    let mut out = p.with_session_batch(&ctx(ORG_B), b).expect("batch");
    let lvl: String = out.take(lvl).expect("row").get(0);
    assert_eq!(lvl, "read committed");
}

fn wedged_batch() -> StatementBatch {
    let mut b = StatementBatch::new();
    let _ = b.query_opt("SELECT 1", &no_params()).expect("q");
    let _ = b
        .query_opt("SELECT pg_sleep(4)::text", &no_params())
        .expect("q");
    let _ = b.query_opt("SELECT 2", &no_params()).expect("q");
    b
}

fn deadline_pool() -> Arc<LifeguardPool> {
    let ctx = crate::context::get_test_context();
    let settings = lifeguard::LifeguardPoolSettings {
        reply_timeout: Some(std::time::Duration::from_secs(1)),
        ..lifeguard::LifeguardPoolSettings::default()
    };
    Arc::new(
        LifeguardPool::new_with_settings(&role_url(&ctx.pg_url), 1, vec![], 0, &settings)
            .expect("pool"),
    )
}

/// A pipelined batch is one dispatch: it must finish within the reply deadline as a whole. A
/// statement that wedges partway through yields a bounded `PoolReplyTimeout`, nothing commits,
/// and the slot serves again (with a clean context) once the server is done.
#[test]
fn reply_timeout_bounds_a_batch_wedged_partway() {
    let t = fixture();
    let p = deadline_pool();
    let started = std::time::Instant::now();
    let err = match p.with_session_batch(&ctx(ORG_A), wedged_batch()) {
        Ok(_) => panic!("pg_sleep(4) must overrun the 1s reply budget"),
        Err(e) => e,
    };
    let waited = started.elapsed();
    assert!(
        matches!(err, LifeError::PoolReplyTimeout { .. }),
        "expected PoolReplyTimeout, got {err:?}"
    );
    assert!(
        waited < std::time::Duration::from_secs(3),
        "reply timeout must be bounded, waited {waited:?}"
    );
    std::thread::sleep(std::time::Duration::from_secs(5));
    assert_eq!(
        sum_count(&p, &t, ORG_B),
        (3, 60),
        "slot usable after the episode"
    );
    let plain = PooledLifeExecutor::new(Arc::clone(&p));
    assert_eq!(org_outside_any_unit(&plain), "");
}

/// The same bound for a coroutine caller (the batch runs inline; the watchdog shuts the wedged
/// connection down at the deadline).
#[test]
fn reply_timeout_bounds_a_wedged_batch_for_coroutine_callers() {
    let t = fixture();
    let p = deadline_pool();
    let p2 = Arc::clone(&p);
    let (err, waited) = may::go!(
        may::coroutine::Builder::new().stack_size(0x8000),
        move || {
            let started = std::time::Instant::now();
            let r = p2.with_session_batch(&ctx(ORG_A), wedged_batch());
            (r.err(), started.elapsed())
        }
    )
    .expect("spawn")
    .join()
    .expect("join");
    assert!(
        matches!(err, Some(LifeError::PoolReplyTimeout { .. })),
        "expected PoolReplyTimeout, got {err:?}"
    );
    assert!(
        waited < std::time::Duration::from_secs(3),
        "reply timeout must be bounded, waited {waited:?}"
    );
    std::thread::sleep(std::time::Duration::from_secs(5));
    assert_eq!(
        sum_count(&p, &t, ORG_A),
        (2, 3),
        "slot usable after the episode"
    );
}

/// A unit of work is atomic whatever batch policy exists elsewhere: one bad row fails it all.
#[test]
fn with_session_batch_never_bisects() {
    let t = fixture();
    let p = pool(1);
    let before = total_rows(&t);
    let mut b = StatementBatch::new();
    for q in [1, 2, -1, 4] {
        // qty >= 0 CHECK: the third insert is a class-23 row error.
        let _ = b
            .execute(
                &format!("INSERT INTO {t} (org, qty) VALUES ($1, $2)"),
                &Values(vec![Value::from(ORG_A), Value::from(q)]),
            )
            .expect("q");
    }
    let err = match p.with_session_batch(&ctx(ORG_A), b) {
        Ok(_) => panic!("the CHECK violation must fail the unit"),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains("check") || err.to_string().contains("23514"),
        "{err}"
    );
    assert_eq!(total_rows(&t), before, "nothing of the unit was written");
}

// ---------------------------------------------------------------------------------------------
// Microbench (ignored by default):
//   LG_BENCH_RTT_US=1450 cargo test --test db_integration_suite -- --ignored --nocapture \
//       --test-threads=1 session_batch::bench_
// A local TCP proxy delays every forwarded chunk by RTT/2 in each direction, modelling the
// network between a service and its database; 0 runs direct.

struct DelayProxy {
    addr: std::net::SocketAddr,
}

fn delay_proxy(backend: std::net::SocketAddr, one_way: std::time::Duration) -> DelayProxy {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        for client in listener.incoming().flatten() {
            let Ok(server) = std::net::TcpStream::connect(backend) else {
                continue;
            };
            let _ = client.set_nodelay(true);
            let _ = server.set_nodelay(true);
            for (mut from, mut to) in [
                (
                    client.try_clone().expect("clone"),
                    server.try_clone().expect("clone"),
                ),
                (server, client),
            ] {
                // Each chunk is due `one_way` after it was read, independently of the chunks
                // before it: a link with latency, not a queue (pipelined writes stay pipelined).
                let (tx, rx) = std::sync::mpsc::channel::<(std::time::Instant, Vec<u8>)>();
                std::thread::spawn(move || {
                    let mut buf = vec![0u8; 64 * 1024];
                    loop {
                        let n = match from.read(&mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => n,
                        };
                        if tx
                            .send((std::time::Instant::now() + one_way, buf[..n].to_vec()))
                            .is_err()
                        {
                            break;
                        }
                    }
                });
                std::thread::spawn(move || {
                    for (due, bytes) in rx {
                        let now = std::time::Instant::now();
                        if due > now {
                            std::thread::sleep(due - now);
                        }
                        if to.write_all(&bytes).is_err() {
                            break;
                        }
                    }
                    let _ = to.shutdown(std::net::Shutdown::Both);
                });
            }
        }
    });
    DelayProxy { addr }
}

fn bench_url(rtt_us: u64) -> String {
    let ctx = crate::context::get_test_context();
    proxied(&role_url(&ctx.pg_url), rtt_us)
}

/// `url` routed through a [`delay_proxy`] adding `rtt_us` of round trip (`0`: `url` itself).
pub(crate) fn proxied(url: &str, rtt_us: u64) -> String {
    let url = url.to_string();
    if rtt_us == 0 {
        return url;
    }
    let (scheme, rest) = url.split_once("://").expect("scheme");
    let (user, hostpath) = rest.rsplit_once('@').expect("userinfo");
    let (hostport, path) = hostpath.split_once('/').unwrap_or((hostpath, "postgres"));
    let backend: std::net::SocketAddr = std::net::ToSocketAddrs::to_socket_addrs(hostport)
        .expect("resolve")
        .next()
        .expect("addr");
    let p = delay_proxy(backend, std::time::Duration::from_micros(rtt_us / 2));
    format!("{scheme}://{user}@{}/{path}", p.addr)
}

pub(crate) fn rtt_us() -> u64 {
    std::env::var("LG_BENCH_RTT_US")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1450)
}

pub(crate) fn stats(mut ms: Vec<f64>) -> (f64, f64, f64) {
    ms.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let q = |p: f64| ms[((ms.len() as f64 - 1.0) * p).round() as usize];
    let mean = ms.iter().sum::<f64>() / ms.len() as f64;
    (q(0.5), q(0.95), mean)
}

pub(crate) fn report(name: &str, rtt: u64, ms: Vec<f64>, round_trips: &str) {
    let (p50, p95, mean) = stats(ms);
    println!(
        "BENCH | {name} | rtt_us={rtt} | p50_ms={p50:.2} | p95_ms={p95:.2} | mean_ms={mean:.2} | round_trips={round_trips}"
    );
}

pub(crate) fn time_ms(f: impl FnOnce()) -> f64 {
    let t = std::time::Instant::now();
    f();
    t.elapsed().as_secs_f64() * 1000.0
}

fn eight_reads(t: &str) -> Vec<String> {
    vec![
        format!("SELECT count(*)::int8 FROM {t}"),
        format!("SELECT COALESCE(sum(qty), 0)::int8 FROM {t}"),
        format!("SELECT COALESCE(max(qty), 0)::int8 FROM {t}"),
        format!("SELECT COALESCE(min(qty), 0)::int8 FROM {t}"),
        format!("SELECT count(*)::int8 FROM {t} WHERE qty > 1"),
        format!("SELECT count(*)::int8 FROM {t} WHERE qty > 10"),
        "SELECT current_setting('app.organization_id', true)::text".to_string(),
        "SELECT now()::text".to_string(),
    ]
}

#[test]
#[ignore = "microbench: run explicitly"]
fn bench_rls_reads_sequential_vs_batched() {
    let t = fixture();
    for rtt in [0, rtt_us()] {
        let p = Arc::new(LifeguardPool::new(&bench_url(rtt), 2, Vec::new(), 0).expect("pool"));
        let exec = PooledLifeExecutor::new(Arc::clone(&p)).with_session_context(ctx(ORG_B));
        let reads = eight_reads(&t);
        // Warm the statement caches.
        for _ in 0..3 {
            for q in &reads {
                let _ = exec.query_one_values(q, &no_params()).expect("warm");
            }
            let mut b = StatementBatch::new();
            for q in &reads {
                let _ = b.query_opt(q, &no_params()).expect("q");
            }
            p.with_session_batch(&ctx(ORG_B), b).expect("warm batch");
        }
        let n = 200;
        let seq: Vec<f64> = (0..n)
            .map(|_| {
                time_ms(|| {
                    for q in &reads {
                        let _ = exec.query_one_values(q, &no_params()).expect("read");
                    }
                })
            })
            .collect();
        report("8 RLS reads, one unit each (today)", rtt, seq, "8");
        let bat: Vec<f64> = (0..n)
            .map(|_| {
                time_ms(|| {
                    let mut b = StatementBatch::new();
                    for q in &reads {
                        let _ = b.query_opt(q, &no_params()).expect("q");
                    }
                    p.with_session_batch(&ctx(ORG_B), b).expect("batch");
                })
            })
            .collect();
        report("8 RLS reads, with_session_batch", rtt, bat, "1");
        let one: Vec<f64> = (0..n)
            .map(|_| {
                time_ms(|| {
                    let _ = exec
                        .query_one_values(&reads[0], &no_params())
                        .expect("read");
                })
            })
            .collect();
        report("1 RLS read, single unit (floor)", rtt, one, "1");
    }
}

#[test]
#[ignore = "microbench: run explicitly"]
fn bench_session_transaction_pipelined_vs_not() {
    let t = fixture();
    for rtt in [0, rtt_us()] {
        let p = Arc::new(LifeguardPool::new(&bench_url(rtt), 2, Vec::new(), 0).expect("pool"));
        let reads = eight_reads(&t);
        let three = |tx: &lifeguard::ExclusivePrimaryLifeExecutor<'_>| -> Result<(), LifeError> {
            for q in reads.iter().take(3) {
                let _ = tx.query_one_values(q, &no_params())?;
            }
            Ok(())
        };
        for (label, on) in [("pipeline off", false), ("pipeline on", true)] {
            set_pipeline_override(Some(on));
            for _ in 0..3 {
                p.with_session_read_transaction(&ctx(ORG_A), three)
                    .expect("warm");
                p.with_session_transaction(&ctx(ORG_A), three)
                    .expect("warm");
            }
            let ro: Vec<f64> = (0..200)
                .map(|_| {
                    time_ms(|| {
                        p.with_session_read_transaction(&ctx(ORG_A), three)
                            .expect("tx")
                    })
                })
                .collect();
            report(
                &format!("read-only session tx, 3 reads, {label}"),
                rtt,
                ro,
                if on {
                    "3 (BEGIN+set ride on read 1; COMMIT unawaited)"
                } else {
                    "5"
                },
            );
            let rw: Vec<f64> = (0..200)
                .map(|_| time_ms(|| p.with_session_transaction(&ctx(ORG_A), three).expect("tx")))
                .collect();
            report(
                &format!("read-write session tx, 3 reads, {label}"),
                rtt,
                rw,
                if on {
                    "4 (BEGIN+set ride on read 1)"
                } else {
                    "5"
                },
            );
        }
        set_pipeline_override(None);
    }
}
