//! ActiveModel trait for mutable model operations - Epic 02 Story 07
//!
//! This module provides the `ActiveModelTrait` which allows mutable operations
//! on models for inserts, updates, and deletes. Similar to SeaORM's `ActiveModelTrait`,
//! but adapted for Lifeguard's `LifeRecord` architecture.

use crate::executor::LifeExecutor;
use crate::query::LifeModelTrait;
use crate::model::ModelTrait;
use sea_query::Value;
use may_postgres::types::ToSql;
use serde_json::Value as JsonValue;

/// Convert SeaQuery values to may_postgres ToSql parameters and execute a closure
///
/// This helper function converts a slice of SeaQuery `Value` enums into
/// `ToSql` trait objects that can be used with `may_postgres`, then executes
/// a closure with the converted parameters.
///
/// The conversion follows the same pattern as `SelectQuery::all()` and `SelectQuery::one()`:
/// 1. First pass: collect all values into typed vectors
/// 2. Second pass: create references to the stored values
/// 3. Execute closure with the parameters (references are valid within closure scope)
///
/// # Arguments
///
/// * `values` - Slice of SeaQuery `Value` enums to convert
/// * `f` - Closure that receives the converted parameters and executes the database operation
///
/// # Returns
///
/// Returns the result of the closure, or an error if conversion fails.
///
/// # Errors
///
/// Returns `ActiveModelError::Other` if an unsupported value type is encountered.
pub fn with_converted_params<F, R>(values: &[Value], f: F) -> Result<R, ActiveModelError>
where
    F: FnOnce(&[&dyn ToSql]) -> Result<R, ActiveModelError>,
{
    // Collect all values first - values are wrapped in Option in this version
    let mut bools: Vec<bool> = Vec::new();
    let mut ints: Vec<i32> = Vec::new();
    let mut big_ints: Vec<i64> = Vec::new();
    let mut strings: Vec<String> = Vec::new();
    let mut bytes: Vec<Vec<u8>> = Vec::new();
    let mut nulls: Vec<Option<i32>> = Vec::new();
    let mut floats: Vec<f32> = Vec::new();
    let mut doubles: Vec<f64> = Vec::new();
    
    // First pass: collect all values into typed vectors
    for value in values.iter() {
        match value {
            Value::Bool(Some(b)) => bools.push(*b),
            Value::Int(Some(i)) => ints.push(*i),
            Value::BigInt(Some(i)) => big_ints.push(*i),
            Value::String(Some(s)) => strings.push(s.clone()),
            Value::Bytes(Some(b)) => bytes.push(b.clone()),
            Value::Bool(None) | Value::Int(None) | 
            Value::BigInt(None) | Value::String(None) | 
            Value::Bytes(None) => nulls.push(None),
            Value::TinyInt(Some(i)) => ints.push(*i as i32),
            Value::SmallInt(Some(i)) => ints.push(*i as i32),
            Value::TinyUnsigned(Some(u)) => ints.push(*u as i32),
            Value::SmallUnsigned(Some(u)) => ints.push(*u as i32),
            Value::Unsigned(Some(u)) => big_ints.push(*u as i64),
            Value::BigUnsigned(Some(u)) => {
                if *u > i64::MAX as u64 {
                    return Err(ActiveModelError::Other(format!(
                        "BigUnsigned value {} exceeds i64::MAX ({}), cannot be safely cast to i64",
                        u, i64::MAX
                    )));
                }
                big_ints.push(*u as i64);
            },
            Value::Float(Some(f)) => floats.push(*f),
            Value::Double(Some(d)) => doubles.push(*d),
            Value::TinyInt(None) | Value::SmallInt(None) |
            Value::TinyUnsigned(None) | Value::SmallUnsigned(None) |
            Value::Unsigned(None) | Value::BigUnsigned(None) |
            Value::Float(None) | Value::Double(None) => nulls.push(None),
            Value::Json(Some(j)) => {
                strings.push(serde_json::to_string(&**j).map_err(|e| {
                    ActiveModelError::Other(format!("Failed to serialize JSON: {}", e))
                })?);
            },
            Value::Json(None) => nulls.push(None),
            _ => {
                return Err(ActiveModelError::Other(format!(
                    "Unsupported value type in query: {:?}",
                    value
                )));
            }
        }
    }
    
    // Second pass: create references to the stored values
    let mut bool_idx = 0;
    let mut int_idx = 0;
    let mut big_int_idx = 0;
    let mut string_idx = 0;
    let mut byte_idx = 0;
    let mut null_idx = 0;
    let mut float_idx = 0;
    let mut double_idx = 0;
    
    let mut params: Vec<&dyn ToSql> = Vec::new();
    
    for value in values.iter() {
        match value {
            Value::Bool(Some(_)) => {
                params.push(&bools[bool_idx] as &dyn ToSql);
                bool_idx += 1;
            }
            Value::Int(Some(_)) => {
                params.push(&ints[int_idx] as &dyn ToSql);
                int_idx += 1;
            }
            Value::BigInt(Some(_)) => {
                params.push(&big_ints[big_int_idx] as &dyn ToSql);
                big_int_idx += 1;
            }
            Value::String(Some(_)) => {
                params.push(&strings[string_idx] as &dyn ToSql);
                string_idx += 1;
            }
            Value::Bytes(Some(_)) => {
                params.push(&bytes[byte_idx] as &dyn ToSql);
                byte_idx += 1;
            }
            Value::Bool(None) | Value::Int(None) | 
            Value::BigInt(None) | Value::String(None) | 
            Value::Bytes(None) => {
                params.push(&nulls[null_idx] as &dyn ToSql);
                null_idx += 1;
            }
            Value::TinyInt(Some(_)) | Value::SmallInt(Some(_)) |
            Value::TinyUnsigned(Some(_)) | Value::SmallUnsigned(Some(_)) => {
                params.push(&ints[int_idx] as &dyn ToSql);
                int_idx += 1;
            }
            Value::Unsigned(Some(_)) | Value::BigUnsigned(Some(_)) => {
                params.push(&big_ints[big_int_idx] as &dyn ToSql);
                big_int_idx += 1;
            }
            Value::Float(Some(_)) => {
                params.push(&floats[float_idx] as &dyn ToSql);
                float_idx += 1;
            }
            Value::Double(Some(_)) => {
                params.push(&doubles[double_idx] as &dyn ToSql);
                double_idx += 1;
            }
            Value::TinyInt(None) | Value::SmallInt(None) |
            Value::TinyUnsigned(None) | Value::SmallUnsigned(None) |
            Value::Unsigned(None) | Value::BigUnsigned(None) |
            Value::Float(None) | Value::Double(None) => {
                params.push(&nulls[null_idx] as &dyn ToSql);
                null_idx += 1;
            }
            Value::Json(Some(_)) => {
                params.push(&strings[string_idx] as &dyn ToSql);
                string_idx += 1;
            }
            Value::Json(None) => {
                params.push(&nulls[null_idx] as &dyn ToSql);
                null_idx += 1;
            }
            _ => {
                return Err(ActiveModelError::Other(format!(
                    "Unsupported value type in query: {:?}",
                    value
                )));
            }
        }
    }
    
    // Execute the closure with the parameters (references are valid here)
    f(&params)
}

/// Wrapper for ActiveModel field values with metadata
///
/// Similar to SeaORM's `ActiveValue`, this enum wraps field values with
/// information about whether they are set, unset, or have been modified.
///
/// # Example
///
/// ```no_run
/// use lifeguard::ActiveValue;
///
/// // Set value
/// let value = ActiveValue::Set(sea_query::Value::Int(Some(42)));
///
/// // Unset value (field not initialized)
/// let unset = ActiveValue::Unset;
///
/// // Not set (explicitly set to None for Option fields)
/// let not_set = ActiveValue::NotSet;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum ActiveValue {
    /// Value is set (field has a value)
    Set(Value),
    /// Value is not set (field is uninitialized/None)
    NotSet,
    /// Value is unset (field was never set, different from NotSet for Option fields)
    Unset,
}

impl ActiveValue {
    /// Convert to `Option<Value>`
    ///
    /// Returns `Some(Value)` if the value is `Set`, `None` otherwise.
    pub fn into_value(self) -> Option<Value> {
        match self {
            ActiveValue::Set(v) => Some(v),
            ActiveValue::NotSet | ActiveValue::Unset => None,
        }
    }

    /// Convert from `Option<Value>`
    ///
    /// Creates an `ActiveValue` from an `Option<Value>`:
    /// - `Some(value)` → `ActiveValue::Set(value)`
    /// - `None` → `ActiveValue::NotSet`
    pub fn from_value(value: Option<Value>) -> Self {
        match value {
            Some(v) => ActiveValue::Set(v),
            None => ActiveValue::NotSet,
        }
    }

    /// Check if the value is set
    pub fn is_set(&self) -> bool {
        matches!(self, ActiveValue::Set(_))
    }

    /// Check if the value is not set
    pub fn is_not_set(&self) -> bool {
        matches!(self, ActiveValue::NotSet)
    }

    /// Check if the value is unset
    pub fn is_unset(&self) -> bool {
        matches!(self, ActiveValue::Unset)
    }

    /// Get the value if set, otherwise return None
    pub fn as_value(&self) -> Option<&Value> {
        match self {
            ActiveValue::Set(v) => Some(v),
            ActiveValue::NotSet | ActiveValue::Unset => None,
        }
    }
}

impl From<Value> for ActiveValue {
    fn from(value: Value) -> Self {
        ActiveValue::Set(value)
    }
}

impl From<Option<Value>> for ActiveValue {
    fn from(value: Option<Value>) -> Self {
        ActiveValue::from_value(value)
    }
}

impl From<ActiveValue> for Option<Value> {
    fn from(value: ActiveValue) -> Self {
        value.into_value()
    }
}

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
    /// Record not found (e.g., UPDATE/DELETE affected zero rows)
    RecordNotFound,
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
            ActiveModelError::RecordNotFound => {
                write!(f, "Record not found (no rows affected)")
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

    /// Convert a column value to `ActiveValue`
    ///
    /// This method wraps the column value in an `ActiveValue` enum, which provides
    /// metadata about whether the value is set, not set, or unset.
    ///
    /// # Arguments
    ///
    /// * `column` - The column to get the value for
    ///
    /// # Returns
    ///
    /// Returns `ActiveValue::Set(value)` if the field is set, `ActiveValue::NotSet` if it's None,
    /// or `ActiveValue::Unset` if the field was never initialized.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{ActiveModelTrait, ActiveValue};
    ///
    /// # struct UserRecord;
    /// # impl ActiveModelTrait for UserRecord {
    /// #     type Entity = ();
    /// #     type Model = ();
    /// #     fn get(&self, _: <() as LifeModelTrait>::Column) -> Option<Value> { None }
    /// #     fn set(&mut self, _: <() as LifeModelTrait>::Column, _: Value) -> Result<(), ActiveModelError> { Ok(()) }
    /// #     fn take(&mut self, _: <() as LifeModelTrait>::Column) -> Option<Value> { None }
    /// #     fn reset(&mut self) {}
    /// #     // ... other methods
    /// # }
    /// # let mut record = UserRecord;
    /// # let column = ();
    ///
    /// let active_value = record.into_active_value(column);
    /// match active_value {
    ///     ActiveValue::Set(value) => println!("Value is set: {:?}", value),
    ///     ActiveValue::NotSet => println!("Value is explicitly None"),
    ///     ActiveValue::Unset => println!("Value was never set"),
    /// }
    /// ```
    fn into_active_value(
        &self,
        column: <Self::Entity as LifeModelTrait>::Column,
    ) -> ActiveValue {
        // Default implementation: convert get() result to ActiveValue
        // Records can override this to provide more detailed state information
        match self.get(column) {
            Some(value) => {
                // Check if the value is a None variant (field is not set)
                // For Option<T> fields, get() returns Some(Value::String(None)) when field is None
                match &value {
                    sea_query::Value::String(None)
                    | sea_query::Value::Int(None)
                    | sea_query::Value::BigInt(None)
                    | sea_query::Value::SmallInt(None)
                    | sea_query::Value::TinyInt(None)
                    | sea_query::Value::BigUnsigned(None)
                    | sea_query::Value::Float(None)
                    | sea_query::Value::Double(None)
                    | sea_query::Value::Bool(None)
                    | sea_query::Value::Bytes(None)
                    | sea_query::Value::Json(None) => ActiveValue::NotSet,
                    _ => ActiveValue::Set(value),
                }
            }
            None => ActiveValue::NotSet, // get() returned None (shouldn't happen with current implementation)
        }
    }

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

    /// Deserialize an ActiveModel from JSON
    ///
    /// This method constructs an ActiveModel by interpreting JSON input.
    /// Fields not present in the JSON automatically become `ActiveValue::NotSet`.
    ///
    /// # Arguments
    ///
    /// * `json` - JSON value to deserialize from
    ///
    /// # Returns
    ///
    /// Returns a new ActiveModel instance with fields set from the JSON.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{ActiveModelTrait, LifeModelTrait};
    /// use serde_json::json;
    ///
    /// # struct UserRecord;
    /// # impl ActiveModelTrait for UserRecord {
    /// #     type Entity = ();
    /// #     type Model = ();
    /// #     fn get(&self, _: <() as LifeModelTrait>::Column) -> Option<Value> { None }
    /// #     fn set(&mut self, _: <() as LifeModelTrait>::Column, _: Value) -> Result<(), ActiveModelError> { Ok(()) }
    /// #     fn take(&mut self, _: <() as LifeModelTrait>::Column) -> Option<Value> { None }
    /// #     fn reset(&mut self) {}
    /// #     // ... other methods
    /// # }
    ///
    /// let json = json!({
    ///     "name": "John",
    ///     "email": "john@example.com"
    /// });
    ///
    /// let record = UserRecord::from_json(json)?;
    /// ```
    fn from_json(_json: JsonValue) -> Result<Self, ActiveModelError>
    where
        Self: Sized,
    {
        // Default implementation: This is a placeholder that should be overridden
        // by the macro-generated implementation in LifeRecord.
        // The macro can generate an implementation that:
        // 1. Deserializes JSON into Model (if Model implements Deserialize), then uses from_model()
        // 2. Or directly parses JSON and uses set() to set Record fields
        Err(ActiveModelError::Other(
            "from_json() not implemented - LifeRecord macro should generate this method".to_string()
        ))
    }

    /// Serialize an ActiveModel to JSON
    ///
    /// This method converts the ActiveModel to a JSON representation.
    /// Only fields that are set (not `NotSet` or `Unset`) are included.
    ///
    /// # Returns
    ///
    /// Returns a JSON value representing the ActiveModel.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{ActiveModelTrait, LifeModelTrait};
    ///
    /// # struct UserRecord;
    /// # impl ActiveModelTrait for UserRecord {
    /// #     type Entity = ();
    /// #     type Model = ();
    /// #     fn get(&self, _: <() as LifeModelTrait>::Column) -> Option<Value> { None }
    /// #     fn set(&mut self, _: <() as LifeModelTrait>::Column, _: Value) -> Result<(), ActiveModelError> { Ok(()) }
    /// #     fn take(&mut self, _: <() as LifeModelTrait>::Column) -> Option<Value> { None }
    /// #     fn reset(&mut self) {}
    /// #     // ... other methods
    /// # }
    /// # let record = UserRecord;
    ///
    /// let json = record.to_json()?;
    /// ```
    fn to_json(&self) -> Result<JsonValue, ActiveModelError> {
        // Default implementation: This is a placeholder that should be overridden
        // by the macro-generated implementation in LifeRecord.
        // The macro can generate an implementation that:
        // 1. Converts Record to Model using to_model(), then serializes (if Model implements Serialize)
        // 2. Or directly iterates over columns and builds JSON from get() values
        Err(ActiveModelError::Other(
            "to_json() not implemented - LifeRecord macro should generate this method".to_string()
        ))
    }
}

/// ActiveModelBehavior trait for lifecycle hooks
///
/// This trait allows you to define custom behavior that runs before or after
/// CRUD operations. All methods have default empty implementations, so you
/// only need to override the hooks you want to use.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{ActiveModelBehavior, ActiveModelTrait};
///
/// struct UserRecord;
///
/// impl ActiveModelBehavior for UserRecord {
///     fn before_insert(&mut self) -> Result<(), ActiveModelError> {
///         // Set default values, validate, etc.
///         Ok(())
///     }
///
///     fn after_insert(&mut self, model: &Self::Model) -> Result<(), ActiveModelError> {
///         // Log, send notifications, etc.
///         Ok(())
///     }
/// }
/// ```
pub trait ActiveModelBehavior: ActiveModelTrait {
    /// Hook called before insert operation
    ///
    /// This is called before the INSERT query is executed. You can use this to:
    /// - Set default values
    /// - Validate data
    /// - Transform fields
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` to continue with the insert, or an error to abort.
    fn before_insert(&mut self) -> Result<(), ActiveModelError> {
        Ok(())
    }

    /// Hook called after insert operation
    ///
    /// This is called after the INSERT query is executed successfully.
    /// The `model` parameter contains the inserted model (with generated IDs).
    ///
    /// # Arguments
    ///
    /// * `model` - The model that was inserted (includes generated primary key values)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if post-processing fails.
    fn after_insert(&mut self, _model: &Self::Model) -> Result<(), ActiveModelError> {
        Ok(())
    }

    /// Hook called before update operation
    ///
    /// This is called before the UPDATE query is executed. You can use this to:
    /// - Validate changes
    /// - Set updated_at timestamps
    /// - Transform fields
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` to continue with the update, or an error to abort.
    fn before_update(&mut self) -> Result<(), ActiveModelError> {
        Ok(())
    }

    /// Hook called after update operation
    ///
    /// This is called after the UPDATE query is executed successfully.
    /// The `model` parameter contains the updated model.
    ///
    /// # Arguments
    ///
    /// * `model` - The model that was updated
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if post-processing fails.
    fn after_update(&mut self, _model: &Self::Model) -> Result<(), ActiveModelError> {
        Ok(())
    }

    /// Hook called before save operation (insert or update)
    ///
    /// This is called before the save operation determines whether to insert or update.
    /// You can use this to:
    /// - Set default values
    /// - Validate data
    /// - Transform fields
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` to continue with the save, or an error to abort.
    fn before_save(&mut self) -> Result<(), ActiveModelError> {
        Ok(())
    }

    /// Hook called after save operation (insert or update)
    ///
    /// This is called after the save operation completes successfully.
    /// The `model` parameter contains the saved model.
    ///
    /// # Arguments
    ///
    /// * `model` - The model that was saved (inserted or updated)
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if post-processing fails.
    fn after_save(&mut self, _model: &Self::Model) -> Result<(), ActiveModelError> {
        Ok(())
    }

    /// Hook called before delete operation
    ///
    /// This is called before the DELETE query is executed. You can use this to:
    /// - Validate deletion is allowed
    /// - Perform soft deletes (set a deleted_at flag instead)
    /// - Check dependencies
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` to continue with the delete, or an error to abort.
    fn before_delete(&mut self) -> Result<(), ActiveModelError> {
        Ok(())
    }

    /// Hook called after delete operation
    ///
    /// This is called after the DELETE query is executed successfully.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if post-processing fails.
    fn after_delete(&mut self) -> Result<(), ActiveModelError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LifeModelTrait, LifeEntityName};
    use sea_query::{Iden, IdenStatic};

    // Test entity for hook tests
    #[derive(Copy, Clone, Debug)]
    enum TestColumn {
        Id,
    }
    
    impl Iden for TestColumn {
        fn unquoted(&self) -> &str { "id" }
    }
    
    impl IdenStatic for TestColumn {
        fn as_str(&self) -> &'static str { "id" }
    }
    
    #[derive(Copy, Clone, Debug, Default)]
    struct TestEntity;
    
    impl LifeEntityName for TestEntity {
        fn table_name(&self) -> &'static str { "test_entities" }
    }
    
    #[derive(Clone, Debug)]
    struct TestModel;
    
    impl crate::ModelTrait for TestModel {
        type Entity = TestEntity;
        fn get(&self, _column: TestColumn) -> sea_query::Value {
            sea_query::Value::Int(Some(1))
        }
        fn set(&mut self, _column: TestColumn, _value: sea_query::Value) -> Result<(), crate::ModelError> {
            Ok(())
        }
        fn get_primary_key_value(&self) -> sea_query::Value {
            sea_query::Value::Int(Some(1))
        }
    }
    
    impl LifeModelTrait for TestEntity {
        type Model = TestModel;
        type Column = TestColumn;
    }

    // ============================================================================
    // ActiveModelBehavior Hook Edge Cases
    // ============================================================================

    #[test]
    fn test_hook_error_propagates() {
        // EDGE CASE: Error in before_* hook should abort operation
        #[derive(Clone, Debug)]
        struct ErrorHookRecord {
            should_error: bool,
        }
        
        impl ActiveModelTrait for ErrorHookRecord {
            type Entity = TestEntity;
            type Model = TestModel;
            
            fn get(&self, _column: TestColumn) -> Option<sea_query::Value> {
                None
            }
            
            fn set(&mut self, _column: TestColumn, _value: sea_query::Value) -> Result<(), ActiveModelError> {
                Ok(())
            }
            
            fn take(&mut self, _column: TestColumn) -> Option<sea_query::Value> {
                None
            }
            
            fn reset(&mut self) {}
            
            fn insert<E: crate::LifeExecutor>(&self, _executor: &E) -> Result<Self::Model, ActiveModelError> {
                Err(ActiveModelError::Other("not implemented".to_string()))
            }
            
            fn update<E: crate::LifeExecutor>(&self, _executor: &E) -> Result<Self::Model, ActiveModelError> {
                Err(ActiveModelError::Other("not implemented".to_string()))
            }
            
            fn save<E: crate::LifeExecutor>(&self, _executor: &E) -> Result<Self::Model, ActiveModelError> {
                Err(ActiveModelError::Other("not implemented".to_string()))
            }
            
            fn delete<E: crate::LifeExecutor>(&self, _executor: &E) -> Result<(), ActiveModelError> {
                Err(ActiveModelError::Other("not implemented".to_string()))
            }
            
            fn from_json(_json: serde_json::Value) -> Result<Self, ActiveModelError> {
                Err(ActiveModelError::Other("not implemented".to_string()))
            }
            
            fn to_json(&self) -> Result<serde_json::Value, ActiveModelError> {
                Err(ActiveModelError::Other("not implemented".to_string()))
            }
        }
        
        impl ActiveModelBehavior for ErrorHookRecord {
            fn before_insert(&mut self) -> Result<(), ActiveModelError> {
                if self.should_error {
                    Err(ActiveModelError::Other("Validation failed".to_string()))
                } else {
                    Ok(())
                }
            }
        }
        
        let mut record = ErrorHookRecord {
            should_error: true,
        };
        
        // Error should propagate
        assert!(record.before_insert().is_err());
        
        record.should_error = false;
        assert!(record.before_insert().is_ok());
    }

    #[test]
    fn test_hook_order_insert_vs_save() {
        // EDGE CASE: Hook execution order for save() vs insert()
        // save() should call before_save -> before_insert -> insert -> after_insert -> after_save
        #[derive(Clone, Debug)]
        struct OrderTrackingRecord {
            call_order: Vec<String>,
        }
        
        impl ActiveModelTrait for OrderTrackingRecord {
            type Entity = TestEntity;
            type Model = TestModel;
            
            fn get(&self, _column: TestColumn) -> Option<sea_query::Value> {
                None
            }
            
            fn set(&mut self, _column: TestColumn, _value: sea_query::Value) -> Result<(), ActiveModelError> {
                Ok(())
            }
            
            fn take(&mut self, _column: TestColumn) -> Option<sea_query::Value> {
                None
            }
            
            fn reset(&mut self) {}
            
            fn insert<E: crate::LifeExecutor>(&self, _executor: &E) -> Result<Self::Model, ActiveModelError> {
                Err(ActiveModelError::Other("not implemented".to_string()))
            }
            
            fn update<E: crate::LifeExecutor>(&self, _executor: &E) -> Result<Self::Model, ActiveModelError> {
                Err(ActiveModelError::Other("not implemented".to_string()))
            }
            
            fn save<E: crate::LifeExecutor>(&self, _executor: &E) -> Result<Self::Model, ActiveModelError> {
                Err(ActiveModelError::Other("not implemented".to_string()))
            }
            
            fn delete<E: crate::LifeExecutor>(&self, _executor: &E) -> Result<(), ActiveModelError> {
                Err(ActiveModelError::Other("not implemented".to_string()))
            }
            
            fn from_json(_json: serde_json::Value) -> Result<Self, ActiveModelError> {
                Err(ActiveModelError::Other("not implemented".to_string()))
            }
            
            fn to_json(&self) -> Result<serde_json::Value, ActiveModelError> {
                Err(ActiveModelError::Other("not implemented".to_string()))
            }
        }
        
        impl ActiveModelBehavior for OrderTrackingRecord {
            fn before_save(&mut self) -> Result<(), ActiveModelError> {
                self.call_order.push("before_save".to_string());
                Ok(())
            }
            
            fn before_insert(&mut self) -> Result<(), ActiveModelError> {
                self.call_order.push("before_insert".to_string());
                Ok(())
            }
            
            fn after_insert(&mut self, _model: &Self::Model) -> Result<(), ActiveModelError> {
                self.call_order.push("after_insert".to_string());
                Ok(())
            }
            
            fn after_save(&mut self, _model: &Self::Model) -> Result<(), ActiveModelError> {
                self.call_order.push("after_save".to_string());
                Ok(())
            }
        }
        
        let mut record = OrderTrackingRecord {
            call_order: Vec::new(),
        };
        
        // Test hook order (conceptual - full test requires executor)
        record.before_save().unwrap();
        record.before_insert().unwrap();
        // insert() would be called here
        let model = TestModel;
        record.after_insert(&model).unwrap();
        record.after_save(&model).unwrap();
        
        // Verify order
        assert_eq!(record.call_order, vec!["before_save", "before_insert", "after_insert", "after_save"]);
    }
}
