//! `TryIntoModel` trait for converting types into `Model` instances.
//!
//! This module provides the `TryIntoModel` trait which allows converting arbitrary types
//! (DTOs, partial models, external types) into `Model` instances with proper error handling.
//!
//! # Example
//!
//! ```rust
//! use lifeguard::{TryIntoModel, ModelTrait, LifeError};
//!
//! struct CreateUserRequest {
//!     name: String,
//!     email: String,
//! }
//!
//! // Manual implementation
//! impl TryIntoModel<UserModel> for CreateUserRequest {
//!     type Error = LifeError;
//!
//!     fn try_into_model(self) -> Result<UserModel, Self::Error> {
//!         Ok(UserModel {
//!             id: 0,  // Default for new records
//!             name: self.name,
//!             email: self.email,
//!         })
//!     }
//! }
//!
//! // Or use the derive macro:
//! // #[derive(DeriveTryIntoModel)]
//! // #[lifeguard(model = "UserModel")]
//! // struct CreateUserRequest { ... }
//! ```

use crate::model::ModelTrait;

/// Trait for converting types into Model instances
///
/// This trait provides a generic way to convert arbitrary types (DTOs, partial models,
/// external types) into Model instances with proper error handling.
///
/// # Example
///
/// ```rust
/// use lifeguard::{TryIntoModel, ModelTrait, LifeError};
///
/// struct CreateUserRequest {
///     name: String,
///     email: String,
/// }
///
/// impl TryIntoModel<UserModel> for CreateUserRequest {
///     type Error = LifeError;
///
///     fn try_into_model(self) -> Result<UserModel, Self::Error> {
///         Ok(UserModel {
///             id: 0,  // Default for new records
///             name: self.name,
///             email: self.email,
///         })
///     }
/// }
/// ```
pub trait TryIntoModel<M>
where
    M: ModelTrait,
{
    /// The error type returned by conversion
    type Error: std::error::Error + Send + Sync + 'static;

    /// Attempt to convert `self` into a Model instance
    ///
    /// # Returns
    ///
    /// Returns `Ok(M)` if conversion succeeds, or `Err(Self::Error)` if conversion fails.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Required fields are missing and no defaults are available
    /// - Type conversions fail (e.g., String → i32 parse error)
    /// - Custom validation fails
    /// - Field mappings are invalid
    fn try_into_model(self) -> Result<M, Self::Error>;
}

/// Default implementation: trivial self-conversion
///
/// Any type that implements `ModelTrait` can be converted to itself.
/// This provides a convenient default implementation.
impl<M> TryIntoModel<M> for M
where
    M: ModelTrait,
{
    type Error = std::convert::Infallible;

    fn try_into_model(self) -> Result<M, Self::Error> {
        Ok(self)
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;
    use crate::model::ModelTrait;
    use crate::query::traits::LifeModelTrait;

    // Test entity and model
    #[derive(Debug, Clone, PartialEq)]
    struct TestModel {
        id: i32,
        name: String,
    }

    #[derive(Default, Copy, Clone)]
    struct TestEntity;

    impl sea_query::Iden for TestEntity {
        fn unquoted(&self) -> &'static str {
            "test_entities"
        }
    }

    impl crate::query::traits::LifeEntityName for TestEntity {
        fn table_name(&self) -> &'static str {
            "test_entities"
        }
    }

    #[derive(Copy, Clone, Debug)]
    enum TestColumn {
        Id,
        Name,
    }

    impl sea_query::Iden for TestColumn {
        fn unquoted(&self) -> &'static str {
            match self {
                TestColumn::Id => "id",
                TestColumn::Name => "name",
            }
        }
    }

    impl sea_query::IdenStatic for TestColumn {
        fn as_str(&self) -> &'static str {
            match self {
                TestColumn::Id => "id",
                TestColumn::Name => "name",
            }
        }
    }

    crate::impl_column_def_helper_for_test!(TestColumn);

    impl LifeModelTrait for TestEntity {
        type Model = TestModel;
        type Column = TestColumn;
    }

    impl ModelTrait for TestModel {
        type Entity = TestEntity;

        fn get(&self, column: <Self::Entity as LifeModelTrait>::Column) -> sea_query::Value {
            match column {
                TestColumn::Id => sea_query::Value::Int(Some(self.id)),
                TestColumn::Name => sea_query::Value::String(Some(self.name.clone())),
            }
        }

        fn set(
            &mut self,
            column: <Self::Entity as LifeModelTrait>::Column,
            value: sea_query::Value,
        ) -> Result<(), crate::model::ModelError> {
            match column {
                TestColumn::Id => {
                    if let sea_query::Value::Int(Some(id)) = value {
                        self.id = id;
                        Ok(())
                    } else {
                        Err(crate::model::ModelError::InvalidValueType {
                            column: "id".to_string(),
                            expected: "Int".to_string(),
                            actual: format!("{value:?}"),
                        })
                    }
                }
                TestColumn::Name => {
                    if let sea_query::Value::String(Some(name)) = value {
                        self.name = name;
                        Ok(())
                    } else {
                        Err(crate::model::ModelError::InvalidValueType {
                            column: "name".to_string(),
                            expected: "String".to_string(),
                            actual: format!("{value:?}"),
                        })
                    }
                }
            }
        }

        fn get_primary_key_value(&self) -> sea_query::Value {
            sea_query::Value::Int(Some(self.id))
        }

        fn get_primary_key_identity(&self) -> crate::relation::identity::Identity {
            crate::relation::identity::Identity::Unary("id".into())
        }

        fn get_primary_key_values(&self) -> Vec<sea_query::Value> {
            vec![sea_query::Value::Int(Some(self.id))]
        }
    }

    #[test]
    fn test_trivial_self_conversion() {
        let model = TestModel {
            id: 1,
            name: "Test".to_string(),
        };

        let converted: Result<TestModel, _> = model.try_into_model();
        assert!(converted.is_ok());
        let converted = converted.unwrap();
        assert_eq!(converted.id, 1);
        assert_eq!(converted.name, "Test");
    }

    #[test]
    fn test_try_into_model_trait_bounds() {
        // Test that the trait can be used with ModelTrait types
        let model = TestModel {
            id: 1,
            name: "Test".to_string(),
        };

        // This should compile and work
        // The default implementation uses Infallible as the error type
        #[allow(clippy::items_after_statements)] // Test code - function definition after statement is acceptable
        fn convert<M>(m: M) -> Result<M, std::convert::Infallible>
        where
            M: ModelTrait + TryIntoModel<M, Error = std::convert::Infallible>,
        {
            m.try_into_model()
        }

        let result = convert(model);
        assert!(result.is_ok());
    }
}
