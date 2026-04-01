//! Query building and execution for `LifeModel` entities.
//!
//! This module provides the query builder API for building and executing SQL queries
//! against database entities. It includes traits for entity definitions, query builders
//! for SELECT/INSERT/UPDATE/DELETE operations, and execution methods.
//!
//! # Default path vs advanced SQL
//!
//! The usual flow is [`LifeModelTrait::find`] → filters / order / limit → [`SelectQuery::all`] /
//! [`SelectQuery::one`] (see [`crate::query::execution`]). CTEs, joins to subqueries, window clauses,
//! and similar features are **opt-in** on [`SelectQuery`]; see [crate::query::select] for the full
//! list and `sea_query` types to import.
//!
//! # Architecture
//!
//! The query module follows `Sea-ORM`'s organizational patterns:
//! - **Traits**: Core entity and model traits (`LifeModelTrait`, `LifeEntityName`)
//! - **Select**: SELECT query builder (`SelectQuery`)
//! - **Scopes**: Named composable predicates (`scope` module, `SelectQuery::scope`, `IntoScope`)
//! - **SQL extras on `SelectQuery`**: [`SelectQuery::with_cte`](select::SelectQuery::with_cte) (CTE + lifeguard `all`/`one`), [`join_subquery`](select::SelectQuery::join_subquery), typed [`window`](select::SelectQuery::window) / [`expr_window_as`](select::SelectQuery::expr_window_as) (see also [`subquery_column`](select::SelectQuery::subquery_column), [`window_function_cust`](select::SelectQuery::window_function_cust))
//! - **Execution**: Query execution methods (`all`, `one`, `first`)
//! - **Value Conversion**: `SeaQuery` `Value` to `ToSql` (`converted_params` + `value_conversion`)
//! - **Error Handling**: Error detection and classification utilities
//! - **Column**: Type-safe column operations (including **F-style** `f_add` / `f_sub` / `f_mul` / `f_div` on [`ColumnTrait`] for `UPDATE SET`)
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
pub(crate) mod converted_params;
pub(crate) mod value_conversion;

// Cursor keyset abstraction
pub mod cursor;
pub use cursor::CursorPaginator;

// Server-Side coroutine extensions
pub mod stream;
pub use stream::SelectQueryStreamEx;

// SELECT query builder
pub mod select;
#[doc(inline)]
pub use select::{SelectModel, SelectQuery};

// Named scopes (composable predicates; see `scope` module)
pub mod scope;
#[doc(inline)]
pub use scope::IntoScope;

// Dataloader N+1 resolution
pub mod loader;

// Aggregation endpoints
pub mod aggregate;
#[doc(inline)]
pub use aggregate::{AggregateQuery, LifeAggregate};

// Query execution methods
pub mod execution;
#[doc(inline)]
pub use execution::{Paginator, PaginatorWithCount};

// Column operations
pub mod column;
#[doc(inline)]
pub use column::{ColumnDefinition, ColumnTrait};

// Table operations (for entity-driven migrations)
pub mod table;
#[doc(inline)]
pub use table::{
    format_index_key_list_derive_value, format_index_key_list_sql,
    index_definition_to_derive_index_value, index_key_parts_coverage_columns, IndexBtreeNulls,
    IndexBtreeSort, IndexDefinition, IndexKeyPart, TableDefinition,
};

// Primary key operations
pub mod primary_key;
#[doc(inline)]
pub use primary_key::{PrimaryKeyArity, PrimaryKeyArityTrait, PrimaryKeyToColumn, PrimaryKeyTrait};

// Model Manager pattern for custom query methods
pub mod manager;
#[doc(inline)]
pub use manager::{ModelManager, StoredProcedure};

// FromRow trait is in traits module
pub use traits::{from_row_unsigned_try_from_failed, FromRow};
