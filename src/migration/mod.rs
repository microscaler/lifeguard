//! Migration system for Lifeguard
//!
//! This module provides the infrastructure for database migrations, including:
//! - Migration trait definition
//! - SchemaManager for schema operations
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
//!     fn down(&self, manager: &SchemaManager) -> Result<(), lifeguard::LifeError> {
//!         let table = Table::drop().table("users").to_owned();
//!         manager.drop_table(table)
//!     }
//! }
//! ```

pub mod error;
pub mod migration;
pub mod schema_manager;
pub mod record;
pub mod checksum;
pub mod state_table;
pub mod lock;
pub mod file;
pub mod status;
pub mod migrator;
pub mod startup;

pub use error::MigrationError;
pub use migration::Migration;
pub use schema_manager::SchemaManager;
pub use record::MigrationRecord;
pub use checksum::{calculate_checksum, validate_checksum};
pub use state_table::{create_state_table, create_state_table_index, initialize_state_table};
pub use lock::{MigrationLock, LockGuard};
pub use file::{MigrationFile, discover_migrations};
pub use status::{MigrationStatus, PendingMigration};
pub use migrator::Migrator;
pub use startup::{startup_migrations, startup_migrations_with_timeout};

// Re-export for convenience
pub use crate::LifeError;
