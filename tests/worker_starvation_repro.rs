//! Reproducer for the `PriceWhisperer` orders wedge of 2 Oct 2026.
//!
//! The pool's workers are OS threads, but every byte a `may_postgres::Client` reads or writes is
//! moved by that connection's I/O coroutine, and that coroutine runs on - and is woken by the
//! epoll selector of - one of `may`'s worker threads. When application code running in a
//! coroutine makes an OS-blocking call (a crossbeam `send` into a full queue, a crossbeam
//! `recv_timeout`, a `std::sync::Mutex` held across a park), it does not park the coroutine: it
//! blocks the may worker thread under it. Every coroutine queued on that worker, and every socket
//! whose fd hashes to its selector, stops.
//!
//! In orders the blocking call was `pricewhisperer-broker`'s `IbkrTwsAdapter::call` (crossbeam
//! `send` into a bounded(64) queue, then `recv_timeout(10s)`) while the live IB Gateway was hung.
//! Each page poll that touched the broker took a may worker out for good; once enough were gone,
//! every pool reply missed its budget (`PoolReplyTimeout` after 20 s on every call), replacing a
//! slot could not help (the fresh connection's I/O coroutine lands on the same starved threads),
//! and `/health` - served by the workers that were left - stayed green for seven hours.
//!
//! This test builds the same situation in miniature: 4 may workers, an 8-slot pool, then three
//! coroutines blocked the way the broker adapter blocks. Pool calls made from a plain OS thread
//! (so the CALLER is never starved, only the connection I/O) must then fail within their budgets -
//! and must not hang: the pool used to reconnect a timed-out slot inline on the caller, and that
//! connect needs the starved runtime too.
//!
//! Run with:
//!
//! ```text
//! TEST_DATABASE_URL=postgresql://postgres:postgres@127.0.0.1:55432/postgres \
//!   cargo test --test worker_starvation_repro -- --nocapture --test-threads=1
//! ```

use lifeguard::{
    LifeError, LifeExecutor, LifeguardPool, LifeguardPoolSettings, PooledLifeExecutor,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const WORKERS: usize = 4;
const SLOTS: usize = 8;
const ROUNDS: usize = 32;

fn healthy_round(exec: &PooledLifeExecutor) -> (usize, usize, usize) {
    let (mut ok, mut timed_out, mut other) = (0, 0, 0);
    for _ in 0..ROUNDS {
        match exec.query_one("SELECT 1", &[]) {
            Ok(_) => ok += 1,
            Err(LifeError::PoolReplyTimeout { .. }) => timed_out += 1,
            Err(e) => {
                eprintln!("REPRO other error: {e}");
                other += 1;
            }
        }
    }
    (ok, timed_out, other)
}

#[test]
fn os_blocking_calls_in_coroutines_starve_pool_io() {
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        eprintln!("REPRO skipped: TEST_DATABASE_URL unset");
        return;
    };
    may::config().set_workers(WORKERS);

    let settings = LifeguardPoolSettings {
        reply_timeout: Some(Duration::from_millis(1500)),
        acquire_timeout: Duration::from_secs(5),
        ..LifeguardPoolSettings::default()
    };
    let pool = Arc::new(
        LifeguardPool::new_with_settings(&url, SLOTS, vec![], 0, &settings).expect("pool"),
    );
    let exec = PooledLifeExecutor::new(pool);

    let (ok, timed_out, other) = healthy_round(&exec);
    eprintln!("REPRO healthy:  ok={ok} timed_out={timed_out} other={other}");
    assert_eq!(ok, ROUNDS, "the pool must work before anything blocks");

    // A consumer that has hung (the IBKR connection manager stuck on a dead gateway): its bounded
    // queue is full and nobody drains it. Keep the receiver alive so sends block rather than fail.
    let (tx, _hung_consumer) = crossbeam_channel::bounded::<usize>(1);
    tx.send(0).expect("fill the queue");
    for i in 0..(WORKERS - 1) {
        let tx = tx.clone();
        // From the coroutine's point of view this "waits for the broker". It actually blocks the
        // may worker thread it runs on, forever.
        let _ = unsafe {
            may::coroutine::spawn(move || {
                let _ = tx.send(i + 1);
            })
        };
        std::thread::sleep(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_millis(300));

    // The starved round runs on its own thread: a call can hang outright (the slot-replacement
    // path connects inline, and a connect needs the same starved runtime), and the test must still
    // report what it saw.
    let ok = Arc::new(AtomicUsize::new(0));
    let timed_out = Arc::new(AtomicUsize::new(0));
    let other = Arc::new(AtomicUsize::new(0));
    let (ok_n, timeout_n, other_n) = (Arc::clone(&ok), Arc::clone(&timed_out), Arc::clone(&other));
    let started = Instant::now();
    std::thread::spawn(move || {
        for i in 0..ROUNDS {
            let call = Instant::now();
            let result = exec.query_one("SELECT 1", &[]);
            let what = match &result {
                Ok(_) => {
                    ok_n.fetch_add(1, Ordering::SeqCst);
                    "ok".to_string()
                }
                Err(LifeError::PoolReplyTimeout { .. }) => {
                    timeout_n.fetch_add(1, Ordering::SeqCst);
                    "PoolReplyTimeout".to_string()
                }
                Err(err) => {
                    other_n.fetch_add(1, Ordering::SeqCst);
                    format!("error: {err}")
                }
            };
            eprintln!("REPRO call {i:2}: {what} in {:?}", call.elapsed());
        }
    });
    let deadline = Instant::now() + Duration::from_secs(90);
    while Instant::now() < deadline
        && ok.load(Ordering::SeqCst)
            + timed_out.load(Ordering::SeqCst)
            + other.load(Ordering::SeqCst)
            < ROUNDS
    {
        std::thread::sleep(Duration::from_millis(200));
    }
    let (ok, timed_out, other) = (
        ok.load(Ordering::SeqCst),
        timed_out.load(Ordering::SeqCst),
        other.load(Ordering::SeqCst),
    );
    let hung = ROUNDS - ok - timed_out - other;
    eprintln!(
        "REPRO starved:  ok={ok} timed_out={timed_out} other={other} never_returned={hung} ({} of {WORKERS} may workers blocked, {:?})",
        WORKERS - 1,
        started.elapsed()
    );
    assert!(
        timed_out + other > 0,
        "with may workers blocked by OS-blocking calls, connection I/O on them stalls and pool calls must fail"
    );
    // The regression: before lifeguard moved slot replacement off the caller, the first timed-out
    // call reconnected INLINE - a connect needs the starved runtime too - and never returned
    // (32 of 32 calls hung for 90 s here, reply budget 1.5 s).
    assert_eq!(
        hung, 0,
        "pool calls must return within their budgets even when the runtime is starved"
    );
    // The blocked coroutines and any heal thread stuck in a connect end with the process.
}
