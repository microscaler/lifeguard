//! Procedural macros for Lifeguard ORM
//!
//! This crate provides derive macros for `LifeModel` and `LifeRecord`.

mod attributes;
mod macros;
mod type_conversion;
mod utils;

use proc_macro::TokenStream;

/// Derive macro for `Entity` - generates Entity unit struct, EntityName, Iden, IdenStatic
///
/// This macro generates the Entity unit struct and implements LifeEntityName, Iden, and IdenStatic.
/// Following SeaORM's architecture, this is a separate derive from Model.
///
/// Note: This macro is typically used internally by `LifeModel`. See `LifeModel` for usage examples.
#[proc_macro_derive(DeriveEntity, attributes(table_name, model, column))]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    macros::derive_entity(input)
}

/// Derive macro for `FromRow` - generates FromRow trait implementation
///
/// This macro generates the `FromRow` implementation for converting
/// `may_postgres::Row` into a Model struct. It's separate from `LifeModel`
/// to avoid trait bound resolution issues during macro expansion.
///
/// Note: `LifeModel` automatically generates `FromRow` for the Model struct,
/// so this derive is typically not needed unless you're using a custom Model.
#[proc_macro_derive(FromRow, attributes(column_name))]
pub fn derive_from_row(input: TokenStream) -> TokenStream {
    macros::derive_from_row(input).into()
}

/// Derive macro for `LifeModel` - generates immutable database row representation
///
/// This macro generates:
/// - `Entity` unit struct (with nested `DeriveEntity` for LifeModelTrait)
/// - `Model` struct (immutable row representation)
/// - `Column` enum (all columns)
/// - `PrimaryKey` enum (primary key columns)
/// - `FromRow` implementation (automatic)
/// - `LifeModelTrait` implementation (via nested DeriveEntity)
///
/// See `lifeguard-derive/tests/test_minimal.rs` for usage examples.
#[proc_macro_derive(LifeModel, attributes(table_name, primary_key, column_name, column_type, default_value, default_expr, renamed_from, unique, indexed, nullable, auto_increment, enum_name))]
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
#[proc_macro_derive(LifeRecord, attributes(table_name, primary_key, column_name, column_type, default_value, default_expr, renamed_from, unique, indexed, nullable, auto_increment, enum_name))]
pub fn derive_life_record(input: TokenStream) -> TokenStream {
    macros::derive_life_record(input)
}

/// Derive macro for `DeriveRelation` - generates Related trait implementations
///
/// This macro generates:
/// - Related trait implementations for each relationship variant in the Relation enum
/// - Query builders using SelectQuery for each relationship
///
/// # Example
///
/// ```ignore
/// use lifeguard_derive::DeriveRelation;
///
/// // In your entity module, define the Relation enum:
/// #[derive(DeriveRelation)]
/// pub enum Relation {
///     #[lifeguard(has_many = "super::posts::Entity")]
///     Posts,
///     #[lifeguard(belongs_to = "super::users::Entity")]
///     User,
/// }
/// 
/// // The macro generates Related trait implementations:
/// // impl Related<super::posts::Entity> for Entity { ... }
/// // impl Related<super::users::Entity> for Entity { ... }
/// ```
#[proc_macro_derive(DeriveRelation, attributes(lifeguard))]
pub fn derive_relation(input: TokenStream) -> TokenStream {
    macros::derive_relation(input)
}

/// Derive macro for `DeriveLinked` - generates Linked trait implementations
///
/// This macro generates `Linked<I, T>` trait implementations from enum variants
/// with `#[lifeguard(linked = "...")]` attributes, reducing boilerplate for
/// multi-hop relationship queries.
///
/// # Example
///
/// ```ignore
/// use lifeguard_derive::DeriveLinked;
///
/// #[derive(DeriveLinked)]
/// pub enum LinkedRelation {
///     #[lifeguard(linked = "PostEntity -> CommentEntity")]
///     Comments,
/// }
/// ```
///
/// This generates:
/// ```ignore
/// impl Linked<PostEntity, CommentEntity> for Entity {
///     fn via() -> Vec<RelationDef> {
///         vec![
///             <Entity as Related<PostEntity>>::to(),
///             <PostEntity as Related<CommentEntity>>::to(),
///         ]
///     }
/// }
/// ```
#[proc_macro_derive(DeriveLinked, attributes(lifeguard))]
pub fn derive_linked(input: TokenStream) -> TokenStream {
    macros::derive_linked(input)
}

/// Derive macro for `DerivePartialModel` - generates PartialModelTrait and FromRow implementations
///
/// This macro generates:
/// - PartialModelTrait implementation with selected_columns() method
/// - FromRow implementation for converting database rows to partial models
///
/// # Example
///
/// ```ignore
/// use lifeguard_derive::DerivePartialModel;
///
/// // Define a partial model that only includes id and name
/// #[derive(DerivePartialModel)]
/// #[lifeguard(entity = "super::users::Entity")]
/// pub struct UserPartial {
///     pub id: i32,
///     pub name: String,
/// }
///
/// // The macro generates:
/// // - impl PartialModelTrait for UserPartial { ... }
/// // - impl FromRow for UserPartial { ... }
/// ```
#[proc_macro_derive(DerivePartialModel, attributes(lifeguard, column_name))]
pub fn derive_partial_model(input: TokenStream) -> TokenStream {
    macros::derive_partial_model(input)
}

/// Derive macro for `DeriveTryIntoModel` - generates TryIntoModel trait implementations
///
/// This macro generates `TryIntoModel` implementations for converting custom types (DTOs, partial models, etc.)
/// into Model instances with proper error handling.
///
/// # Example
///
/// ```ignore
/// use lifeguard_derive::DeriveTryIntoModel;
///
/// #[derive(DeriveTryIntoModel)]
/// #[lifeguard(model = "UserModel")]
/// struct CreateUserRequest {
///     name: String,
///     email: String,
///     // Missing fields (id, etc.) will use Default::default()
/// }
///
/// // The macro generates:
/// // impl TryIntoModel<UserModel> for CreateUserRequest {
/// //     type Error = LifeError;
/// //     fn try_into_model(self) -> Result<UserModel, LifeError> {
/// //         Ok(UserModel {
/// //             id: Default::default(),
/// //             name: self.name,
/// //             email: self.email,
/// //         })
/// //     }
/// // }
/// ```
#[proc_macro_derive(DeriveTryIntoModel, attributes(lifeguard))]
pub fn derive_try_into_model(input: TokenStream) -> TokenStream {
    macros::derive_try_into_model(input)
}
