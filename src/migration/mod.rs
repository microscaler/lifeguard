//! Migration system for Lifeguard
//!
//! This module provides the infrastructure for database migrations, including:
//! - `Migration` trait definition
//! - `SchemaManager` for schema operations
//! - Migration state tracking
//! - Migration execution and validation
//!
//! # Example
//!
//! ```rust,no_run
//! use lifeguard::migration::{Migration, SchemaManager};
//! use sea_query::{Table, ColumnDef};
//!
//! pub struct CreateUsersTable;
//!
//! impl Migration for CreateUsersTable {
//!     fn name(&self) -> &str {
//!         "create_users_table"
//!     }
//!     
//!     fn version(&self) -> i64 {
//!         20240120120000
//!     }
//!     
//!     fn up(&self, manager: &SchemaManager) -> Result<(), lifeguard::LifeError> {
//!         let table = Table::create()
//!             .table("users")
//!             .col(ColumnDef::new("id").integer().not_null().auto_increment().primary_key())
//!             .col(ColumnDef::new("email").string().not_null().unique())
//!             .to_owned();
//!         manager.create_table(table)
//!     }
//!     
//!     fn down(&self, manager: &SchemaManager<'_>) -> Result<(), lifeguard::LifeError> {
//!         let table = Table::drop().table("users").to_owned();
//!         manager.drop_table(table)
//!     }
//! }
//! ```

pub mod checksum;
pub mod error;
pub mod file;
pub mod lock;
#[allow(clippy::module_inception)]
pub mod migration;
pub mod migrator;
pub mod record;
pub mod registry;
pub mod schema_manager;
pub mod startup;
pub mod state_table;
pub mod status;

pub use checksum::{calculate_checksum, validate_checksum};
pub use error::MigrationError;
pub use file::{discover_migrations, MigrationFile};
pub use lock::{
    acquire_migration_lock, is_migration_lock_held, release_migration_lock, MigrationLockGuard,
};
pub use migration::Migration;
pub use migrator::Migrator;
pub use record::MigrationRecord;
pub use registry::{
    clear_registry, execute_migration, get_migration, is_registered, register_migration,
    unregister_migration, MigrationDirection,
};
pub use schema_manager::SchemaManager;
pub use startup::{startup_migrations, startup_migrations_with_timeout};
pub use state_table::{create_state_table, create_state_table_index, initialize_state_table};
pub use status::{MigrationStatus, PendingMigration};

// Re-export for convenience
pub use crate::LifeError;
