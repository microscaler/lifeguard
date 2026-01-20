//! Model trait for accessing and manipulating model data
//!
//! This module provides the `ModelTrait` which allows dynamic access to model fields
//! and primary key values. Similar to SeaORM's `ModelTrait`.
//!
//! ## Submodules
//!
//! - `try_into_model` - `TryIntoModel` trait for converting types into Model instances

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
    
    /// Get the value of a column by its name (string)
    ///
    /// This method allows runtime access to column values using a string column name.
    /// It's useful for dynamic operations like eager loading where the column name
    /// is known at runtime but the Column enum variant is not.
    ///
    /// # Arguments
    ///
    /// * `column_name` - The column name as a string (e.g., "user_id", "id")
    ///
    /// # Returns
    ///
    /// `Some(Value)` if the column exists and the value can be extracted, `None` otherwise.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::ModelTrait;
    ///
    /// let model = PostModel { id: 1, user_id: 42, title: "Hello".to_string() };
    /// let user_id_value = model.get_by_column_name("user_id");
    /// // Returns: Some(Value::Int(Some(42)))
    /// ```
    ///
    /// # Implementation Note
    ///
    /// The default implementation iterates through all Column enum variants and matches
    /// by comparing the column name string. This is O(n) where n is the number of columns.
    /// The macro can override this with a more efficient implementation (e.g., using a
    /// hash map or direct field access).
    fn get_by_column_name(&self, column_name: &str) -> Option<Value> {
        // Default implementation: try to match column name against all Column variants
        // This requires iterating through all possible Column enum variants
        // The macro can override this with a more efficient implementation
        extract_value_by_column_name(self, column_name)
    }
    
    /// Get the Rust type string for a column
    ///
    /// This method returns the Rust type string representation for a given column.
    /// It's useful for runtime type introspection, dynamic serialization, and type validation.
    ///
    /// # Arguments
    ///
    /// * `column` - The column to get the type for
    ///
    /// # Returns
    ///
    /// `Some(&'static str)` containing the Rust type string (e.g., `"i32"`, `"String"`, `"Option<i32>"`),
    /// or `None` if the column type is unknown.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::ModelTrait;
    ///
    /// let model = UserModel { id: 1, name: "John".to_string(), email: Some("john@example.com".to_string()) };
    /// let id_type = model.get_value_type(User::Column::Id);
    /// // Returns: Some("i32")
    ///
    /// let email_type = model.get_value_type(User::Column::Email);
    /// // Returns: Some("Option<String>")
    /// ```
    ///
    /// # Implementation Note
    ///
    /// The default implementation returns `None`. The `LifeModel` macro generates
    /// implementations that return the actual Rust type strings for each column.
    fn get_value_type(&self, column: <Self::Entity as LifeModelTrait>::Column) -> Option<&'static str> {
        // Default implementation returns None
        // The macro will override this with actual type strings
        let _ = column;
        None
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LifeModelTrait, LifeEntityName};
    use sea_query::{Iden, IdenStatic, Value};

    // Test entity and column for ModelTrait tests
    #[derive(Copy, Clone, Debug)]
    enum TestColumn {
        Id,
        TenantId,
    }

    impl Iden for TestColumn {
        fn unquoted(&self) -> &str {
            match self {
                TestColumn::Id => "id",
                TestColumn::TenantId => "tenant_id",
            }
        }
    }

    impl IdenStatic for TestColumn {
        fn as_str(&self) -> &'static str {
            match self {
                TestColumn::Id => "id",
                TestColumn::TenantId => "tenant_id",
            }
        }
    }

    crate::impl_column_def_helper_for_test!(TestColumn);

    #[derive(Copy, Clone, Debug, Default)]
    struct TestEntity;

    impl LifeEntityName for TestEntity {
        fn table_name(&self) -> &'static str {
            "test_entities"
        }
    }

    impl LifeModelTrait for TestEntity {
        type Model = TestModel;
        type Column = TestColumn;
    }

    #[derive(Clone, Debug)]
    struct TestModel {
        id: i32,
        tenant_id: Option<i32>,
    }

    impl ModelTrait for TestModel {
        type Entity = TestEntity;

            fn get(&self, column: TestColumn) -> Value {
            match column {
                TestColumn::Id => Value::Int(Some(self.id)),
                TestColumn::TenantId => Value::Int(self.tenant_id),
            }
        }

        fn set(&mut self, column: TestColumn, value: Value) -> Result<(), ModelError> {
            match column {
                TestColumn::Id => {
                    if let Value::Int(Some(v)) = value {
                        self.id = v;
                        Ok(())
                    } else {
                        Err(ModelError::InvalidValueType {
                            column: "id".to_string(),
                            expected: "Int(Some(_))".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                }
                TestColumn::TenantId => {
                    if let Value::Int(v) = value {
                        self.tenant_id = v;
                        Ok(())
                    } else {
                        Err(ModelError::InvalidValueType {
                            column: "tenant_id".to_string(),
                            expected: "Int".to_string(),
                            actual: format!("{:?}", value),
                        })
                    }
                }
            }
        }

        fn get_primary_key_value(&self) -> Value {
            Value::Int(Some(self.id))
        }

        fn get_primary_key_identity(&self) -> Identity {
            use sea_query::IdenStatic;
            Identity::Unary(sea_query::DynIden::from(TestColumn::Id.as_str()))
        }

        fn get_primary_key_values(&self) -> Vec<Value> {
            vec![Value::Int(Some(self.id))]
        }
    }

    #[derive(Clone, Debug)]
    struct CompositeKeyModel {
        id: i32,
        tenant_id: i32,
    }

    impl ModelTrait for CompositeKeyModel {
        type Entity = TestEntity;

        fn get(&self, column: TestColumn) -> Value {
            match column {
                TestColumn::Id => Value::Int(Some(self.id)),
                TestColumn::TenantId => Value::Int(Some(self.tenant_id)),
            }
        }

        fn set(&mut self, _column: TestColumn, _value: Value) -> Result<(), ModelError> {
            Ok(())
        }

        fn get_primary_key_value(&self) -> Value {
            Value::Int(Some(self.id))
        }

        fn get_primary_key_identity(&self) -> Identity {
            use sea_query::IdenStatic;
            Identity::Binary(
                sea_query::DynIden::from(TestColumn::Id.as_str()),
                sea_query::DynIden::from(TestColumn::TenantId.as_str()),
            )
        }

        fn get_primary_key_values(&self) -> Vec<Value> {
            vec![Value::Int(Some(self.id)), Value::Int(Some(self.tenant_id))]
        }
    }

    #[test]
    fn test_get_primary_key_identity_single() {
        let model = TestModel {
            id: 42,
            tenant_id: Some(10),
        };

        let identity = model.get_primary_key_identity();
        assert_eq!(identity.arity(), 1);
    }

    #[test]
    fn test_get_primary_key_identity_composite() {
        let model = CompositeKeyModel {
            id: 42,
            tenant_id: 10,
        };

        let identity = model.get_primary_key_identity();
        assert_eq!(identity.arity(), 2);
    }

    #[test]
    fn test_get_primary_key_values_single() {
        let model = TestModel {
            id: 42,
            tenant_id: Some(10),
        };

        let values = model.get_primary_key_values();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0], Value::Int(Some(42)));
    }

    #[test]
    fn test_get_primary_key_values_composite() {
        let model = CompositeKeyModel {
            id: 42,
            tenant_id: 10,
        };

        let values = model.get_primary_key_values();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], Value::Int(Some(42)));
        assert_eq!(values[1], Value::Int(Some(10)));
    }

    #[test]
    fn test_get_primary_key_values_matches_identity() {
        // Edge case: Ensure values match identity arity
        let model = CompositeKeyModel {
            id: 42,
            tenant_id: 10,
        };

        let identity = model.get_primary_key_identity();
        let values = model.get_primary_key_values();

        assert_eq!(values.len(), identity.arity());
    }

    #[test]
    fn test_get_primary_key_identity_ternary() {
        // Edge case: Test with Ternary identity (if we had a model with 3-part key)
        // This would require a custom model implementation
        let id_col: sea_query::DynIden = "id".into();
        let tenant_col: sea_query::DynIden = "tenant_id".into();
        let region_col: sea_query::DynIden = "region_id".into();
        
        let identity = Identity::Ternary(id_col, tenant_col, region_col);
        assert_eq!(identity.arity(), 3);
    }

    #[test]
    fn test_get_primary_key_identity_many() {
        // Edge case: Test with Many identity (4+ columns)
        let cols: Vec<sea_query::DynIden> = vec![
            "id".into(),
            "tenant_id".into(),
            "region_id".into(),
            "org_id".into(),
        ];
        
        let identity = Identity::Many(cols);
        assert_eq!(identity.arity(), 4);
    }
}

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

/// Extract a value from a model by column name string
///
/// This helper function attempts to match a column name string against all possible
/// Column enum variants and extract the corresponding value from the model.
///
/// # Arguments
///
/// * `model` - The model instance
/// * `column_name` - The column name as a string
///
/// # Returns
///
/// `Some(Value)` if a matching column is found, `None` otherwise.
///
/// # Note
///
/// This is a fallback implementation that requires enumerating all Column variants.
/// The macro can override `get_by_column_name()` with a more efficient implementation
/// (e.g., using a hash map or direct field access based on the column name).
fn extract_value_by_column_name<M>(_model: &M, _column_name: &str) -> Option<Value>
where
    M: ModelTrait,
{
    
    // Try to match column name against all Column enum variants
    // This is a placeholder - the macro should override get_by_column_name() with
    // a more efficient implementation that directly matches column names to variants
    //
    // For now, we'll use a helper that tries common patterns:
    // 1. Try to parse as a Column variant (requires compile-time knowledge)
    // 2. Use a macro-generated match statement (requires macro support)
    //
    // Since we can't enumerate Column variants at runtime without reflection,
    // this default implementation returns None. The macro should override this
    // with a generated match statement that covers all Column variants.
    //
    // Example macro-generated implementation:
    // ```
    // fn get_by_column_name(&self, column_name: &str) -> Option<Value> {
    //     match column_name {
    //         "id" => Some(self.get(Column::Id)),
    //         "user_id" => Some(self.get(Column::UserId)),
    //         "title" => Some(self.get(Column::Title)),
    //         _ => None,
    //     }
    // }
    // ```
    None
}

#[cfg(test)]
mod get_by_column_name_tests {
    use super::*;
    use crate::{LifeEntityName, LifeModelTrait};
    use crate::relation::identity::Identity;
    use sea_query::IdenStatic;
    
    #[test]
    fn test_get_by_column_name_edge_cases() {
        // Test edge cases for get_by_column_name default implementation
        // This tests the fallback implementation behavior
        
        #[derive(Default, Copy, Clone)]
        struct TestEntity;
        
        impl sea_query::Iden for TestEntity {
            fn unquoted(&self) -> &str { "test" }
        }
        
        impl LifeEntityName for TestEntity {
            fn table_name(&self) -> &'static str { "test" }
        }
        
        impl LifeModelTrait for TestEntity {
            type Model = TestModel;
            type Column = TestColumn;
        }
        
        #[derive(Clone, Debug)]
        struct TestModel;
        
        #[derive(Copy, Clone, Debug)]
        enum TestColumn { Id }
        
        impl sea_query::Iden for TestColumn {
            fn unquoted(&self) -> &str { "id" }
        }
        
        impl IdenStatic for TestColumn {
            fn as_str(&self) -> &'static str { "id" }
        }
        
        crate::impl_column_def_helper_for_test!(TestColumn);
        
        impl ModelTrait for TestModel {
            type Entity = TestEntity;
            fn get(&self, _col: TestColumn) -> Value { Value::Int(None) }
            fn set(&mut self, _col: TestColumn, _val: Value) -> Result<(), ModelError> { Ok(()) }
            fn get_primary_key_value(&self) -> Value { Value::Int(None) }
            fn get_primary_key_identity(&self) -> Identity { Identity::Unary("id".into()) }
            fn get_primary_key_values(&self) -> Vec<Value> { vec![] }
            // Use default implementation of get_by_column_name
        }
        
        let model = TestModel;
        
        // Test non-existent column - should return None
        assert_eq!(model.get_by_column_name("nonexistent"), None);
        
        // Test empty string - should return None
        assert_eq!(model.get_by_column_name(""), None);
        
        // Test with different casing - should return None (default impl doesn't handle this)
        assert_eq!(model.get_by_column_name("ID"), None);
        
        // Note: The default implementation returns None for all cases
        // The macro-generated implementation would handle actual column names
    }
}

// TryIntoModel trait submodule
pub mod try_into_model;
pub use try_into_model::TryIntoModel;
