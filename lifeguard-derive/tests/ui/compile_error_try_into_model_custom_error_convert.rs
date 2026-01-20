//! Test that custom error type with convert attribute requires From trait
//!
//! This test verifies that when a custom error type is specified and a field
//! uses #[lifeguard(convert = "...")], the conversion function's error type
//! must implement From<ConversionError> for the custom error type.

use lifeguard_derive::DeriveTryIntoModel;
use lifeguard::{ModelTrait, LifeModelTrait, LifeEntityName};

// Test entity and model
#[derive(Default, Copy, Clone)]
pub struct UserEntity;

impl sea_query::Iden for UserEntity {
    fn unquoted(&self) -> &str {
        "users"
    }
}

impl LifeEntityName for UserEntity {
    fn table_name(&self) -> &'static str {
        "users"
    }
}

impl LifeModelTrait for UserEntity {
    type Model = UserModel;
    type Column = UserColumn;
}

#[derive(Copy, Clone, Debug)]
pub enum UserColumn {
    Id,
    Name,
}

impl sea_query::Iden for UserColumn {
    fn unquoted(&self) -> &str {
        match self {
            UserColumn::Id => "id",
            UserColumn::Name => "name",
        }
    }
}

impl sea_query::IdenStatic for UserColumn {
    fn as_str(&self) -> &'static str {
        match self {
            UserColumn::Id => "id",
            UserColumn::Name => "name",
        }
    }
}
lifeguard::impl_column_def_helper_for_test!(UserColumn);

#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserModel {
    pub id: i32,
    pub name: String,
}

impl ModelTrait for UserModel {
    type Entity = UserEntity;

    fn get(&self, column: UserColumn) -> sea_query::Value {
        match column {
            UserColumn::Id => sea_query::Value::Int(Some(self.id)),
            UserColumn::Name => sea_query::Value::String(Some(self.name.clone())),
        }
    }

    fn set(
        &mut self,
        column: UserColumn,
        value: sea_query::Value,
    ) -> Result<(), lifeguard::ModelError> {
        match column {
            UserColumn::Id => {
                if let sea_query::Value::Int(Some(id)) = value {
                    self.id = id;
                    Ok(())
                } else {
                    Err(lifeguard::ModelError::InvalidValueType {
                        column: "id".to_string(),
                        expected: "Int".to_string(),
                        actual: format!("{:?}", value),
                    })
                }
            }
            UserColumn::Name => {
                if let sea_query::Value::String(Some(name)) = value {
                    self.name = name;
                    Ok(())
                } else {
                    Err(lifeguard::ModelError::InvalidValueType {
                        column: "name".to_string(),
                        expected: "String".to_string(),
                        actual: format!("{:?}", value),
                    })
                }
            }
        }
    }

    fn get_primary_key_value(&self) -> sea_query::Value {
        sea_query::Value::Int(Some(self.id))
    }

    fn get_primary_key_identity(&self) -> lifeguard::relation::identity::Identity {
        lifeguard::relation::identity::Identity::Unary("id".into())
    }

    fn get_primary_key_values(&self) -> Vec<sea_query::Value> {
        vec![sea_query::Value::Int(Some(self.id))]
    }
}

// Custom error type that does NOT implement From<lifeguard::LifeError>
#[derive(Debug)]
pub struct CustomError {
    message: String,
}

impl std::fmt::Display for CustomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CustomError {}

// Conversion function that returns a different error type
fn convert_name(s: String) -> Result<String, std::num::ParseIntError> {
    // This is a dummy conversion that will fail
    s.parse::<i32>()?;
    Ok(s)
}

// ERROR: This should fail to compile because CustomError does not implement
// From<lifeguard::LifeError>, and the macro wraps conversion errors in LifeError::Other
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel", error = "CustomError")]
struct CreateUserRequest {
    #[lifeguard(convert = "convert_name")]
    name: String,
}
