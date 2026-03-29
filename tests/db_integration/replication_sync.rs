//! Wait for physical replication to replay past a primary WAL LSN (design doc §4, strategy 1).

use std::thread;
use std::time::{Duration, Instant};

/// Returns `pg_current_wal_lsn()` from `primary_url` as text (e.g. `0/1234ABCD`).
pub fn primary_current_wal_lsn(primary_url: &str) -> Result<String, String> {
    let client = may_postgres::connect(primary_url).map_err(|e| e.to_string())?;
    let row = client
        .query_one("SELECT pg_current_wal_lsn()::text AS lsn", &[])
        .map_err(|e| e.to_string())?;
    let s: String = row.get(0);
    Ok(s.trim().to_string())
}

/// Poll `replica_url` until `pg_last_wal_replay_lsn() >= min_lsn::pg_lsn` or timeout.
pub fn wait_replica_replayed_at_least(
    replica_url: &str,
    min_lsn: &str,
    timeout: Duration,
    poll: Duration,
) -> Result<(), String> {
    let start = Instant::now();
    loop {
        let client = match may_postgres::connect(replica_url) {
            Ok(c) => c,
            Err(e) => {
                if start.elapsed() >= timeout {
                    return Err(format!(
                        "wait_replica_replayed_at_least: connect failed after {timeout:?}: {e}"
                    ));
                }
                thread::sleep(poll);
                continue;
            }
        };
        let ok: bool = match client.query_one(
            "SELECT (pg_last_wal_replay_lsn() >= $1::pg_lsn) AS ok",
            &[&min_lsn],
        ) {
            Ok(row) => row.get(0),
            Err(e) => {
                if start.elapsed() >= timeout {
                    return Err(format!(
                        "wait_replica_replayed_at_least: query failed (min_lsn={min_lsn}): {e}"
                    ));
                }
                thread::sleep(poll);
                continue;
            }
        };
        if ok {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            let replay: String = client
                .query_one("SELECT pg_last_wal_replay_lsn()::text", &[])
                .map_or_else(|_| "(unknown)".into(), |r| r.get::<_, String>(0));
            return Err(format!(
                "wait_replica_replayed_at_least: timeout {timeout:?} min_lsn={min_lsn} last_replay_lsn={replay}"
            ));
        }
        thread::sleep(poll);
    }
}

/// True if the instance reports `pg_is_in_recovery()`.
pub fn postgres_is_in_recovery(url: &str) -> Result<bool, String> {
    let client = may_postgres::connect(url).map_err(|e| e.to_string())?;
    let row = client
        .query_one("SELECT pg_is_in_recovery() AS ir", &[])
        .map_err(|e| e.to_string())?;
    Ok(row.get(0))
}
