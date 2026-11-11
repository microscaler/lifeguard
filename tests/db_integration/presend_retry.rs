//! Pre-send retry: a unit whose connection is dead before anything reached the server moves to
//! a fresh connection once; a failure after the unit's first write is returned, never retried.
//!
//! A proxy between the pool and PostgreSQL cuts connections on demand: every open one at once
//! (connections that died while idle), or the one whose client bytes carry a marker. A marker in
//! the SQL text is seen in the `Parse` sent while the unit prepares, before it writes anything; a
//! marker in a parameter value is seen in the `Bind` of the unit's write.

use crate::session_batch::{ctx, fixture, role_url, total_rows};
use lifeguard::{LifeExecutor, LifeguardPool, PooledLifeExecutor, StatementBatch};
use sea_query::{Value, Values};
use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const ORG_A: &str = "650e8400-e29b-41d4-a716-446655440001";
const ORG_GUC: &str = "SELECT COALESCE(current_setting('app.organization_id', true), '')";

/// The marker that cuts a connection, and how many more cuts to make.
type Marker = Arc<Mutex<(Option<Vec<u8>>, usize)>>;

struct KillProxy {
    addr: SocketAddr,
    open: Arc<Mutex<Vec<(TcpStream, TcpStream)>>>,
    marker: Marker,
}

fn clone(s: &TcpStream) -> TcpStream {
    s.try_clone().expect("clone stream")
}

impl KillProxy {
    fn start(backend: SocketAddr) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let open = Arc::new(Mutex::new(Vec::new()));
        let marker: Marker = Arc::new(Mutex::new((None, 0)));
        let (o, m) = (Arc::clone(&open), Arc::clone(&marker));
        std::thread::spawn(move || {
            for client in listener.incoming().flatten() {
                let Ok(server) = TcpStream::connect(backend) else {
                    continue;
                };
                let _ = client.set_nodelay(true);
                let _ = server.set_nodelay(true);
                o.lock()
                    .expect("lock")
                    .push((clone(&client), clone(&server)));
                // Client to server, cutting both ends instead of forwarding a marked chunk.
                let (mut from, mut to) = (clone(&client), clone(&server));
                let (kc, ks) = (clone(&client), clone(&server));
                let m = Arc::clone(&m);
                std::thread::spawn(move || {
                    let mut buf = vec![0u8; 64 * 1024];
                    loop {
                        let n = match from.read(&mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => n,
                        };
                        let cut = {
                            let mut g = m.lock().expect("lock");
                            let (marker, left) = &mut *g;
                            let hit = *left > 0
                                && marker.as_ref().is_some_and(|k| {
                                    buf[..n].windows(k.len()).any(|w| w == k.as_slice())
                                });
                            if hit {
                                *left -= 1;
                            }
                            hit
                        };
                        if cut {
                            let _ = kc.shutdown(Shutdown::Both);
                            let _ = ks.shutdown(Shutdown::Both);
                            break;
                        }
                        if to.write_all(&buf[..n]).is_err() {
                            break;
                        }
                    }
                    let _ = to.shutdown(Shutdown::Both);
                });
                let (mut from, mut to) = (server, client);
                std::thread::spawn(move || {
                    let _ = std::io::copy(&mut from, &mut to);
                    let _ = to.shutdown(Shutdown::Both);
                });
            }
        });
        Self { addr, open, marker }
    }

    /// Cut every open connection, as if each had died while idle.
    fn cut_all(&self) {
        for (c, s) in self.open.lock().expect("lock").drain(..) {
            let _ = c.shutdown(Shutdown::Both);
            let _ = s.shutdown(Shutdown::Both);
        }
    }

    /// Cut the next `n` connections whose client bytes contain `marker`.
    fn cut_on(&self, marker: &str, n: usize) {
        *self.marker.lock().expect("lock") = (Some(marker.as_bytes().to_vec()), n);
    }
}

/// A one-slot pool (every unit on the same slot) reaching PostgreSQL through a [`KillProxy`].
fn setup() -> (KillProxy, Arc<LifeguardPool>) {
    let url = role_url(&crate::context::get_test_context().pg_url);
    let (scheme, rest) = url.split_once("://").expect("scheme");
    let (user, hostpath) = rest.rsplit_once('@').expect("userinfo");
    let (hostport, path) = hostpath.split_once('/').unwrap_or((hostpath, "postgres"));
    let backend = std::net::ToSocketAddrs::to_socket_addrs(hostport)
        .expect("resolve")
        .next()
        .expect("addr");
    let proxy = KillProxy::start(backend);
    let url = format!("{scheme}://{user}@{}/{path}", proxy.addr);
    let pool = Arc::new(LifeguardPool::new(&url, 1, Vec::new(), 0).expect("pool"));
    (proxy, pool)
}

fn no_params() -> Values {
    Values(Vec::new())
}

fn org_of(exec: &PooledLifeExecutor, sql: &str) -> Result<String, lifeguard::LifeError> {
    exec.query_one_values(sql, &no_params()).map(|r| r.get(0))
}

/// Give the connection's I/O coroutine time to see the cut and mark the client closed.
fn settle() {
    std::thread::sleep(Duration::from_millis(300));
}

#[test]
fn a_connection_dead_at_checkout_is_replaced_and_the_unit_succeeds() {
    let t = fixture();
    let (proxy, pool) = setup();
    let exec = PooledLifeExecutor::new(Arc::clone(&pool)).with_session_context(ctx(ORG_A));
    assert_eq!(org_of(&exec, ORG_GUC).expect("warm"), ORG_A);

    proxy.cut_all();
    settle();
    let mut b = StatementBatch::new();
    let g = b.query_opt(ORG_GUC, &no_params()).expect("q");
    let n = b
        .query_opt(&format!("SELECT count(*)::int8 FROM {t}"), &no_params())
        .expect("q");
    let mut out = pool
        .with_session_batch(&ctx(ORG_A), b)
        .expect("batch on the replaced connection");
    let g: String = out.take(g).expect("row").get(0);
    let n: i64 = out.take(n).expect("row").get(0);
    assert_eq!(g, ORG_A, "context set on the new connection");
    assert_eq!(n, 2, "policy applied on the new connection");
    assert_eq!(pool.presend_retry_counts(), (1, 0));
}

#[test]
fn an_rls_unit_failing_before_it_sends_is_retried_once_with_its_context() {
    let (proxy, pool) = setup();
    let exec = PooledLifeExecutor::new(Arc::clone(&pool)).with_session_context(ctx(ORG_A));
    assert_eq!(org_of(&exec, ORG_GUC).expect("warm"), ORG_A);

    let marker = "lg-presend-cut-unit";
    proxy.cut_on(marker, 1);
    let g = org_of(&exec, &format!("{ORG_GUC} /* {marker} */")).expect("retried unit");
    assert_eq!(g, ORG_A, "context set on the new connection");
    assert_eq!(pool.presend_retry_counts(), (0, 1));
}

#[test]
fn a_batch_failing_before_it_sends_is_retried_once_with_its_context() {
    let t = fixture();
    let (proxy, pool) = setup();
    let marker = "lg-presend-cut-batch";
    proxy.cut_on(marker, 1);
    let mut b = StatementBatch::new();
    let g = b
        .query_opt(&format!("{ORG_GUC} /* {marker} */"), &no_params())
        .expect("q");
    let n = b
        .query_opt(&format!("SELECT count(*)::int8 FROM {t}"), &no_params())
        .expect("q");
    let mut out = pool
        .with_session_batch(&ctx(ORG_A), b)
        .expect("retried batch");
    let g: String = out.take(g).expect("row").get(0);
    let n: i64 = out.take(n).expect("row").get(0);
    assert_eq!(g, ORG_A);
    assert_eq!(n, 2);
    assert_eq!(pool.presend_retry_counts(), (0, 1));
}

#[test]
fn a_session_transaction_failing_before_it_sends_is_retried_once_with_its_context() {
    let t = fixture();
    let (proxy, pool) = setup();
    let marker = "lg-presend-cut-tx";
    proxy.cut_on(marker, 1);
    let before = total_rows(&t);
    let g: String = pool
        .with_session_transaction(&ctx(ORG_A), |tx| {
            let g: String = tx
                .query_one_values(&format!("{ORG_GUC} /* {marker} */"), &no_params())?
                .get(0);
            tx.execute_values(
                &format!("INSERT INTO {t} (org, qty) VALUES ($1, 5)"),
                &Values(vec![Value::from(ORG_A)]),
            )?;
            Ok(g)
        })
        .expect("retried transaction");
    assert_eq!(g, ORG_A);
    assert_eq!(total_rows(&t), before + 1, "written once");
    assert_eq!(pool.presend_retry_counts(), (0, 1));
}

#[test]
fn a_unit_is_retried_at_most_once() {
    let (proxy, pool) = setup();
    let exec = PooledLifeExecutor::new(Arc::clone(&pool)).with_session_context(ctx(ORG_A));
    let marker = "lg-presend-cut-always";
    proxy.cut_on(marker, usize::MAX);
    let r = org_of(&exec, &format!("{ORG_GUC} /* {marker} */"));
    assert!(r.is_err(), "second pre-send failure is returned: {r:?}");
    assert_eq!(pool.presend_retry_counts(), (0, 1));

    // The failed unit marked the slot broken; it is replaced in the background.
    proxy.cut_on(marker, 0);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let g = loop {
        match org_of(&exec, ORG_GUC) {
            Ok(g) => break g,
            Err(e) if std::time::Instant::now() < deadline => {
                let _ = e;
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("pool did not recover: {e}"),
        }
    };
    assert_eq!(g, ORG_A);
}

#[test]
fn a_failure_after_the_unit_sent_is_returned_and_not_retried() {
    let t = fixture();
    let (proxy, pool) = setup();
    let exec = PooledLifeExecutor::new(Arc::clone(&pool)).with_session_context(ctx(ORG_A));
    assert_eq!(org_of(&exec, ORG_GUC).expect("warm"), ORG_A);
    let before = total_rows(&t);

    // The marker rides in a parameter: the proxy sees it in the unit's write, after the
    // statements were prepared. If the unit were retried the row would be written.
    let marker = "lg-after-send-cut";
    proxy.cut_on(marker, 1);
    let mut b = StatementBatch::new();
    let _ = b
        .execute(
            &format!("INSERT INTO {t} (org, qty) SELECT $1, 7 WHERE $2::text IS NOT NULL"),
            &Values(vec![Value::from(ORG_A), Value::from(marker)]),
        )
        .expect("stmt");
    let r = pool.with_session_batch(&ctx(ORG_A), b);
    assert!(r.is_err(), "the cut after sending is returned");
    assert_eq!(pool.presend_retry_counts(), (0, 0));
    assert_eq!(total_rows(&t), before, "not retried");
}
