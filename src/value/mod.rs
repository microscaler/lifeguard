//! Value type system for Lifeguard
//!
//! This module provides traits for type-safe value conversions between Rust types
//! and `sea_query::Value`. These traits enable better type safety, error handling,
//! and developer experience when working with database values.
//!
//! ## Traits
//!
//! - **`ValueType`** - Maps Rust types to their corresponding `sea_query::Value` variant
//! - **`TryGetable`** - Safe value extraction with error handling
//! - **`TryGetableMany`** - Extract multiple values from collections
//! - **`IntoValueTuple`** - Convert composite keys to `ValueTuple`
//! - **`FromValueTuple`** - Convert `ValueTuple` to composite keys
//! - **`TryFromU64`** - Safe conversion from `u64` for primary keys

pub mod types;
pub mod try_getable;
pub mod tuple;
pub mod u64;

#[cfg(test)]
mod integration_tests;

pub use types::ValueType;
pub use try_getable::{TryGetable, TryGetableMany, ValueExtractionError};
pub use tuple::{IntoValueTuple, FromValueTuple};
pub use u64::TryFromU64;
