//! Tests for DerivePartialModel macro

use lifeguard_derive::DerivePartialModel;
use lifeguard::PartialModelTrait;
use lifeguard::{LifeModelTrait, LifeEntityName};

// Test entity for partial models (manually defined, similar to DeriveRelation tests)
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

pub struct UserModel;

#[derive(Copy, Clone, Debug)]
pub enum UserColumn {
    Id,
    Name,
    Email,
    Age,
}

impl sea_query::Iden for UserColumn {
    fn unquoted(&self) -> &str {
        match self {
            UserColumn::Id => "id",
            UserColumn::Name => "name",
            UserColumn::Email => "email",
            UserColumn::Age => "age",
        }
    }
}

impl sea_query::IdenStatic for UserColumn {
    fn as_str(&self) -> &'static str {
        match self {
            UserColumn::Id => "id",
            UserColumn::Name => "name",
            UserColumn::Email => "email",
            UserColumn::Age => "age",
        }
    }
}

#[test]
fn test_derive_partial_model_basic() {
    #[derive(DerivePartialModel)]
    #[lifeguard(entity = "UserEntity")]
    pub struct UserPartial {
        pub id: i32,
        pub name: String,
    }
    
    // Verify PartialModelTrait is implemented
    let columns = UserPartial::selected_columns();
    assert_eq!(columns, vec!["id", "name"]);
    
    // Verify Entity type is correct
    fn _test_entity_type<P: PartialModelTrait<Entity = UserEntity>>() {}
    _test_entity_type::<UserPartial>();
}

#[test]
fn test_derive_partial_model_with_column_name() {
    #[derive(DerivePartialModel)]
    #[lifeguard(entity = "UserEntity")]
    pub struct UserPartial {
        pub id: i32,
        #[column_name = "full_name"]
        pub name: String,
    }
    
    // Verify column names use custom column_name attribute
    let columns = UserPartial::selected_columns();
    assert_eq!(columns, vec!["id", "full_name"]);
}

#[test]
fn test_derive_partial_model_single_column() {
    #[derive(DerivePartialModel)]
    #[lifeguard(entity = "UserEntity")]
    pub struct UserIdOnly {
        pub id: i32,
    }
    
    let columns = UserIdOnly::selected_columns();
    assert_eq!(columns, vec!["id"]);
}

#[test]
fn test_derive_partial_model_from_row() {
    use lifeguard::FromRow;
    
    #[derive(DerivePartialModel, Debug, PartialEq)]
    #[lifeguard(entity = "UserEntity")]
    pub struct UserPartial {
        pub id: i32,
        pub name: String,
    }
    
    // Create a mock row (this is a simplified test - actual FromRow would use real database rows)
    // Note: This test verifies the macro generates FromRow, but we can't easily test
    // the actual row parsing without a database connection
    fn _test_from_row<P: FromRow>() {}
    _test_from_row::<UserPartial>();
}

#[test]
fn test_derive_partial_model_field_name_to_snake_case() {
    #[derive(DerivePartialModel)]
    #[lifeguard(entity = "UserEntity")]
    pub struct UserPartial {
        pub user_id: i32,
        pub full_name: String,
    }
    
    // Verify field names are converted to snake_case for column names
    let columns = UserPartial::selected_columns();
    assert_eq!(columns, vec!["user_id", "full_name"]);
}
