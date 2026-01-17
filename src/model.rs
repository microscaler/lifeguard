//! Model trait for accessing and manipulating model data
//!
//! This module provides the `ModelTrait` which allows dynamic access to model fields
//! and primary key values. Similar to SeaORM's `ModelTrait`.

use crate::query::LifeModelTrait;
use crate::relation::identity::Identity;
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
    /// For composite primary keys, this returns the first primary key value (backward compatible).
    ///
    /// # Returns
    ///
    /// The primary key value as a `sea_query::Value`.
    /// 
    /// # Edge Cases
    ///
    /// - **No primary key:** Returns `Value::String(None)` if the entity has no primary key defined.
    ///   This is a design limitation - consider checking for primary key existence before calling.
    /// - **Composite primary keys:** Returns the first primary key value for backward compatibility.
    ///   Use `get_primary_key_values()` to get all primary key values.
    fn get_primary_key_value(&self) -> Value;
    
    /// Get the primary key as Identity (supports composite keys)
    ///
    /// This method returns an `Identity` enum that represents the primary key column(s).
    /// For single-column primary keys, returns `Identity::Unary`.
    /// For composite primary keys, returns `Identity::Binary`, `Identity::Ternary`, or `Identity::Many`.
    ///
    /// # Returns
    ///
    /// An `Identity` enum representing the primary key column(s)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{ModelTrait, relation::identity::Identity};
    ///
    /// // For a single primary key:
    /// let identity = model.get_primary_key_identity();
    /// // Returns: Identity::Unary(Column::Id.into_iden())
    ///
    /// // For a composite primary key (id, tenant_id):
    /// let identity = model.get_primary_key_identity();
    /// // Returns: Identity::Binary(Column::Id.into_iden(), Column::TenantId.into_iden())
    /// ```
    fn get_primary_key_identity(&self) -> Identity;
    
    /// Get primary key values as Vec<Value> (helper for WHERE clauses)
    ///
    /// This method extracts all primary key values from the model as a vector.
    /// It works with both single and composite primary keys.
    ///
    /// # Returns
    ///
    /// A vector of `Value`s corresponding to each primary key column
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::ModelTrait;
    ///
    /// // For a single primary key:
    /// let values = model.get_primary_key_values();
    /// // Returns: vec![Value::Int(Some(1))]
    ///
    /// // For a composite primary key (id, tenant_id):
    /// let values = model.get_primary_key_values();
    /// // Returns: vec![Value::Int(Some(1)), Value::Int(Some(10))]
    /// ```
    fn get_primary_key_values(&self) -> Vec<Value> {
        // Default implementation: extract values from Identity
        // The macro will override this for efficiency
        let identity = self.get_primary_key_identity();
        extract_values_from_identity(self, &identity)
    }
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

/// Extract values from model based on Identity columns
///
/// This helper function extracts the actual `Value`s from a model based on
/// the columns specified in the `Identity`. 
///
/// # Arguments
///
/// * `model` - The model instance
/// * `_identity` - The identity (which columns to extract)
///
/// # Returns
///
/// A vector of `Value`s corresponding to the identity columns
///
/// # Note
///
/// This is a fallback implementation. The macro will generate more efficient
/// implementations that directly access model fields. The macro-generated
/// `get_primary_key_values()` will override the default implementation.
fn extract_values_from_identity<M>(model: &M, _identity: &Identity) -> Vec<Value>
where
    M: ModelTrait,
{
    // This is a placeholder implementation that will be replaced by macro-generated code.
    // The macro will generate code that directly accesses model fields based on the
    // primary key columns, avoiding the need to convert Identity back to Column enum.
    //
    // For now, return a single value (backward compatible with single keys).
    // The macro will override get_primary_key_values() to return all values for composite keys.
    vec![model.get_primary_key_value()]
}
