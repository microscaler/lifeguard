//! Tests for DeriveTryIntoModel macro
//!
//! These tests verify that the DeriveTryIntoModel macro correctly generates
//! TryIntoModel trait implementations for converting custom types into Models.

use lifeguard_derive::DeriveTryIntoModel;
use lifeguard::{TryIntoModel, ModelTrait, LifeModelTrait, LifeEntityName};

// Test entities and models
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
    Email,
}

impl sea_query::Iden for UserColumn {
    fn unquoted(&self) -> &str {
        match self {
            UserColumn::Id => "id",
            UserColumn::Name => "name",
            UserColumn::Email => "email",
        }
    }
}

impl sea_query::IdenStatic for UserColumn {
    fn as_str(&self) -> &'static str {
        match self {
            UserColumn::Id => "id",
            UserColumn::Name => "name",
            UserColumn::Email => "email",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserModel {
    pub id: i32,
    pub name: String,
    pub email: String,
}

impl ModelTrait for UserModel {
    type Entity = UserEntity;

    fn get(&self, column: UserColumn) -> sea_query::Value {
        match column {
            UserColumn::Id => sea_query::Value::Int(Some(self.id)),
            UserColumn::Name => sea_query::Value::String(Some(self.name.clone())),
            UserColumn::Email => sea_query::Value::String(Some(self.email.clone())),
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
            UserColumn::Email => {
                if let sea_query::Value::String(Some(email)) = value {
                    self.email = email;
                    Ok(())
                } else {
                    Err(lifeguard::ModelError::InvalidValueType {
                        column: "email".to_string(),
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

// Test DTO structs
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel")]
struct CreateUserRequest {
    name: String,
    email: String,
    // Note: id is missing - will need to be handled (use Default::default() or add it)
}

#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel")]
struct CreateUserRequestWithId {
    id: i32,
    name: String,
    email: String,
}

#[test]
fn test_derive_try_into_model_basic() {
    // Test basic conversion with all fields
    let request = CreateUserRequestWithId {
        id: 1,
        name: "John".to_string(),
        email: "john@example.com".to_string(),
    };

    let model: Result<UserModel, _> = request.try_into_model();
    assert!(model.is_ok());
    let model = model.unwrap();
    assert_eq!(model.id, 1);
    assert_eq!(model.name, "John");
    assert_eq!(model.email, "john@example.com");
}

#[test]
fn test_derive_try_into_model_with_default() {
    // Test conversion with missing fields (will use Default::default() for id)
    let request = CreateUserRequest {
        name: "Jane".to_string(),
        email: "jane@example.com".to_string(),
    };

    // The macro uses ..Default::default() to handle missing fields
    // UserModel implements Default, so id will be 0 (default for i32)
    let model: Result<UserModel, _> = request.try_into_model();
    assert!(model.is_ok());
    let model = model.unwrap();
    assert_eq!(model.id, 0); // Default value for i32
    assert_eq!(model.name, "Jane");
    assert_eq!(model.email, "jane@example.com");
}

#[test]
fn test_derive_try_into_model_error_type() {
    // Test that the error type is LifeError by default
    let request = CreateUserRequestWithId {
        id: 1,
        name: "Test".to_string(),
        email: "test@example.com".to_string(),
    };

    let result: Result<UserModel, lifeguard::LifeError> = request.try_into_model();
    assert!(result.is_ok());
}

// Custom error type for testing
#[derive(Debug, PartialEq)]
pub struct CustomError {
    message: String,
}

impl std::fmt::Display for CustomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CustomError {}

// Conversion function that returns CustomError
fn convert_to_uppercase(s: String) -> Result<String, CustomError> {
    Ok(s.to_uppercase())
}

// DTO with custom error type and convert attribute
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel", error = "CustomError")]
struct CreateUserRequestCustomError {
    #[lifeguard(convert = "convert_to_uppercase")]
    name: String,
    email: String,
}

#[test]
fn test_derive_try_into_model_custom_error_type_with_convert() {
    // Test that custom error types work correctly with convert attribute
    // when the conversion function returns the custom error type
    let request = CreateUserRequestCustomError {
        name: "john".to_string(),
        email: "john@example.com".to_string(),
    };

    let result: Result<UserModel, CustomError> = request.try_into_model();
    assert!(result.is_ok());
    let model = result.unwrap();
    assert_eq!(model.name, "JOHN"); // Converted to uppercase
    assert_eq!(model.email, "john@example.com");
    assert_eq!(model.id, 0); // Default value
}

// Conversion function that returns a different error type, but we implement From
#[derive(Debug, PartialEq)]
pub struct ParseError {
    message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

// Implement From<ParseError> for CustomError
impl From<ParseError> for CustomError {
    fn from(err: ParseError) -> Self {
        CustomError {
            message: format!("Parse error: {}", err.message),
        }
    }
}

// Conversion function that takes a String and returns a String, but can fail with ParseError
fn parse_and_format(s: String) -> Result<String, ParseError> {
    // Parse as number to validate, then return uppercase
    s.parse::<i32>()
        .map_err(|_| ParseError {
            message: format!("Failed to parse '{}' as integer", s),
        })?;
    Ok(s.to_uppercase())
}

#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel", error = "CustomError")]
struct CreateUserRequestWithParse {
    name: String,
    #[lifeguard(convert = "parse_and_format")]
    email: String, // This will be validated and converted
}

#[test]
fn test_derive_try_into_model_custom_error_type_with_from_trait() {
    // Test that custom error types work when conversion function returns
    // a different error type but From trait is implemented
    let request = CreateUserRequestWithParse {
        name: "Test".to_string(),
        email: "42".to_string(), // Valid number string
    };

    let result: Result<UserModel, CustomError> = request.try_into_model();
    assert!(result.is_ok());
    let model = result.unwrap();
    assert_eq!(model.id, 0); // Default value
    assert_eq!(model.name, "Test");
    assert_eq!(model.email, "42"); // Converted (uppercase of "42" is still "42")
}

#[test]
fn test_derive_try_into_model_custom_error_type_with_from_trait_error() {
    // Test that conversion errors are properly propagated through From trait
    let request = CreateUserRequestWithParse {
        name: "Test".to_string(),
        email: "not_a_number".to_string(), // Invalid - will fail to parse
    };

    let result: Result<UserModel, CustomError> = request.try_into_model();
    assert!(result.is_err());
    let err = result.unwrap_err();
    // Error should be wrapped through From<ParseError> for CustomError
    assert!(err.message.contains("Parse error"));
    assert!(err.message.contains("Failed to parse"));
}
