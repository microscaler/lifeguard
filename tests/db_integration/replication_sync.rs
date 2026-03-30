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
///
/// Uses **one** server session for all poll queries (no connect-per-interval). Initial
/// `may_postgres::connect` may retry until the deadline while the standby is still coming up.
///
/// A failed query does **not** imply a bad connection: on `query_one` error we return immediately
/// rather than opening a new session (same idea as pool managers that do not recycle on SQL
/// failure—lifecycle belongs to connectivity/idle policy, not statement outcome).
pub fn wait_replica_replayed_at_least(
    replica_url: &str,
    min_lsn: &str,
    timeout: Duration,
    poll: Duration,
) -> Result<(), String> {
    let start = Instant::now();
    let deadline = start + timeout;

    fn connect_until_deadline(
        url: &str,
        deadline: Instant,
        poll: Duration,
    ) -> Result<may_postgres::Client, String> {
        loop {
            match may_postgres::connect(url) {
                Ok(c) => return Ok(c),
                Err(e) => {
                    if Instant::now() >= deadline {
                        return Err(e.to_string());
                    }
                    thread::sleep(poll);
                }
            }
        }
    }

    let client = connect_until_deadline(replica_url, deadline, poll).map_err(|e| {
        format!("wait_replica_replayed_at_least: connect failed after {timeout:?}: {e}")
    })?;

    if !min_lsn
        .bytes()
        .all(|b| b.is_ascii_hexdigit() || b == b'/')
    {
        return Err(format!(
            "wait_replica_replayed_at_least: invalid min_lsn (expected pg_lsn text): {min_lsn}"
        ));
    }

    loop {
        // `may_postgres` cannot bind Rust `&str` as `pg_lsn`; embed validated LSN from our own
        // `pg_current_wal_lsn()::text` query only.
        let sql = format!("SELECT (pg_last_wal_replay_lsn() >= '{min_lsn}'::pg_lsn) AS ok");
        let ok: bool = client
            .query_one(sql.as_str(), &[])
            .map_err(|e| {
                format!("wait_replica_replayed_at_least: query failed (min_lsn={min_lsn}): {e}")
            })?
            .get(0);

        if ok {
            return Ok(());
        }

        if Instant::now() >= deadline {
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
