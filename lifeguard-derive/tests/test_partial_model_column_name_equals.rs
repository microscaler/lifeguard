//! Test that DerivePartialModel correctly parses #[column_name = "value"] syntax
//!
//! This test verifies that the standard equals sign syntax works correctly,
//! matching the behavior of the LifeModel macro and extract_column_name().

use lifeguard_derive::DerivePartialModel;
use lifeguard::PartialModelTrait;
use lifeguard::{LifeModelTrait, LifeEntityName};

// Test entity for partial models
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
lifeguard::impl_column_def_helper_for_test!(UserColumn);

// Test partial model with column_name attribute using equals sign syntax
#[derive(DerivePartialModel)]
#[lifeguard(entity = "UserEntity")]
pub struct UserPartialWithCustomColumn {
    pub id: i32,
    #[column_name = "full_name"]  // Standard syntax with equals sign
    pub name: String,
}

#[test]
fn test_column_name_equals_syntax() {
    // Verify that the column_name attribute with equals sign syntax is correctly parsed
    let columns = UserPartialWithCustomColumn::selected_columns();
    
    // The column name should be "full_name" (from the attribute), not "name" (from the field)
    assert_eq!(columns, vec!["id", "full_name"]);
    
    // Verify Entity type is correct
    fn _test_entity_type<P: PartialModelTrait<Entity = UserEntity>>() {}
    _test_entity_type::<UserPartialWithCustomColumn>();
}

// Test partial model without column_name attribute (should use snake_case)
#[derive(DerivePartialModel)]
#[lifeguard(entity = "UserEntity")]
pub struct UserPartialDefault {
    pub id: i32,
    pub name: String,  // No column_name attribute - should use "name"
}

#[test]
fn test_column_name_default_snake_case() {
    // Verify that without column_name attribute, field name is used (already snake_case)
    let columns = UserPartialDefault::selected_columns();
    assert_eq!(columns, vec!["id", "name"]);
}

// Test partial model with camelCase field name (should convert to snake_case)
#[derive(DerivePartialModel)]
#[lifeguard(entity = "UserEntity")]
pub struct UserPartialCamelCase {
    pub id: i32,
    pub fullName: String,  // camelCase - should convert to "full_name"
}

#[test]
fn test_column_name_camel_case_conversion() {
    // Verify that camelCase field names are converted to snake_case
    let columns = UserPartialCamelCase::selected_columns();
    assert_eq!(columns, vec!["id", "full_name"]);
}
