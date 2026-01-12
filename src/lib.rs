//! # Lifeguard
//!
//! Coroutine-native PostgreSQL ORM and data access platform for Rust's `may` runtime.
//!
//! See [README on GitHub](https://github.com/microscaler/lifeguard) for full architecture.
//!
//! ## Status
//!
//! Currently being rebuilt from scratch. Epic 01 in progress.
//!
//! ## Architecture
//!
//! - **may_postgres**: Coroutine-native PostgreSQL client (foundation)
//! - **LifeQuery**: SQL builder layer (Epic 02)
//! - **LifeModel/LifeRecord**: ORM layer (Epic 03)
//! - **LifeExecutor**: Database execution abstraction (Epic 04)
//! - **LifeguardPool**: Persistent connection pool (Epic 04)

pub mod config;

// Connection module - Epic 01 Story 02
pub mod connection;

// Macros will be rebuilt in Epic 02-03
// mod macros;

pub mod metrics;

// Pool will be rebuilt in Epic 04
// pub mod pool;

// Test helpers will be rebuilt in Epic 01 Story 08
// mod test_helpers;

// Entity tests will be rebuilt in Epic 03
// mod tests_cfg;

// Public API will be rebuilt in Epic 04
// pub use pool::LifeguardPool;

// Re-export connection types for convenience
pub use connection::{connect, validate_connection_string, ConnectionError, ConnectionString};
