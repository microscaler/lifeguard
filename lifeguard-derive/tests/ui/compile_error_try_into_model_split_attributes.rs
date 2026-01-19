//! Test that split attributes work correctly
//!
//! This test verifies that when attributes are split across multiple
//! #[lifeguard] blocks (e.g., #[lifeguard(map_from = "foo")] and
//! #[lifeguard(convert = "bar")] on separate lines), both attributes
//! are correctly extracted and used.

use lifeguard_derive::DeriveTryIntoModel;
use lifeguard::{TryIntoModel, ModelTrait, LifeModelTrait, LifeEntityName};

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

// Conversion function
fn convert_to_uppercase(s: String) -> Result<String, lifeguard::LifeError> {
    Ok(s.to_uppercase())
}

// This should work: attributes split across multiple #[lifeguard] blocks
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel")]
struct CreateUserRequest {
    #[lifeguard(map_from = "name")]
    #[lifeguard(convert = "convert_to_uppercase")]
    user_name: String,
    
    email: String,
}

fn main() {
    let request = CreateUserRequest {
        user_name: "john".to_string(),
        email: "john@example.com".to_string(),
    };
    
    let model: Result<UserModel, lifeguard::LifeError> = request.try_into_model();
    match model {
        Ok(user) => {
            // user_name should be mapped to name and converted to uppercase
            assert_eq!(user.name, "JOHN");
            assert_eq!(user.email, "john@example.com");
        }
        Err(e) => {
            panic!("Failed to convert request to model: {}", e);
        }
    }
}
