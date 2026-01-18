//! Type-safe column operations for query building.
//!
//! This module provides traits and implementations for type-safe column operations
//! that match SeaORM's API. Columns can be used in filters with compile-time type checking.
//!
//! # Structure
//!
//! - `definition`: Column metadata and type inference
//! - `trait`: ColumnTrait for building filter expressions
//! - `type_mapping`: Type mapping utilities (internal)

mod type_mapping;
pub mod definition;
pub mod column_trait;

// Re-export public types
pub use definition::ColumnDefinition;
pub use column_trait::ColumnTrait;
