//! Core traits for ActiveModel operations.
//!
//! This module provides `ActiveModelTrait` and `ActiveModelBehavior` for mutable
//! model operations including field access, CRUD operations, and lifecycle hooks.

use crate::executor::LifeExecutor;
use crate::query::LifeModelTrait;
use crate::model::ModelTrait;
use super::error::ActiveModelError;
use super::value::ActiveValue;
use sea_query::Value;
use serde_json::Value as JsonValue;

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
                    Value::String(None)
                    | Value::Int(None)
                    | Value::BigInt(None)
                    | Value::SmallInt(None)
                    | Value::TinyInt(None)
                    | Value::BigUnsigned(None)
                    | Value::Float(None)
                    | Value::Double(None)
                    | Value::Bool(None)
                    | Value::Bytes(None)
                    | Value::Json(None) => ActiveValue::NotSet,
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
    
    crate::impl_column_def_helper_for_test!(TestColumn);
    
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
        fn get_primary_key_identity(&self) -> crate::Identity {
            use crate::relation::identity::Identity;
            use sea_query::IntoIden;
            Identity::Unary(TestColumn::Id.into_iden())
        }
        fn get_primary_key_values(&self) -> Vec<sea_query::Value> {
            vec![sea_query::Value::Int(Some(1))]
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
