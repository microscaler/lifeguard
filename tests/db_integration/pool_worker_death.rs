//! Pool worker death and recovery (2026-08-29 starvation incident).
//!
//! Three guarantees, layered:
//! 1. A worker whose `may_postgres` connection dies mid-flight surfaces a
//!    bounded error (or transparently heals) — never an infinite hang.
//! 2. The pool replaces the dead connection: the next request on that slot
//!    succeeds (self-heal via `exec_with_optional_heal`, now reachable because
//!    may_postgres fails pending responses on I/O-loop exit).
//! 3. Under concurrent multi-call load across every slot the pool does not
//!    permanently lose capacity.
//!
//! Plus the dispatch-side reply deadline: a statement that overruns
//! `reply_timeout` yields [`LifeError::PoolReplyTimeout`] within a bound.
//!
//! The tests run Postgres traffic through an in-test TCP proxy so the
//! transport can be severed abruptly (no clean server ErrorResponse — the
//! abrupt cut is the shape that wedged production).

use crate::context::get_test_context;
use lifeguard::{LifeError, LifeExecutor, LifeguardPool, LifeguardPoolSettings, PooledLifeExecutor};
use sea_query::Values;
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Multi-connection TCP proxy with abrupt kill: every proxied connection can
/// be severed at once; new connections keep working (so slot heal succeeds).
struct KillSwitchProxy {
    addr: SocketAddr,
    live: Arc<Mutex<Vec<(TcpStream, TcpStream)>>>,
}

impl KillSwitchProxy {
    fn start(backend: SocketAddr) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("proxy bind");
        let addr = listener.local_addr().expect("proxy addr");
        let live: Arc<Mutex<Vec<(TcpStream, TcpStream)>>> = Arc::new(Mutex::new(Vec::new()));
        let live2 = live.clone();
        thread::spawn(move || {
            for client in listener.incoming() {
                let Ok(client) = client else { return };
                let Ok(server) = TcpStream::connect(backend) else {
                    client.shutdown(Shutdown::Both).ok();
                    continue;
                };
                client.set_nodelay(true).ok();
                server.set_nodelay(true).ok();
                if let (Ok(ck), Ok(sk)) = (client.try_clone(), server.try_clone()) {
                    live2.lock().expect("proxy lock").push((ck, sk));
                }
                let (c2, s2) = (
                    client.try_clone().expect("clone"),
                    server.try_clone().expect("clone"),
                );
                thread::spawn(move || pump(client, server));
                thread::spawn(move || pump(s2, c2));
            }
        });
        KillSwitchProxy { addr, live }
    }

    /// Abruptly severs every currently proxied connection.
    fn sever_all(&self) {
        let mut conns = self.live.lock().expect("proxy lock");
        for (c, s) in conns.drain(..) {
            c.shutdown(Shutdown::Both).ok();
            s.shutdown(Shutdown::Both).ok();
        }
    }
}

fn pump(mut from: TcpStream, mut to: TcpStream) {
    let mut buf = [0u8; 8192];
    loop {
        match from.read(&mut buf) {
            Ok(0) | Err(_) => {
                to.shutdown(Shutdown::Both).ok();
                return;
            }
            Ok(n) => {
                if to.write_all(&buf[..n]).is_err() {
                    from.shutdown(Shutdown::Both).ok();
                    return;
                }
            }
        }
    }
}

/// Rewrites the test context's PG URL to go through the proxy.
fn proxied_url(pg_url: &str, proxy: SocketAddr) -> String {
    // Both `host=… port=…` and URL forms appear in test envs; normalize the
    // authority to the proxy while keeping credentials/dbname.
    if pg_url.contains("://") {
        // postgres://user:pass@host:port/db
        let (scheme, rest) = pg_url.split_once("://").expect("scheme");
        let (auth, tail) = match rest.split_once('@') {
            Some((creds, tail)) => (Some(creds), tail),
            None => (None, rest),
        };
        let path = tail.split_once('/').map(|(_, p)| p).unwrap_or("");
        match auth {
            Some(creds) => format!(
                "{scheme}://{creds}@{}:{}/{path}",
                proxy.ip(),
                proxy.port()
            ),
            None => format!("{scheme}://{}:{}/{path}", proxy.ip(), proxy.port()),
        }
    } else {
        let kept: Vec<&str> = pg_url
            .split_whitespace()
            .filter(|kv| !kv.starts_with("host=") && !kv.starts_with("port="))
            .collect();
        format!(
            "host={} port={} {}",
            proxy.ip(),
            proxy.port(),
            kept.join(" ")
        )
    }
}

fn backend_addr(pg_url: &str) -> SocketAddr {
    let (mut host, mut port) = ("127.0.0.1".to_string(), 5432u16);
    if pg_url.contains("://") {
        let rest = pg_url.split_once("://").expect("scheme").1;
        let tail = rest.split_once('@').map_or(rest, |(_, t)| t);
        let authority = tail.split('/').next().unwrap_or(tail);
        if let Some((h, p)) = authority.rsplit_once(':') {
            host = h.to_string();
            port = p.parse().unwrap_or(5432);
        } else if !authority.is_empty() {
            host = authority.to_string();
        }
    } else {
        for kv in pg_url.split_whitespace() {
            if let Some(v) = kv.strip_prefix("host=") {
                host = v.to_string();
            } else if let Some(v) = kv.strip_prefix("port=") {
                port = v.parse().unwrap_or(5432);
            }
        }
    }
    format!("{host}:{port}")
        .parse()
        .or_else(|_| {
            use std::net::ToSocketAddrs;
            (host.as_str(), port)
                .to_socket_addrs()
                .map(|mut it| it.next().expect("resolved"))
        })
        .expect("backend addr")
}

fn pool_through_proxy(
    slots: usize,
    settings: &LifeguardPoolSettings,
) -> (PooledLifeExecutor, KillSwitchProxy) {
    let ctx = get_test_context();
    let backend = backend_addr(&ctx.pg_url);
    let proxy = KillSwitchProxy::start(backend);
    let url = proxied_url(&ctx.pg_url, proxy.addr);
    let pool = LifeguardPool::new_with_settings(&url, slots, vec![], 0, settings)
        .expect("LifeguardPool through proxy");
    (PooledLifeExecutor::new(Arc::new(pool)), proxy)
}

/// (1) + (2): sever every worker connection, then require that queries return
/// bounded results and the pool self-heals so follow-up queries succeed.
#[test]
fn severed_workers_recover_and_dispatch_stays_bounded() {
    let settings = LifeguardPoolSettings {
        reply_timeout: Some(Duration::from_secs(10)),
        ..LifeguardPoolSettings::default()
    };
    let (ex, proxy) = pool_through_proxy(2, &settings);

    ex.query_one_values("SELECT 1", &Values(Vec::new()))
        .expect("warmup query");

    proxy.sever_all();

    // Every slot's connection is now dead. Each dispatch must come back within
    // a bound — healed (Ok) or a classified error — and the pool must recover.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut recovered = 0usize;
    for _ in 0..8 {
        assert!(Instant::now() < deadline, "pool did not recover in time");
        let started = Instant::now();
        let r = ex.query_one_values("SELECT 1", &Values(Vec::new()));
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "dispatch exceeded bound after severance: {:?}",
            started.elapsed()
        );
        if r.is_ok() {
            recovered += 1;
        }
    }
    assert!(
        recovered >= 4,
        "pool must self-heal severed workers (got {recovered}/8 successes)"
    );

    // Steady state after heal: everything succeeds.
    for _ in 0..4 {
        ex.query_one_values("SELECT 1", &Values(Vec::new()))
            .expect("post-heal query");
    }
}

/// (3): concurrent multi-call load across severed slots must not starve the
/// pool — all callers finish, and capacity is fully restored afterwards.
#[test]
fn concurrent_load_after_severance_does_not_starve() {
    let settings = LifeguardPoolSettings {
        reply_timeout: Some(Duration::from_secs(10)),
        ..LifeguardPoolSettings::default()
    };
    let (ex, proxy) = pool_through_proxy(2, &settings);
    let ex = Arc::new(ex);

    ex.query_one_values("SELECT 1", &Values(Vec::new()))
        .expect("warmup");
    proxy.sever_all();

    let mut handles = Vec::new();
    for _ in 0..6 {
        let ex = ex.clone();
        handles.push(thread::spawn(move || {
            // Multi-call handler shape (the /tickers pattern): several
            // sequential pooled calls per logical request.
            let mut oks = 0usize;
            for _ in 0..3 {
                if ex.query_one_values("SELECT 1", &Values(Vec::new())).is_ok() {
                    oks += 1;
                }
            }
            oks
        }));
    }

    let started = Instant::now();
    let mut total_ok = 0usize;
    for h in handles {
        total_ok += h.join().expect("worker thread completed (no hang)");
    }
    assert!(
        started.elapsed() < Duration::from_secs(60),
        "concurrent load must complete within a bound, took {:?}",
        started.elapsed()
    );

    // Capacity restored: a full sweep of follow-up calls succeeds.
    for _ in 0..6 {
        ex.query_one_values("SELECT 1", &Values(Vec::new()))
            .expect("post-load query — pool starved?");
    }
    assert!(total_ok > 0, "at least some healed calls should succeed");
}

/// Reply deadline: a statement overrunning `reply_timeout` yields a bounded
/// `PoolReplyTimeout`, and the worker slot remains usable afterwards.
#[test]
fn reply_timeout_bounds_wedged_dispatch() {
    let ctx = get_test_context();
    let settings = LifeguardPoolSettings {
        reply_timeout: Some(Duration::from_secs(1)),
        ..LifeguardPoolSettings::default()
    };
    let pool = LifeguardPool::new_with_settings(&ctx.pg_url, 1, vec![], 0, &settings)
        .expect("pool");
    let ex = PooledLifeExecutor::new(Arc::new(pool));

    let started = Instant::now();
    let err = match ex.query_one_values("SELECT pg_sleep(4)", &Values(Vec::new())) {
        Ok(_) => panic!("pg_sleep(4) must overrun the 1s reply budget"),
        Err(e) => e,
    };
    let waited = started.elapsed();
    assert!(
        matches!(err, LifeError::PoolReplyTimeout { .. }),
        "expected PoolReplyTimeout, got {err:?}"
    );
    assert!(
        waited < Duration::from_secs(3),
        "reply timeout must be bounded, waited {waited:?}"
    );

    // The worker finishes the sleep on its own; the slot must then serve again.
    thread::sleep(Duration::from_secs(4));
    ex.query_one_values("SELECT 1", &Values(Vec::new()))
        .expect("slot usable after reply-timeout episode");
}
