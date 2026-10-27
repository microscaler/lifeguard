//! Per-connection prepared-statement cache for pool workers.
//!
//! `may_postgres` runs a `&str` query as Parse+Describe+Sync (one round trip), then
//! Bind+Execute+Sync (a second), then sends Close for the throw-away statement. ORM queries repeat
//! the same handful of SQL strings, so each pool worker keeps the [`Statement`]s it has prepared
//! on its connection and reuses them: one round trip per query instead of two.
//!
//! Each pool slot is one OS thread owning one connection, so the cache is thread-local to that
//! worker. Anything that replaces the worker's connection (heal, max-lifetime rotation) calls
//! [`clear`]; a slot replaced after a wedge gets a fresh thread and therefore a fresh cache.
//!
//! Capacity per connection comes from `LIFEGUARD_STATEMENT_CACHE_CAPACITY` (default
//! [`DEFAULT_CAPACITY`]); `0` turns caching off and restores prepare-per-call. When the cache is
//! full it is emptied, which is crude but cheap and only happens to services issuing more distinct
//! statements than the capacity (dynamic `IN (...)` lists are the usual cause).
//!
//! A cached statement can go stale when the schema changes under it (`cached plan must not change
//! result type`, `0A000`) or when something dropped it server-side (`26000`). [`is_stale_error`]
//! recognises both; the caller evicts and re-prepares once.
//!
//! Through PgBouncer in transaction mode this needs `max_prepared_statements > 0` (PgBouncer
//! 1.21+), exactly as the uncached path already does, since that path also uses named statements.

use may_postgres::{Client, Error, Statement};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Default statements kept per connection.
pub const DEFAULT_CAPACITY: usize = 256;

/// Configured capacity (`LIFEGUARD_STATEMENT_CACHE_CAPACITY`, read once per process).
#[must_use]
pub fn capacity() -> usize {
    static CAP: OnceLock<usize> = OnceLock::new();
    *CAP.get_or_init(|| {
        std::env::var("LIFEGUARD_STATEMENT_CACHE_CAPACITY")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(DEFAULT_CAPACITY)
    })
}

thread_local! {
    static CACHE: RefCell<HashMap<Box<str>, Statement>> = RefCell::new(HashMap::new());
}

/// Whether this lookup was served from the cache (for metrics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lookup {
    /// Statement was already prepared on this connection.
    Hit,
    /// Statement was prepared now (and cached when caching is on).
    Miss,
}

/// The prepared statement for `query` on this worker's connection, preparing it on a miss.
///
/// # Errors
///
/// Returns the prepare error (syntax error, unknown relation, ...) unchanged.
pub fn get_or_prepare(client: &Client, query: &str) -> Result<(Statement, Lookup), Error> {
    let cap = capacity();
    if cap == 0 {
        return client.prepare(query).map(|s| (s, Lookup::Miss));
    }
    if let Some(stmt) = CACHE.with(|c| c.borrow().get(query).cloned()) {
        return Ok((stmt, Lookup::Hit));
    }
    let stmt = client.prepare(query)?;
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= cap {
            c.clear();
        }
        c.insert(query.into(), stmt.clone());
    });
    Ok((stmt, Lookup::Miss))
}

/// Drop one cached statement (after a stale-statement error).
pub fn evict(query: &str) {
    CACHE.with(|c| {
        c.borrow_mut().remove(query);
    });
}

/// Forget every statement: the worker's connection was replaced.
pub fn clear() {
    CACHE.with(|c| c.borrow_mut().clear());
}

/// `true` for errors that mean a cached statement no longer matches the server.
#[must_use]
pub fn is_stale_error(e: &Error) -> bool {
    match e.code().map(may_postgres::error::SqlState::code) {
        // invalid_sql_statement_name: the server no longer has the statement.
        Some("26000") => true,
        // feature_not_supported is also raised for "cached plan must not change result type".
        Some("0A000") => e
            .as_db_error()
            .is_some_and(|d| d.message().contains("cached plan")),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_capacity_is_bounded() {
        const { assert!(DEFAULT_CAPACITY > 0 && DEFAULT_CAPACITY <= 1024) };
    }

    #[test]
    fn clear_and_evict_are_safe_on_empty_cache() {
        clear();
        evict("SELECT 1");
        CACHE.with(|c| assert!(c.borrow().is_empty()));
    }
}
