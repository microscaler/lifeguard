//! # Lifeguard
//!
//! Coroutine-native `PostgreSQL` ORM and data access platform for Rust's `may` runtime.

// Database tools must never panic - ban panic-causing methods in production code
// Note: Tests are allowed to use unwrap/expect via #[cfg(test)] or #[allow] attributes
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::unreachable)]
#![deny(clippy::unimplemented)]
#![deny(clippy::todo)]
// Pedantic is opt-in via `-W clippy::pedantic` in CI; allow the group crate-wide so `-D warnings`
// does not force thousands of doc/style edits. Restriction lints above remain denied.
#![allow(clippy::pedantic)]
#![allow(clippy::type_complexity)] // GraphState stores boxed async closures; aliases would obscure.
//!
//! See [README on GitHub](https://github.com/microscaler/lifeguard) for full architecture.
//!
//! ## Status
//!
//! Currently being rebuilt from scratch. Epic 01 in progress.
//!
//! ## Architecture
//!
//! - **`may_postgres`**: Coroutine-native `PostgreSQL` client (foundation)
//! - **`LifeQuery`**: SQL builder layer (Epic 02)
//! - **`LifeModel`/`LifeRecord`**: ORM layer (Epic 03)
//! - **`LifeExecutor`**: Database execution abstraction (Epic 04)
//! - **`LifeguardPool`**: Persistent connection pool with bounded worker queues, configurable
//!   acquire timeout ([`LifeError::PoolAcquireTimeout`]), and optional [`LifeguardPool::from_database_config`].
//!
//!   See the [connection pooling PRD](https://github.com/microscaler/lifeguard/blob/main/docs/planning/PRD_CONNECTION_POOLING.md)
//!   for the full pooling roadmap (also available in the repository checkout; this link works on **docs.rs**).

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

// Channel-backed logging (may `mpsc` singleton)
pub mod logging;
pub use logging::{
    enqueue, flush_log_channel, global_log_sender, try_enqueue, ChannelLogger, LogLevel, LogMsg,
    LogRecord, CHANNEL_LOG_BRIDGE, init_log_bridge,
};
#[cfg(feature = "tracing")]
pub use logging::{channel_layer, ChannelLayer};

pub mod pool;

// Test helpers - Epic 01 Story 08
// Available for integration tests
pub mod test_helpers;

// Entity tests will be rebuilt in Epic 03
// mod tests_cfg;

pub use pool::{DatabaseConfig, LifeguardPool, LifeguardPoolSettings, OwnedParam, PooledLifeExecutor};

// Optional GraphQL: `LifeModel` nests `async_graphql::SimpleObject` on the generated `Model`.
// Crates that enable `lifeguard`/`graphql` should depend on the same `async-graphql` version
// and enable the scalar features they use (e.g. `chrono`, `uuid`, `decimal`); the workspace
// crate pins these in `[workspace.dependencies]` for tests and internal packages.
#[cfg(feature = "graphql")]
pub use async_graphql;

// Re-export connection types for convenience
pub use connection::{
    connect, validate_connection_string, check_connection_health,
    check_connection_health_with_timeout, ConnectionError, ConnectionString,
};

// Re-export executor types for convenience
pub use executor::{LifeExecutor, LifeError, MayPostgresExecutor};

// Query builder - Epic 02 Story 03
pub mod query;
pub use query::{
    SelectQuery, SelectModel, from_row_unsigned_try_from_failed, FromRow, LifeEntityName, LifeModelTrait,
    ColumnTrait, ColumnDefinition,
    PrimaryKeyTrait, PrimaryKeyToColumn, PrimaryKeyArity, PrimaryKeyArityTrait,
    ModelManager, StoredProcedure,
    TableDefinition, IndexDefinition,
};

// query_old.rs has been removed - all code migrated to query/ modules

// ActiveModel operations - Epic 02 Story 07
pub mod active_model;
pub use active_model::{ActiveModelTrait, ActiveModelBehavior, ActiveModelError, ActiveValue, with_converted_params};

// Model trait - Core Traits & Types
pub mod model;
pub use model::{ModelError, ModelTrait, TryIntoModel};

// Relation trait - Epic 02 Story 08
pub mod relation;
pub use relation::{
    RelationTrait, RelationBuilder, RelationMetadata, Related, FindRelated,
    Identity, BorrowedIdentityIter, IntoIdentity,
    RelationDef, RelationType, join_tbl_on_condition, build_where_condition,
    join_condition,
};

// Partial Model trait - Epic 02 Story 09
pub mod partial_model;
pub use partial_model::{PartialModelTrait, PartialModelBuilder, SelectPartialQuery};

// JSON helpers - Custom deserializers for floating-point types
pub mod json_helpers;
pub use json_helpers::{deserialize_f32, deserialize_f64, deserialize_option_f32, deserialize_option_f64};

// Re-export raw SQL helpers for convenience
pub use raw_sql::{
    execute_unprepared, execute_statement, find_by_statement, find_all_by_statement, query_value,
};

// Value type system - Epic 02 Story 10 (Phase 4: Value Type Infrastructure)
pub mod value;
pub use value::{
    ValueType, TryGetable, TryGetableMany, ValueExtractionError,
    IntoValueTuple, FromValueTuple, TryFromU64,
};

// Re-export transaction types for convenience
pub use transaction::{Transaction, TransactionError, IsolationLevel};

// Migration system - Epic 03
pub mod migration;
pub use migration::{
    Migration, SchemaManager, MigrationError, MigrationRecord, MigrationStatus,
    Migrator, MigrationLockGuard, startup_migrations, startup_migrations_with_timeout,
    acquire_migration_lock, release_migration_lock, is_migration_lock_held,
};

// Cache Coherence Architecture - Epic 07 Phase 4
pub mod cache;
pub use cache::{CacheProvider, CacheError, DefaultCacheProvider, CachedResult};
