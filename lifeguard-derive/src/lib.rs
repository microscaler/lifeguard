//! Procedural macros for Lifeguard ORM
//!
//! This crate provides derive macros for `LifeModel` and `LifeRecord`.

mod attributes;
mod macros;
mod utils;

use proc_macro::TokenStream;

/// Derive macro for `LifeModel` - generates immutable database row representation
///
/// This macro generates:
/// - `Model` struct (immutable row representation)
/// - `Column` enum (all columns)
/// - `PrimaryKey` enum (primary key columns)
/// - `Entity` type (entity itself)
/// - `FromRow` implementation for deserializing database rows
/// - Field getters (immutable access)
/// - Table name and column metadata
/// - Primary key identification
///
/// # Example
/// ```ignore
/// use lifeguard_derive::LifeModel;
///
/// #[derive(LifeModel)]
/// #[table_name = "users"]
/// pub struct User {
///     #[primary_key]
///     pub id: i32,
///     pub name: String,
///     pub email: String,
/// }
/// ```
#[proc_macro_derive(LifeModel, attributes(table_name, primary_key, column_name, column_type, default_value, unique, indexed, nullable, auto_increment, enum_name))]
pub fn derive_life_model(input: TokenStream) -> TokenStream {
    macros::derive_life_model(input)
}
