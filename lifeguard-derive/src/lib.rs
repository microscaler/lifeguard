//! Procedural macros for Lifeguard ORM
//!
//! This crate provides derive macros for `LifeModel` and `LifeRecord`.

mod attributes;
mod macros;
mod utils;

use proc_macro::TokenStream;

/// Derive macro for `Entity` - generates Entity unit struct, EntityName, Iden, IdenStatic
///
/// This macro generates the Entity unit struct and implements LifeEntityName, Iden, and IdenStatic.
/// Following SeaORM's architecture, this is a separate derive from Model.
///
/// # Example
/// ```ignore
/// use lifeguard_derive::DeriveEntity;
///
/// #[derive(DeriveEntity)]
/// #[table_name = "users"]
/// pub struct Entity;
/// ```
#[proc_macro_derive(DeriveEntity, attributes(table_name))]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    macros::derive_entity(input)
}

/// Derive macro for `Model` - generates Model struct
///
/// This macro generates the Model struct (immutable row representation).
/// Note: FromRow is a separate derive macro.
///
/// # Example
/// ```ignore
/// use lifeguard_derive::{DeriveModel, FromRow};
///
/// #[derive(DeriveModel, FromRow)]
/// pub struct Model {
///     pub id: i32,
///     pub name: String,
/// }
/// ```
#[proc_macro_derive(DeriveModel, attributes(column_name))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    macros::derive_model(input)
}

/// Derive macro for `Column` - generates Column enum
///
/// This macro generates the Column enum with Iden implementation for use in sea_query.
///
/// # Example
/// ```ignore
/// use lifeguard_derive::DeriveColumn;
///
/// #[derive(DeriveColumn)]
/// pub enum Column {
///     Id,
///     Name,
/// }
/// ```
#[proc_macro_derive(DeriveColumn, attributes(column_name))]
pub fn derive_column(input: TokenStream) -> TokenStream {
    macros::derive_column(input)
}

/// Derive macro for `PrimaryKey` - generates PrimaryKey enum
///
/// This macro generates the PrimaryKey enum for primary key columns.
///
/// # Example
/// ```ignore
/// use lifeguard_derive::DerivePrimaryKey;
///
/// #[derive(DerivePrimaryKey)]
/// pub enum PrimaryKey {
///     Id,
/// }
/// ```
#[proc_macro_derive(DerivePrimaryKey, attributes(primary_key))]
pub fn derive_primary_key(input: TokenStream) -> TokenStream {
    macros::derive_primary_key(input)
}

/// Derive macro for `FromRow` - generates FromRow trait implementation
///
/// This macro generates the `FromRow` implementation for converting
/// `may_postgres::Row` into a Model struct. It's separate from `DeriveModel`
/// to avoid trait bound resolution issues during macro expansion.
///
/// # Example
/// ```ignore
/// use lifeguard_derive::FromRow;
///
/// #[derive(FromRow)]
/// pub struct UserModel {
///     pub id: i32,
///     pub name: String,
/// }
/// ```
#[proc_macro_derive(FromRow, attributes(column_name))]
pub fn derive_from_row(input: TokenStream) -> TokenStream {
    macros::derive_from_row(input).into()
}

/// Derive macro for `LifeModel` - generates immutable database row representation
///
/// This macro generates:
/// - `Model` struct (immutable row representation)
/// - `Column` enum (all columns)
/// - `PrimaryKey` enum (primary key columns)
/// - `Entity` type (entity itself)
/// - `LifeModelTrait` implementation
/// - Field getters (immutable access)
/// - Table name and column metadata
/// - Primary key identification
///
/// **Note:** You must also derive `FromRow` on the Model struct separately.
///
/// # Example
/// ```ignore
/// use lifeguard_derive::{LifeModel, FromRow};
///
/// #[derive(LifeModel)]
/// #[table_name = "users"]
/// pub struct User {
///     #[primary_key]
///     pub id: i32,
///     pub name: String,
/// }
///
/// // Apply FromRow to the generated Model
/// #[derive(FromRow)]
/// pub struct UserModel {
///     pub id: i32,
///     pub name: String,
/// }
/// ```
/// Derive macro for `LifeModelTrait` - generates LifeModelTrait implementation
///
/// This macro generates the `LifeModelTrait` implementation for Entity.
/// It's separate from `LifeModel` to avoid trait bound resolution issues during macro expansion.
///
/// # Example
/// ```ignore
/// use lifeguard_derive::DeriveLifeModelTrait;
///
/// #[derive(DeriveLifeModelTrait)]
/// pub struct Entity;
/// ```
#[proc_macro_derive(DeriveLifeModelTrait, attributes(model))]
pub fn derive_life_model_trait(input: TokenStream) -> TokenStream {
    macros::derive_life_model_trait(input)
}

/// Derive macro for `LifeModel` - generates immutable database row representation
///
/// This macro generates:
/// - `Model` struct (immutable row representation)
/// - `Column` enum (all columns)
/// - `PrimaryKey` enum (primary key columns)
/// - `Entity` type (entity itself)
/// - Field getters (immutable access)
/// - Table name and column metadata
/// - Primary key identification
///
/// **Note:** This does NOT generate:
/// - `FromRow` implementation (use separate `FromRow` derive)
/// - `LifeModelTrait` implementation (use separate `DeriveLifeModelTrait` derive)
///
/// # Example
/// ```ignore
/// use lifeguard_derive::{LifeModel, FromRow, DeriveLifeModelTrait};
///
/// #[derive(LifeModel)]
/// #[table_name = "users"]
/// pub struct User {
///     #[primary_key]
///     pub id: i32,
///     pub name: String,
/// }
///
/// // Apply FromRow to the generated Model
/// #[derive(FromRow)]
/// pub struct UserModel {
///     pub id: i32,
///     pub name: String,
/// }
///
/// // Apply LifeModelTrait to Entity
/// #[derive(DeriveLifeModelTrait)]
/// pub struct Entity;
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
