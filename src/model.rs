//! Model trait for accessing and manipulating model data
//!
//! This module provides the `ModelTrait` which allows dynamic access to model fields
//! and primary key values. Similar to SeaORM's `ModelTrait`.

use crate::query::LifeModelTrait;
use sea_query::Value;

/// Trait for Model-level operations
///
/// This trait provides methods for accessing and manipulating model data at runtime.
/// It's similar to SeaORM's `ModelTrait` and allows dynamic column access.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{ModelTrait, LifeModelTrait};
///
/// # struct Entity; // Entity
/// # impl lifeguard::LifeModelTrait for Entity {
/// #     type Model = EntityModel;
/// #     type Column = EntityColumn;
/// # }
/// # struct EntityModel { id: i32, name: String };
/// # impl lifeguard::ModelTrait for EntityModel {
/// #     type Entity = Entity;
/// #     fn get(&self, _col: Entity::Column) -> sea_query::Value { todo!() }
/// #     fn get_primary_key_value(&self) -> sea_query::Value { todo!() }
/// # }
/// let model = EntityModel { id: 1, name: "John".to_string() };
/// let id_value = model.get(Entity::Column::Id);
/// let pk_value = model.get_primary_key_value();
/// ```
///
/// # Edge Cases & Limitations
///
/// ## Missing Primary Key
/// If an entity has no primary key defined, `get_primary_key_value()` returns `Value::String(None)`.
/// This is a design limitation - consider checking for primary key existence before calling this method.
///
/// ## Composite Primary Keys
/// Currently, only single-column primary keys are fully supported. For entities with composite primary keys,
/// `get_primary_key_value()` returns only the first primary key value. Full composite key support is a future enhancement.
///
/// ## Supported Types
/// The following types are fully supported for `get()` and `set()` operations:
/// - Primitives: `i32`, `i64`, `i16`, `u8`, `u16`, `u32`, `u64`, `f32`, `f64`, `bool`, `String`
/// - Option types: `Option<T>` where `T` is any supported type
/// - JSON: `serde_json::Value` and `Option<serde_json::Value>`
///
/// Unknown types will fall back to `Value::String(None)` in `get()` operations, which may hide bugs.
/// Consider using only supported types or extending the macro to support additional types.
pub trait ModelTrait: Clone + Send + std::fmt::Debug {
    /// The Entity type that this Model belongs to
    type Entity: LifeModelTrait;

    /// Get the value of a column from the model
    ///
    /// # Arguments
    ///
    /// * `column` - The column to get the value for
    ///
    /// # Returns
    ///
    /// The column value as a `sea_query::Value`
    fn get(&self, column: <Self::Entity as LifeModelTrait>::Column) -> Value;

    /// Set the value of a column in the model
    ///
    /// # Arguments
    ///
    /// * `column` - The column to set the value for
    /// * `value` - The value to set
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the value cannot be set
    ///
    /// # Note
    ///
    /// This method modifies the model in-place. For immutable models, consider
    /// using `ActiveModel` or `LifeRecord` instead.
    fn set(
        &mut self,
        column: <Self::Entity as LifeModelTrait>::Column,
        value: Value,
    ) -> Result<(), ModelError>;

    /// Get the primary key value(s) from the model
    ///
    /// For single-column primary keys, returns the value directly.
    /// For composite primary keys, this would return a tuple (future enhancement).
    ///
    /// # Returns
    ///
    /// The primary key value as a `sea_query::Value`.
    /// 
    /// # Edge Cases
    ///
    /// - **No primary key:** Returns `Value::String(None)` if the entity has no primary key defined.
    ///   This is a design limitation - consider checking for primary key existence before calling.
    /// - **Composite primary keys:** Currently only returns the first primary key value.
    ///   Full composite key support is a future enhancement.
    fn get_primary_key_value(&self) -> Value;
}

/// Error type for ModelTrait operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// Invalid value type for the column
    InvalidValueType {
        column: String,
        expected: String,
        actual: String,
    },
    /// Column not found
    ColumnNotFound(String),
    /// Other error
    Other(String),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::InvalidValueType {
                column,
                expected,
                actual,
            } => write!(
                f,
                "Invalid value type for column {}: expected {}, got {}",
                column, expected, actual
            ),
            ModelError::ColumnNotFound(column) => {
                write!(f, "Column not found: {}", column)
            }
            ModelError::Other(msg) => write!(f, "Model error: {}", msg),
        }
    }
}

impl std::error::Error for ModelError {}
