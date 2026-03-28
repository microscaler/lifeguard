//! Cache provider trait definitions and implementation stubs
//!
//! This module defines the `CacheProvider` trait formatting the boundaries
//! for `LifeReflector`'s caching mechanisms (i.e. Redis write-through/fallbacks).

/// Represents cache-layer operational errors.
#[derive(Debug)]
pub enum CacheError {
    Connection(String),
    Serialization(String),
    NotFound(String),
    Internal(String),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connection(msg) => write!(f, "Cache connection failed: {msg}"),
            Self::Serialization(msg) => write!(f, "Cache serialization failed: {msg}"),
            Self::NotFound(msg) => write!(f, "Cache key not found: {msg}"),
            Self::Internal(msg) => write!(f, "Internal cache error: {msg}"),
        }
    }
}

impl std::error::Error for CacheError {}

/// A wrapper for cached entity results aiming to abstract caching serialization logic.
pub enum CachedResult<T> {
    Hit(T),
    Miss,
}

/// Abstract Trait characterizing the fundamental operations needed for
/// the Lifeguard ORM Cache Coherence system.
pub trait CacheProvider: Send + Sync {
    /// Attempts to fetch a cached model based on its fully qualified key schema.
    ///
    /// The `key` string acts as universally identifiable locator (e.g. `lifeguard:model:users:42`)
    fn get(&self, key: &str) -> Result<CachedResult<String>, CacheError>;

    /// Stores a serialized model representation mapping to the specified key.
    ///
    /// Optionally accepts a `ttl_seconds` argument configuring the Active Set expiration constraints.
    fn set(&self, key: &str, value: &str, ttl_seconds: Option<u64>) -> Result<(), CacheError>;

    /// Invalidates/deletes a specific key, forcibly enforcing caching evictions logic prior to `NOTIFY`.
    fn invalidate(&self, key: &str) -> Result<(), CacheError>;
}

/// The default transparent provider utilized when caching is inactive or unconfigured.
///
/// Ensures safe compile-time linkage through `ActiveModelBehavior` default implementations
/// without enforcing Redis runtime burdens on applications bypassing caching.
#[derive(Clone, Default)]
pub struct DefaultCacheProvider;

impl CacheProvider for DefaultCacheProvider {
    fn get(&self, _key: &str) -> Result<CachedResult<String>, CacheError> {
        // By default, the system assumes caching is "Miss" ensuring pass-through db retrieval
        Ok(CachedResult::Miss)
    }

    fn set(&self, _key: &str, _value: &str, _ttl_seconds: Option<u64>) -> Result<(), CacheError> {
        // No-op if unconfigured
        Ok(())
    }

    fn invalidate(&self, _key: &str) -> Result<(), CacheError> {
        // No-op if unconfigured
        Ok(())
    }
}
