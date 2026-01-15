
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

// Executor module - Epic 01 Story 03
pub mod executor;

// Raw SQL helpers - Epic 01 Story 04
pub mod raw_sql;

// Transaction module - Epic 01 Story 06
pub mod transaction;

// Macros will be rebuilt in Epic 02-03
// mod macros;

pub mod metrics;

// Pool will be rebuilt in Epic 04
// pub mod pool;

// Test helpers - Epic 01 Story 08
#[cfg(test)]
pub mod test_helpers;

// Entity tests will be rebuilt in Epic 03
// mod tests_cfg;

// Public API will be rebuilt in Epic 04
// pub use pool::LifeguardPool;

// Re-export connection types for convenience
pub use connection::{
    connect, validate_connection_string, check_connection_health,
    check_connection_health_with_timeout, ConnectionError, ConnectionString,
};

// Re-export executor types for convenience
pub use executor::{LifeExecutor, LifeError, MayPostgresExecutor};

// Query builder - Epic 02 Story 03
pub mod query;
pub use query::{SelectQuery, FromRow, LifeEntityName, LifeModelTrait, ColumnTrait, ColumnDefinition, PrimaryKeyTrait, PrimaryKeyToColumn, PrimaryKeyArity, PrimaryKeyArityTrait};

// ActiveModel operations - Epic 02 Story 07
pub mod active_model;
pub use active_model::{ActiveModelTrait, ActiveModelError};

// Model trait - Core Traits & Types
pub mod model;
pub use model::{ModelError, ModelTrait};

// Re-export raw SQL helpers for convenience
pub use raw_sql::{
    execute_unprepared, execute_statement, find_by_statement, find_all_by_statement, query_value,
};

// Re-export transaction types for convenience
pub use transaction::{Transaction, TransactionError, IsolationLevel};
