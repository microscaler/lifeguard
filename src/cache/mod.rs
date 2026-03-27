//! Cache Coherence Architecture mapping
//!
//! Exposes interfaces and default implementations for the transparent
//! caching layer (LifeReflector) over ActiveModel behaviors.

pub mod provider;
pub mod redis_provider;

pub use provider::{CacheProvider, CacheError, DefaultCacheProvider, CachedResult};
pub use redis_provider::RedisCacheProvider;
