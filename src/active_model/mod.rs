//! `ActiveModel` operations for mutable model operations.
//!
//! This module provides traits and utilities for mutable model operations including
//! inserts, updates, and deletes. Similar to `SeaORM`'s `ActiveModelTrait`, but adapted
//! for Lifeguard's `LifeRecord` architecture.
//!
//! # Architecture
//!
//! The `active_model` module follows `Sea-ORM`'s organizational patterns:
//! - **Traits**: Core `ActiveModel` traits (`ActiveModelTrait`, `ActiveModelBehavior`)
//! - **Value**: `ActiveValue` enum for field value metadata
//! - **Error**: `ActiveModelError` for operation errors
//! - **Conversion**: `SeaQuery` → `ToSql` via [`crate::active_model::with_converted_params`] (`ActiveModelError`)
//!
//! # Examples
//!
//! ```no_run
//! use lifeguard::{ActiveModelTrait, LifeExecutor};
//!
//! # struct UserRecord;
//! # impl ActiveModelTrait for UserRecord {
//! #     type Entity = ();
//! #     type Model = ();
//! #     fn get(&self, _: <() as LifeModelTrait>::Column) -> Option<sea_query::Value> { None }
//! #     fn set(&mut self, _: <() as LifeModelTrait>::Column, _: sea_query::Value) -> Result<(), ActiveModelError> { Ok(()) }
//! #     fn take(&mut self, _: <() as LifeModelTrait>::Column) -> Option<sea_query::Value> { None }
//! #     fn reset(&mut self) {}
//! #     // ... other methods
//! # }
//! # let executor: &dyn LifeExecutor = todo!();
//!
//! // Create and insert a record
//! let mut record = UserRecord::default();
//! record.set(User::Column::Name, sea_query::Value::String(Some("John".to_string())))?;
//! let model = record.insert(executor)?;
//! ```

// Validation types (PRD Phase B; no dependency on traits)
pub mod validate_op;
#[doc(inline)]
pub use validate_op::{ValidateOp, ValidationError, ValidationStrategy};

// Core traits
pub mod traits;
#[doc(inline)]
pub use traits::{ActiveModelBehavior, ActiveModelTrait};

// Value wrapper
pub mod value;
#[doc(inline)]
pub use value::ActiveValue;

// Error types
pub mod error;
#[doc(inline)]
pub use error::ActiveModelError;

// Validation orchestration (`run_validators` after lifecycle hooks)
pub mod validation;
#[doc(inline)]
pub use validation::{run_validators, run_validators_with_strategy};

/// Built-in `len` / `range`-style validators on [`sea_query::Value`] (PRD Phase B follow-on).
pub mod predicates;

// Graph sorting and nesting mechanics
pub mod graph;
#[doc(inline)]
pub use graph::{GraphEdge, GraphState};

// Value conversion utilities
pub(crate) mod conversion;
pub use conversion::with_converted_params;
