//! WAL Lag Monitoring
//!
//! Spawns a background **OS thread** to periodically poll PostgreSQL replica lag so routing does
//! not depend on the `may` scheduler or coroutine stack size (same rationale as [`super::pooled`] workers).

use super::config::LifeguardPoolSettings;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Policy for when [`WalLagMonitor`] treats a standby as **lagging** (PRD R7.2).
///
/// Byte lag uses `pg_wal_lsn_diff(pg_last_wal_receive_lsn(), pg_last_wal_replay_lsn())` on the
/// standby. Optional **apply lag** uses wall-clock time since `pg_last_xact_replay_timestamp()` on
/// the standby (rough “how far behind primary commits” in time when transactions are flowing).
#[derive(Debug, Clone)]
pub struct WalLagPolicy {
    /// **`0`** disables the byte criterion (unless both limits are off — see [`Self::from_pool_settings`]).
    pub max_bytes: u64,
    /// When **Some**, lagging if apply lag exceeds this duration.
    pub max_apply_lag: Option<Duration>,
}

impl Default for WalLagPolicy {
    fn default() -> Self {
        Self {
            max_bytes: 1_000_000,
            max_apply_lag: None,
        }
    }
}

impl WalLagPolicy {
    /// Builds an effective policy from [`LifeguardPoolSettings`].
    ///
    /// If **both** byte and time limits are disabled (`max_bytes == 0` and `max_apply_lag` is `None`),
    /// the historical default **1_000_000** bytes is restored so routing stays conservative.
    #[must_use]
    pub fn from_pool_settings(settings: &LifeguardPoolSettings) -> Self {
        let mut max_bytes = settings.wal_lag_max_bytes;
        let max_apply_lag = settings.wal_lag_max_apply_lag;
        if max_bytes == 0 && max_apply_lag.is_none() {
            max_bytes = 1_000_000;
        }
        Self {
            max_bytes,
            max_apply_lag,
        }
    }

    /// Returns `true` if the replica should be treated as lagging for routing.
    #[must_use]
    pub fn is_lagging(&self, lag_bytes: i64, lag_seconds: Option<f64>) -> bool {
        let mut lagging = false;
        if self.max_bytes > 0 && lag_bytes > i64::try_from(self.max_bytes).unwrap_or(i64::MAX) {
            lagging = true;
        }
        if let Some(limit) = self.max_apply_lag {
            let cap = limit.as_secs_f64();
            if cap > 0.0 {
                if let Some(s) = lag_seconds {
                    if s > cap {
                        lagging = true;
                    }
                }
            }
        }
        lagging
    }
}

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
    /// (minimum **10ms**), using [`WalLagPolicy::default`] (1 MiB byte threshold).
    #[must_use]
    pub fn start_monitor(replica_conn_string: String) -> Self {
        Self::start_monitor_with_poll_interval(
            replica_conn_string,
            Duration::from_millis(500),
            WalLagPolicy::default(),
        )
    }

    /// Starts a background thread that polls the database for WAL lag every `poll_interval`
    /// (minimum **10ms**).
    ///
    /// Production default poll interval is **500ms**; use [`Self::start_monitor`] for that.
    #[must_use]
    pub fn start_monitor_with_poll_interval(
        replica_conn_string: String,
        poll_interval: Duration,
        policy: WalLagPolicy,
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

            let query = "
                SELECT
                    (
                        CASE WHEN pg_is_in_recovery() THEN
                            pg_wal_lsn_diff(
                                pg_last_wal_receive_lsn(),
                                pg_last_wal_replay_lsn()
                            )::bigint
                        ELSE
                            0::bigint
                        END
                    ) AS lag_bytes,
                    (
                        CASE WHEN pg_is_in_recovery()
                             AND pg_last_xact_replay_timestamp() IS NOT NULL THEN
                            EXTRACT(EPOCH FROM (
                                clock_timestamp() - pg_last_xact_replay_timestamp()
                            ))::double precision
                        ELSE
                            NULL::double precision
                        END
                    ) AS lag_seconds
            ";

            loop {
                thread::sleep(poll_interval);

                match client.query_one(query, &[]) {
                    Ok(row) => {
                        let lag_bytes: i64 = row.get(0);
                        let lag_seconds: Option<f64> = row.get(1);
                        let lagging = policy.is_lagging(lag_bytes, lag_seconds);
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

        Self { is_lagging }
    }

    /// Check if the replica is currently lagging
    pub fn is_replica_lagging(&self) -> bool {
        self.is_lagging.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_default_bytes_only_over_1m() {
        let p = WalLagPolicy::default();
        assert!(!p.is_lagging(100, None));
        assert!(p.is_lagging(1_000_001, None));
        assert!(!p.is_lagging(1_000_000, None));
    }

    #[test]
    fn policy_apply_lag_seconds() {
        let p = WalLagPolicy {
            max_bytes: 0,
            max_apply_lag: Some(Duration::from_secs(10)),
        };
        assert!(!p.is_lagging(0, Some(5.0)));
        assert!(p.is_lagging(0, Some(15.0)));
        assert!(!p.is_lagging(0, None));
    }

    #[test]
    fn policy_bytes_or_time() {
        let p = WalLagPolicy {
            max_bytes: 100,
            max_apply_lag: Some(Duration::from_secs(2)),
        };
        assert!(p.is_lagging(200, Some(0.5)));
        assert!(p.is_lagging(50, Some(5.0)));
        assert!(!p.is_lagging(50, Some(1.0)));
    }

    #[test]
    fn from_pool_settings_restores_default_bytes_when_both_off() {
        let s = LifeguardPoolSettings {
            wal_lag_max_bytes: 0,
            wal_lag_max_apply_lag: None,
            ..Default::default()
        };
        let p = WalLagPolicy::from_pool_settings(&s);
        assert_eq!(p.max_bytes, 1_000_000);
        assert!(p.is_lagging(2_000_000, None));
    }
}
