//! WAL Lag Monitoring
//!
//! Spawns a background coroutine to periodically poll PostgreSQL replica lag to determine
//! if reads are safe to route to the replica or if they must fall back to the primary.

use may::coroutine;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct WalLagMonitor {
    is_lagging: Arc<AtomicBool>,
}

impl WalLagMonitor {
    /// Starts a background coroutine that polls the database for WAL lag
    /// 
    /// In a real implementation, this will use a dedicated `may_postgres::Client`
    /// to poll `pg_current_wal_lsn()` and `pg_last_wal_replay_lsn()` every 500ms.
    pub fn start_monitor(replica_conn_string: String) -> Self {
        let is_lagging = Arc::new(AtomicBool::new(false));
        let lag_ref = is_lagging.clone();
        
        unsafe {
            coroutine::spawn::<_, ()>(move || {
            // Attempt to connect inside the coroutine so it doesn't block startup
            // If connection fails, we assume lag is true to be safe and fall back to primary
            let client = match may_postgres::connect(&replica_conn_string) {
                Ok(c) => c,
                Err(_) => {
                    lag_ref.store(true, Ordering::Release);
                    return;
                }
            };

            loop {
                // Poll every 500ms as per design
                coroutine::sleep(Duration::from_millis(500));
                
                // Simplified replica check using pg_is_in_recovery() and WAL replay
                // (In PostgreSQL 10+, lsns are used: pg_last_wal_replay_lsn)
                let query = "
                    SELECT 
                        CASE WHEN pg_is_in_recovery() THEN
                            pg_wal_lsn_diff(pg_last_wal_receive_lsn(), pg_last_wal_replay_lsn())
                        ELSE
                            0
                        END as lag_bytes
                ";

                match client.query_one(query, &[]) {
                    Ok(row) => {
                        let lag_bytes: i64 = row.get(0);
                        // Define threshold (e.g., > 1 MB lag)
                        let lagging = lag_bytes > 1_000_000;
                        lag_ref.store(lagging, Ordering::Release);
                    }
                    Err(_) => {
                        // Statement failure does not imply a disposable connection (cf. pool
                        // managers that do not reset sessions on SQL error). Treat as unknown
                        // lag: route reads to primary until the next successful poll.
                        lag_ref.store(true, Ordering::Release);
                    }
                }
            }
            });
        }

        Self {
            is_lagging,
        }
    }

    /// Check if the replica is currently lagging
    pub fn is_replica_lagging(&self) -> bool {
        self.is_lagging.load(Ordering::Acquire)
    }
}
