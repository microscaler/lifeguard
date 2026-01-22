//! Query building and execution for LifeModel entities.
//!
//! This module provides the query builder API for building and executing SQL queries
//! against database entities. It includes traits for entity definitions, query builders
//! for SELECT/INSERT/UPDATE/DELETE operations, and execution methods.
//!
//! # Architecture
//!
//! The query module follows Sea-ORM's organizational patterns:
//! - **Traits**: Core entity and model traits (`LifeModelTrait`, `LifeEntityName`)
//! - **Select**: SELECT query builder (`SelectQuery`)
//! - **Execution**: Query execution methods (`all`, `one`, `first`)
//! - **Value Conversion**: SeaQuery Value to ToSql parameter conversion
//! - **Error Handling**: Error detection and classification utilities
//! - **Column**: Type-safe column operations
//! - **Primary Key**: Primary key operations and traits
//!
//! # Examples
//!
//! ```no_run
//! use lifeguard::{LifeModelTrait, LifeExecutor};
//! use sea_query::Expr;
//!
//! # struct User;
//! # struct UserModel { id: i32, name: String };
//! # impl lifeguard::FromRow for UserModel {
//! #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
//! # }
//! # impl lifeguard::LifeEntityName for User {
//! #     fn table_name(&self) -> &'static str { "users" }
//! # }
//! # impl Default for User {
//! #     fn default() -> Self { User }
//! # }
//! # impl lifeguard::LifeModelTrait for User {
//! #     type Model = UserModel;
//! #     type Column = ();
//! # }
//! # let executor: &dyn LifeExecutor = todo!();
//!
//! // Find all users
//! let users = User::find().all(executor)?;
//!
//! // Find users with filters
//! let active_users = User::find()
//!     .filter(Expr::col("active").eq(true))
//!     .all(executor)?;
//! ```

// Core traits for entities and models
pub mod traits;
#[doc(inline)]
pub use traits::{LifeEntityName, LifeModelTrait};

// Error handling utilities
pub(crate) mod error_handling;
// Note: is_no_rows_error is pub(crate), so we don't re-export it

// Value conversion utilities
pub(crate) mod value_conversion;

// SELECT query builder
pub mod select;
#[doc(inline)]
pub use select::{SelectQuery, SelectModel};

// Query execution methods
pub mod execution;
#[doc(inline)]
pub use execution::{Paginator, PaginatorWithCount};

// Column operations
pub mod column;
#[doc(inline)]
pub use column::{ColumnTrait, ColumnDefinition};

// Table operations (for entity-driven migrations)
pub mod table;
#[doc(inline)]
pub use table::{TableDefinition, IndexDefinition};

// Primary key operations
pub mod primary_key;
#[doc(inline)]
pub use primary_key::{
    PrimaryKeyTrait, PrimaryKeyToColumn, PrimaryKeyArity, PrimaryKeyArityTrait,
};

// Model Manager pattern for custom query methods
pub mod manager;
#[doc(inline)]
pub use manager::{ModelManager, StoredProcedure};

// FromRow trait is in traits module
pub use traits::FromRow;
