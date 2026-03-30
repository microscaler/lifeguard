//! WAL Lag Monitoring
//!
//! Spawns a background **OS thread** to periodically poll PostgreSQL replica lag so routing does
//! not depend on the `may` scheduler or coroutine stack size (same rationale as [`super::pooled`] workers).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Tracks whether a configured read replica is “lagging” for routing decisions.
///
/// A background thread polls the replica periodically; while lag is above an internal
/// threshold, callers should treat reads as unsafe on the replica tier. See
/// [`WalLagMonitor::start_monitor`] and the connection pooling PRD for evolution of lag semantics.
#[derive(Clone)]
pub struct WalLagMonitor {
    is_lagging: Arc<AtomicBool>,
}

impl WalLagMonitor {
    /// Starts a background thread that polls the database for WAL lag every `poll_interval`
    /// (minimum **10ms**).
    ///
    /// Production default is **500ms**; use [`Self::start_monitor`] for that.
    pub fn start_monitor_with_poll_interval(
        replica_conn_string: String,
        poll_interval: Duration,
    ) -> Self {
        let poll_interval = poll_interval.max(Duration::from_millis(10));
        let is_lagging = Arc::new(AtomicBool::new(false));
        let lag_ref = is_lagging.clone();

        let handle = thread::spawn(move || {
                // Retry initial connect with backoff (PRD R7.1): transient replica/startup failures
                // must not permanently disable read routing for the process lifetime.
                let mut backoff = Duration::from_millis(200);
                let backoff_cap = Duration::from_secs(5);
                let client = loop {
                    match may_postgres::connect(&replica_conn_string) {
                        Ok(c) => break c,
                        Err(_) => {
                            lag_ref.store(true, Ordering::Release);
                            thread::sleep(backoff);
                            backoff = (backoff * 2).min(backoff_cap);
                        }
                    }
                };

                loop {
                    thread::sleep(poll_interval);

                    let query = "
                    SELECT (
                        CASE WHEN pg_is_in_recovery() THEN
                            pg_wal_lsn_diff(
                                pg_last_wal_receive_lsn(),
                                pg_last_wal_replay_lsn()
                            )::bigint
                        ELSE
                            0::bigint
                        END
                    ) AS lag_bytes
                ";

                    match client.query_one(query, &[]) {
                        Ok(row) => {
                            let lag_bytes: i64 = row.get(0);
                            let lagging = lag_bytes > 1_000_000;
                            lag_ref.store(lagging, Ordering::Release);
                        }
                        Err(_) => {
                            lag_ref.store(true, Ordering::Release);
                        }
                    }
                }
            });

        #[allow(clippy::mem_forget)]
        std::mem::forget(handle);

        Self {
            is_lagging,
        }
    }

    /// Starts a background thread that polls the database for WAL lag every **500ms**.
    #[must_use]
    pub fn start_monitor(replica_conn_string: String) -> Self {
        Self::start_monitor_with_poll_interval(
            replica_conn_string,
            Duration::from_millis(500),
        )
    }

    /// Check if the replica is currently lagging
    pub fn is_replica_lagging(&self) -> bool {
        self.is_lagging.load(Ordering::Acquire)
    }
}
