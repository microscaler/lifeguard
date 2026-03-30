//! Connectivity vs application errors for pool **slot heal** (PRD §5.5).
//!
//! # Taxonomy (R5.1)
//!
//! | Class | Heal? | Notes |
//! |-------|-------|--------|
//! | `may_postgres::Error`: closed connection, transport, SQLSTATE **08\*\*\*** (connection exception), **57P01** / **57P02** / **57P03** (shutdown) | **Yes** | Replace `Client` in the worker slot. |
//! | `std::io::Error` in cause chain: `ConnectionReset`, `BrokenPipe`, `UnexpectedEof`, … | **Yes** | Half-open TCP / dropped connection. |
//! | SQL errors (constraints, syntax, …), `LifeError::QueryError` | **No** | R5.3 — do not heal on generic statement failure. |
//!
//! Use [`life_error_is_connectivity_heal_candidate`] on errors from pool `execute` / `query_*`
//! paths before reopening a session.

use crate::executor::LifeError;
use may_postgres::Error as PostgresError;
use std::io;

/// `true` when the pool worker may drop the current [`may_postgres::Client`] and open a new one.
#[must_use]
pub(crate) fn life_error_is_connectivity_heal_candidate(err: &LifeError) -> bool {
    match err {
        LifeError::PostgresError(e) => postgres_error_is_connectivity_heal_candidate(e),
        _ => false,
    }
}

#[must_use]
pub(crate) fn postgres_error_is_connectivity_heal_candidate(e: &PostgresError) -> bool {
    if e.is_closed() {
        return true;
    }
    if io_chain_suggests_dead_transport(e) {
        return true;
    }
    if let Some(code) = e.code() {
        let c = code.code();
        if c.starts_with("08") {
            return true;
        }
        if matches!(c, "57P01" | "57P02" | "57P03") {
            return true;
        }
    }
    // Stable `Display` strings for non-`Db` kinds (may_postgres `Kind`).
    let msg = e.to_string();
    msg.contains("error communicating with the server")
        || msg.contains("error connecting to server")
        || msg.contains("timeout waiting for server")
        || msg.contains("connection closed")
}

fn io_chain_suggests_dead_transport(e: &PostgresError) -> bool {
    io_chain_suggests_dead_transport_dyn(e as &(dyn std::error::Error + 'static))
}

fn io_chain_suggests_dead_transport_dyn(e: &(dyn std::error::Error + 'static)) -> bool {
    let mut cur: Option<&dyn std::error::Error> = Some(e);
    while let Some(err) = cur {
        if let Some(io) = err.downcast_ref::<io::Error>() {
            match io.kind() {
                io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::BrokenPipe
                | io::ErrorKind::UnexpectedEof => return true,
                _ => {}
            }
        }
        cur = err.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::LifeError;

    #[test]
    fn query_error_is_not_heal_candidate() {
        assert!(!life_error_is_connectivity_heal_candidate(&LifeError::QueryError(
            "syntax".into(),
        )));
    }

    #[test]
    fn other_error_is_not_heal_candidate() {
        assert!(!life_error_is_connectivity_heal_candidate(&LifeError::Other(
            "oops".into(),
        )));
    }

    #[test]
    fn io_chain_detects_connection_reset() {
        let inner = io::Error::new(io::ErrorKind::ConnectionReset, "reset");
        let boxed: Box<dyn std::error::Error + Send + Sync> = Box::new(inner);
        assert!(super::io_chain_suggests_dead_transport_dyn(boxed.as_ref()));
    }
}
