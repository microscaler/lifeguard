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

pub use error::MigrationError;
pub use migration::Migration;
pub use schema_manager::SchemaManager;

// Re-export for convenience
pub use crate::LifeError;
