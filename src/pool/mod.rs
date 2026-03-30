//! Connection pool for `may` + `may_postgres`.
//!
//! ## Components
//!
//! - **[`LifeguardPool`]**: one `may` worker coroutine per logical connection, round-robin dispatch
//!   to workers, bounded per-worker job queues, and configurable acquire timeout (see
//!   [`LifeguardPoolSettings`] and [`crate::LifeError::PoolAcquireTimeout`]).
//! - **[`PooledLifeExecutor`]**: [`crate::executor::LifeExecutor`] over the pool via
//!   [`crate::executor::LifeExecutor::execute_values`] / `query_*_values` (and ORM paths).
//! - **[`OwnedParam`]**: owned bind parameters for jobs crossing channels (cannot send `&dyn ToSql`
//!   across threads/coroutine boundaries).
//! - **[`wal::WalLagMonitor`]**: optional background polling used when routing reads to replicas; see
//!   module [`wal`].
//!
//! ## Configuration
//!
//! Use [`LifeguardPool::from_database_config`] when settings map from application config
//! ([`DatabaseConfig`]); otherwise [`LifeguardPool::new_with_settings`] for full control.
//!
//! ## docs.rs note
//!
//! Planning documents and ADRs live only in the Git repository. For pooling design and roadmap,
//! use the [connection pooling PRD](https://github.com/microscaler/lifeguard/blob/main/docs/planning/PRD_CONNECTION_POOLING.md).

pub mod config;
pub mod owned_param;
pub mod pooled;
pub mod wal;

pub use config::{DatabaseConfig, LifeguardPoolSettings};
pub use owned_param::OwnedParam;
pub use pooled::{LifeguardPool, PooledLifeExecutor};
