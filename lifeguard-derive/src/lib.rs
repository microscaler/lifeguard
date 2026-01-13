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

/// Derive macro for `LifeRecord` - generates mutable change-set objects
///
/// This macro generates:
/// - `Record` struct (mutable change-set with Option<T> fields)
/// - `from_model()` method (create from LifeModel for updates)
/// - `to_model()` method (convert to LifeModel, None fields use defaults)
/// - `dirty_fields()` method (returns list of changed fields)
/// - `is_dirty()` method (checks if any fields changed)
/// - Setter methods for each field
///
/// # Example
/// ```ignore
/// use lifeguard_derive::{LifeModel, LifeRecord};
///
/// #[derive(LifeModel, LifeRecord)]
/// #[table_name = "users"]
/// pub struct User {
///     #[primary_key]
///     pub id: i32,
///     pub name: String,
///     pub email: String,
/// }
///
/// // Create a record for update
/// let mut record = UserRecord::from_model(&user_model);
/// record.set_name("New Name".to_string());
/// // Only changed fields will be updated
/// ```
#[proc_macro_derive(LifeRecord, attributes(table_name, primary_key, column_name, column_type, default_value, unique, indexed, nullable, auto_increment, enum_name))]
pub fn derive_life_record(input: TokenStream) -> TokenStream {
    macros::derive_life_record(input)
}
