//! Partial Model operations for selecting subset of columns.
//!
//! This module provides support for partial model queries where only a subset of columns
//! are selected from the database. This is useful for performance optimization when you
//! only need specific fields.
//!
//! # Architecture
//!
//! The `partial_model` module follows `Sea-ORM`'s organizational patterns:
//! - **Traits**: Core partial model traits (`PartialModelTrait`, `PartialModelBuilder`)
//! - **Query**: Partial query builder (`SelectPartialQuery`)

// Core traits
pub mod traits;
#[doc(inline)]
pub use traits::{PartialModelTrait, PartialModelBuilder};

// Query builder
pub mod query;
#[doc(inline)]
pub use query::SelectPartialQuery;
