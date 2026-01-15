//! ActiveModel trait for mutable model operations - Epic 02 Story 07
//!
//! This module provides the `ActiveModelTrait` which allows mutable operations
//! on models for inserts, updates, and deletes. Similar to SeaORM's `ActiveModelTrait`,
//! but adapted for Lifeguard's `LifeRecord` architecture.

use crate::executor::LifeExecutor;
use crate::query::LifeModelTrait;
use crate::model::ModelTrait;
use sea_query::Value;

/// Error type for ActiveModel operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveModelError {
    /// Invalid value type for the column
    InvalidValueType {
        column: String,
        expected: String,
        actual: String,
    },
    /// Column not found
    ColumnNotFound(String),
    /// Primary key required but not set
    PrimaryKeyRequired,
    /// Database operation failed
    DatabaseError(String),
    /// Other error
    Other(String),
}

impl std::fmt::Display for ActiveModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActiveModelError::InvalidValueType {
                column,
                expected,
                actual,
            } => write!(
                f,
                "Invalid value type for column {}: expected {}, got {}",
                column, expected, actual
            ),
            ActiveModelError::ColumnNotFound(column) => {
                write!(f, "Column not found: {}", column)
            }
            ActiveModelError::PrimaryKeyRequired => {
                write!(f, "Primary key is required for this operation")
            }
            ActiveModelError::DatabaseError(msg) => {
                write!(f, "Database error: {}", msg)
            }
            ActiveModelError::Other(msg) => write!(f, "ActiveModel error: {}", msg),
        }
    }
}

impl std::error::Error for ActiveModelError {}

/// Trait for ActiveModel operations
///
/// This trait provides methods for mutable model operations including field access,
/// CRUD operations, and field management. It's similar to SeaORM's `ActiveModelTrait`
/// but adapted for Lifeguard's `LifeRecord` architecture.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{ActiveModelTrait, LifeModelTrait};
///
/// // In a real application, the macro would generate this:
/// // impl ActiveModelTrait for UserRecord {
/// //     type Entity = User;
/// //     type Model = UserModel;
/// //     
/// //     fn get(&self, column: User::Column) -> Option<Value> { ... }
/// //     fn set(&mut self, column: User::Column, value: Value) -> Result<(), ActiveModelError> { ... }
/// //     fn take(&mut self, column: User::Column) -> Option<Value> { ... }
/// //     fn reset(&mut self) { ... }
/// // }
/// ```
pub trait ActiveModelTrait: Clone + Send + std::fmt::Debug {
    /// The Entity type that this ActiveModel belongs to
    type Entity: LifeModelTrait;
    
    /// The Model type that this ActiveModel can convert to
    type Model: ModelTrait<Entity = Self::Entity>;

    /// Get the value of a column from the active model
    ///
    /// Returns `Some(Value)` if the field is set, `None` if it's not set (for Option fields).
    ///
    /// # Arguments
    ///
    /// * `column` - The column to get the value for
    ///
    /// # Returns
    ///
    /// The column value as `Option<sea_query::Value>`, or `None` if the field is not set
    fn get(&self, column: <Self::Entity as LifeModelTrait>::Column) -> Option<Value>;

    /// Set the value of a column in the active model
    ///
    /// # Arguments
    ///
    /// * `column` - The column to set the value for
    /// * `value` - The value to set
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the value cannot be set
    fn set(
        &mut self,
        column: <Self::Entity as LifeModelTrait>::Column,
        value: Value,
    ) -> Result<(), ActiveModelError>;

    /// Take (move) the value of a column from the active model
    ///
    /// This removes the value from the active model and returns it.
    /// After calling `take()`, the field will be `None` (for Option fields).
    ///
    /// # Arguments
    ///
    /// * `column` - The column to take the value from
    ///
    /// # Returns
    ///
    /// The column value as `Option<sea_query::Value>`, or `None` if the field was not set
    fn take(&mut self, column: <Self::Entity as LifeModelTrait>::Column) -> Option<Value>;

    /// Reset all fields to their default state (None for Option fields)
    ///
    /// This clears all field values, setting them back to their uninitialized state.
    fn reset(&mut self);

    /// Insert the active model into the database
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor to use for the operation
    ///
    /// # Returns
    ///
    /// Returns the inserted model on success, or an error if the operation fails
    ///
    /// # Note
    ///
    /// This is a placeholder for future implementation. The actual implementation
    /// will need to generate INSERT SQL and execute it via the executor.
    fn insert<E: LifeExecutor>(&self, _executor: &E) -> Result<Self::Model, ActiveModelError> {
        Err(ActiveModelError::Other("insert() not yet implemented".to_string()))
    }

    /// Update the active model in the database
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor to use for the operation
    ///
    /// # Returns
    ///
    /// Returns the updated model on success, or an error if the operation fails
    ///
    /// # Note
    ///
    /// This requires a primary key to be set. Only dirty (changed) fields will be updated.
    ///
    /// # Note
    ///
    /// This is a placeholder for future implementation. The actual implementation
    /// will need to generate UPDATE SQL and execute it via the executor.
    fn update<E: LifeExecutor>(&self, _executor: &E) -> Result<Self::Model, ActiveModelError> {
        Err(ActiveModelError::Other("update() not yet implemented".to_string()))
    }

    /// Save the active model (insert or update based on primary key)
    ///
    /// If the primary key is set and exists in the database, performs an update.
    /// Otherwise, performs an insert.
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor to use for the operation
    ///
    /// # Returns
    ///
    /// Returns the saved model on success, or an error if the operation fails
    ///
    /// # Note
    ///
    /// This is a placeholder for future implementation. The actual implementation
    /// will need to check if the record exists and either insert or update accordingly.
    fn save<E: LifeExecutor>(&self, _executor: &E) -> Result<Self::Model, ActiveModelError> {
        Err(ActiveModelError::Other("save() not yet implemented".to_string()))
    }

    /// Delete the active model from the database
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor to use for the operation
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if the operation fails
    ///
    /// # Note
    ///
    /// This requires a primary key to be set.
    ///
    /// # Note
    ///
    /// This is a placeholder for future implementation. The actual implementation
    /// will need to generate DELETE SQL and execute it via the executor.
    fn delete<E: LifeExecutor>(&self, _executor: &E) -> Result<(), ActiveModelError> {
        Err(ActiveModelError::Other("delete() not yet implemented".to_string()))
    }
}
