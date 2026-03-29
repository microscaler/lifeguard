//! Connection pool for `may` + `may_postgres`.
//!
//! - [`LifeguardPool`]: one `may` worker coroutine per connection, round-robin dispatch.
//! - [`PooledLifeExecutor`]: [`crate::executor::LifeExecutor`] over the pool via
//!   [`crate::executor::LifeExecutor::execute_values`] / `query_*_values` (and ORM paths).
//! - [`OwnedParam`]: owned bind parameters for jobs crossing channels.

pub mod config;
pub mod owned_param;
pub mod pooled;
pub mod wal;

pub use owned_param::OwnedParam;
pub use pooled::{LifeguardPool, PooledLifeExecutor};
