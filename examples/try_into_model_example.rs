//! Example: Using TryIntoModel trait for DTO → Model conversions
//!
//! This example demonstrates how to use the `TryIntoModel` trait and `DeriveTryIntoModel`
//! macro to convert custom types (DTOs, request structs, etc.) into Model instances.

use lifeguard::{TryIntoModel, ModelTrait, LifeModelTrait, LifeEntityName};
use lifeguard_derive::DeriveTryIntoModel;

// Example Entity and Model (simplified for demonstration)
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

// Example 1: Basic DTO → Model conversion
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel")]
struct CreateUserRequest {
    name: String,
    email: String,
    // id is missing - will use Default::default() (0 for i32)
}

// Example 2: DTO with all fields
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel")]
struct UpdateUserRequest {
    id: i32,
    name: String,
    email: String,
}

// Example 3: DTO with custom field mapping
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel")]
struct ExternalUserData {
    #[lifeguard(map_from = "name")]
    user_name: String,
    
    #[lifeguard(map_from = "email")]
    user_email: String,
    
    // id is missing - will use Default::default()
}

fn main() {
    // Example 1: Basic conversion
    let request = CreateUserRequest {
        name: "John Doe".to_string(),
        email: "john@example.com".to_string(),
    };
    
    let model: Result<UserModel, lifeguard::LifeError> = request.try_into_model();
    match model {
        Ok(user) => {
            println!("Created user: id={}, name={}, email={}", user.id, user.name, user.email);
            // Output: Created user: id=0, name=John Doe, email=john@example.com
        }
        Err(e) => {
            eprintln!("Failed to convert request to model: {}", e);
        }
    }
    
    // Example 2: Conversion with all fields
    let update_request = UpdateUserRequest {
        id: 42,
        name: "Jane Smith".to_string(),
        email: "jane@example.com".to_string(),
    };
    
    let model: Result<UserModel, _> = update_request.try_into_model();
    match model {
        Ok(user) => {
            println!("Updated user: id={}, name={}, email={}", user.id, user.name, user.email);
            // Output: Updated user: id=42, name=Jane Smith, email=jane@example.com
        }
        Err(e) => {
            eprintln!("Failed to convert update request to model: {}", e);
        }
    }
    
    // Example 3: Conversion with custom field mapping
    let external_data = ExternalUserData {
        user_name: "Bob Johnson".to_string(),
        user_email: "bob@example.com".to_string(),
    };
    
    let model: Result<UserModel, _> = external_data.try_into_model();
    match model {
        Ok(user) => {
            println!("Converted external data: id={}, name={}, email={}", 
                     user.id, user.name, user.email);
            // Output: Converted external data: id=0, name=Bob Johnson, email=bob@example.com
        }
        Err(e) => {
            eprintln!("Failed to convert external data to model: {}", e);
        }
    }
    
    // Example 4: Trivial self-conversion (default implementation)
    let user = UserModel {
        id: 100,
        name: "Self".to_string(),
        email: "self@example.com".to_string(),
    };
    
    let converted: Result<UserModel, _> = user.try_into_model();
    match converted {
        Ok(model) => {
            println!("Self-converted user: id={}, name={}, email={}", 
                     model.id, model.name, model.email);
            // Output: Self-converted user: id=100, name=Self, email=self@example.com
        }
        Err(_) => {
            // This should never happen for self-conversion (uses Infallible error type)
            eprintln!("Unexpected error in self-conversion");
        }
    }
}
