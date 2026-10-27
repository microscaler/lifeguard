//! Per-connection prepared-statement cache.
//!
//! `may_postgres` runs a `&str` query as Parse+Describe+Sync (one round trip), then
//! Bind+Execute+Sync (a second), then sends Close for the throw-away statement. ORM queries repeat
//! the same handful of SQL strings, so each connection keeps the [`Statement`]s it has prepared and
//! reuses them: one round trip per query instead of two.
//!
//! A [`StatementCache`] belongs to exactly one connection. `may_postgres` statements are named per
//! connection, and every `Client` clone of that connection can use them. The pool keeps one cache
//! per slot connection and drops it with the connection, so a replaced connection never sees stale
//! names. [`crate::MayPostgresExecutor`] keeps one for its client.
//!
//! Capacity per connection comes from `LIFEGUARD_STATEMENT_CACHE_CAPACITY` (default
//! [`DEFAULT_CAPACITY`]); `0` turns caching off and restores prepare-per-call. When the cache is
//! full it is emptied, which is crude but cheap and only happens to services issuing more distinct
//! statements than the capacity (dynamic `IN (...)` lists are the usual cause; prefer `= ANY($1)`).
//!
//! A cached statement can go stale when the schema changes under it (`cached plan must not change
//! result type`, `0A000`) or when something dropped it server-side (`26000`). [`is_stale_error`]
//! recognises both; callers evict and re-prepare once.
//!
//! Through PgBouncer in transaction mode this needs `max_prepared_statements > 0` (PgBouncer
//! 1.21+), exactly as the uncached path already does, since that path also uses named statements.

use may_postgres::{Client, Error, Statement};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

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

/// Whether a lookup was served from the cache (for metrics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lookup {
    /// Statement was already prepared on this connection.
    Hit,
    /// Statement was prepared now (and cached when caching is on).
    Miss,
}

/// Prepared statements of one connection, keyed by SQL text.
///
/// Lookups take a read lock for a hash probe; the lock is never held across I/O, so a miss
/// prepares outside it and two callers missing the same SQL at once both prepare (harmless: one
/// statement is kept, the other is closed when dropped).
#[derive(Default)]
pub struct StatementCache {
    map: RwLock<HashMap<Box<str>, Statement>>,
}

impl std::fmt::Debug for StatementCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let n = self.map.read().map(|m| m.len()).unwrap_or(0);
        f.debug_struct("StatementCache")
            .field("entries", &n)
            .finish()
    }
}

impl StatementCache {
    /// Empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The prepared statement for `query` on `client`'s connection, preparing it on a miss.
    ///
    /// `client` must be a handle of the connection this cache belongs to.
    ///
    /// # Errors
    ///
    /// Returns the prepare error (syntax error, unknown relation, ...) unchanged.
    pub fn get_or_prepare(
        &self,
        client: &Client,
        query: &str,
    ) -> Result<(Statement, Lookup), Error> {
        let cap = capacity();
        if cap == 0 {
            return client.prepare(query).map(|s| (s, Lookup::Miss));
        }
        if let Some(stmt) = self
            .map
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(query)
            .cloned()
        {
            return Ok((stmt, Lookup::Hit));
        }
        let stmt = client.prepare(query)?;
        let mut map = self
            .map
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if map.len() >= cap {
            map.clear();
        }
        let kept = map.entry(query.into()).or_insert(stmt).clone();
        Ok((kept, Lookup::Miss))
    }

    /// Drop one cached statement (after a stale-statement error).
    pub fn evict(&self, query: &str) {
        self.map
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(query);
    }

    /// Forget every statement.
    pub fn clear(&self) {
        self.map
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    /// Number of cached statements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    /// `true` when nothing is cached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Run `op` with the cached statement for `query`, evicting and re-preparing once when the
    /// cached statement turns out stale.
    ///
    /// # Errors
    ///
    /// The prepare error, or `op`'s error.
    pub fn with_statement<T>(
        &self,
        client: &Client,
        query: &str,
        mut op: impl FnMut(&Statement) -> Result<T, Error>,
    ) -> Result<(T, Lookup), Error> {
        let (stmt, lookup) = self.get_or_prepare(client, query)?;
        match op(&stmt) {
            Err(e) if lookup == Lookup::Hit && is_stale_error(&e) => {
                self.evict(query);
                let (fresh, _) = self.get_or_prepare(client, query)?;
                op(&fresh).map(|v| (v, Lookup::Miss))
            }
            other => other.map(|v| (v, lookup)),
        }
    }
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
        let c = StatementCache::new();
        c.clear();
        c.evict("SELECT 1");
        assert!(c.is_empty());
        assert_eq!(format!("{c:?}"), "StatementCache { entries: 0 }");
    }
}
