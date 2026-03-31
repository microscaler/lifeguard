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
//!   acquire timeout ([`LifeError::PoolAcquireTimeout`]), optional idle `SELECT 1` liveness probes
//!   ([`LifeguardPoolSettings::idle_liveness_interval`]), WAL-aware replica routing ([`WalLagPolicy`],
//!   [`crate::pool::wal::WalLagMonitor`]), and [`LifeguardPool::from_database_config`] for file/env config.
//!   With the **`metrics`** feature, pool-scoped Prometheus series use a low-cardinality
//!   **`pool_tier`** label (`primary` / `replica`); see [`crate::metrics::METRICS`].
//!
//!   See the [connection pooling PRD](https://github.com/microscaler/lifeguard/blob/main/docs/planning/PRD_CONNECTION_POOLING.md),
//!   [operator tuning / non-goals](https://github.com/microscaler/lifeguard/blob/main/docs/POOLING_OPERATIONS.md),
//!   and [TCP keepalive / idle tuning](https://github.com/microscaler/lifeguard/blob/main/docs/POOL_TCP_KEEPALIVE.md)
//!   (PRD and ops links work on **docs.rs** via GitHub URLs; clone has the same paths under `docs/`).
//!
//! ## Explicit opt-in APIs
//!
//! Advanced `SELECT` features (CTEs, subquery joins, windows, raw `WITH` escape hatches) are **not**
//! implied by normal [`SelectQuery`] usage — you chain the methods documented in
//! [`crate::query::select`]. For connection pools, [`ReadPreference`] overrides where **reads** go
//! ([`PooledLifeExecutor::with_read_preference`]); writes stay on the primary tier.

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
#[cfg(feature = "tracing")]
pub use logging::{channel_layer, ChannelLayer};
pub use logging::{
    enqueue, flush_log_channel, global_log_sender, init_log_bridge, try_enqueue, ChannelLogger,
    LogLevel, LogMsg, LogRecord, CHANNEL_LOG_BRIDGE,
};

pub mod pool;

// Test helpers - Epic 01 Story 08
// Available for integration tests
pub mod test_helpers;

// Entity tests will be rebuilt in Epic 03
// mod tests_cfg;

pub use pool::{
    DatabaseConfig, ExclusivePrimaryLifeExecutor, LifeguardPool, LifeguardPoolSettings, OwnedParam,
    PooledLifeExecutor, ReadPreference, WalLagPolicy,
};

#[doc(inline)]
pub use lifeguard_derive::scope;

// Optional GraphQL: `LifeModel` nests `async_graphql::SimpleObject` on the generated `Model`.
// Crates that enable `lifeguard`/`graphql` should depend on the same `async-graphql` version
// and enable the scalar features they use (e.g. `chrono`, `uuid`, `decimal`); the workspace
// crate pins these in `[workspace.dependencies]` for tests and internal packages.
#[cfg(feature = "graphql")]
pub use async_graphql;

// Re-export connection types for convenience
pub use connection::{
    check_connection_health, check_connection_health_with_timeout, connect,
    validate_connection_string, ConnectionError, ConnectionString,
};

// Re-export executor types for convenience
pub use executor::{LifeError, LifeExecutor, MayPostgresExecutor};

// Query builder - Epic 02 Story 03
pub mod query;
pub use query::{
    from_row_unsigned_try_from_failed, ColumnDefinition, ColumnTrait, FromRow, IndexDefinition,
    IntoScope, LifeEntityName, LifeModelTrait, ModelManager, PrimaryKeyArity, PrimaryKeyArityTrait,
    PrimaryKeyToColumn, PrimaryKeyTrait, SelectModel, SelectQuery, StoredProcedure, TableDefinition,
};

// query_old.rs has been removed - all code migrated to query/ modules

// ActiveModel operations - Epic 02 Story 07
pub mod active_model;
pub use active_model::{
    predicates, run_validators, run_validators_with_strategy, with_converted_params,
    ActiveModelBehavior, ActiveModelError, ActiveModelTrait, ActiveValue, ValidateOp,
    ValidationError, ValidationStrategy,
};

// Model trait - Core Traits & Types
pub mod model;
pub use model::{ModelError, ModelTrait, TryIntoModel};

// Session / identity map (PRD Phase E v0)
pub mod session;
pub use session::{
    fingerprint_pk_values, is_pending_insert_key, ModelIdentityMap, PENDING_INSERT_KEY_PREFIX,
    Session, SessionDirtyNotifier, SessionIdentityModelCell,
};

// Relation trait - Epic 02 Story 08
pub mod relation;
pub use relation::{
    build_where_condition, join_condition, join_tbl_on_condition, BorrowedIdentityIter,
    FindRelated, Identity, IntoIdentity, Related, RelationBuilder, RelationDef, RelationMetadata,
    RelationTrait, RelationType,
};

// Partial Model trait - Epic 02 Story 09
pub mod partial_model;
pub use partial_model::{PartialModelBuilder, PartialModelTrait, SelectPartialQuery};

// JSON helpers - Custom deserializers for floating-point types
pub mod json_helpers;
pub use json_helpers::{
    deserialize_f32, deserialize_f64, deserialize_option_f32, deserialize_option_f64,
};

// Re-export raw SQL helpers for convenience
pub use raw_sql::{
    execute_statement, execute_unprepared, find_all_by_statement, find_by_statement, query_value,
};

// Value type system - Epic 02 Story 10 (Phase 4: Value Type Infrastructure)
pub mod value;
pub use value::{
    FromValueTuple, IntoValueTuple, TryFromU64, TryGetable, TryGetableMany, ValueExtractionError,
    ValueType,
};

// Re-export transaction types for convenience
pub use transaction::{IsolationLevel, Transaction, TransactionError};

// Migration system - Epic 03
pub mod migration;
pub use migration::{
    acquire_migration_lock, is_migration_lock_held, release_migration_lock, startup_migrations,
    startup_migrations_with_timeout, Migration, MigrationError, MigrationLockGuard,
    MigrationRecord, MigrationStatus, Migrator, SchemaManager,
};

// Cache Coherence Architecture - Epic 07 Phase 4
pub mod cache;
pub use cache::{CacheError, CacheProvider, CachedResult, DefaultCacheProvider};
