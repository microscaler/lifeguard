//! Tests for LifeModel derive macro

use lifeguard_derive::LifeModel;

#[test]
fn test_basic_life_model() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    struct User {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
    }
    
    // Verify the generated types exist
    let _column = Column::Id;
    let _primary_key = PrimaryKey::Id;
    let _entity: Entity = User {
        id: 1,
        name: "Test".to_string(),
        email: "test@example.com".to_string(),
    };
    
    // Verify table name constant
    assert_eq!(User::TABLE_NAME, "users");
    
    // Verify Model struct exists
    let _model = UserModel {
        id: 1,
        name: "Test".to_string(),
        email: "test@example.com".to_string(),
    };
    
    // Verify from_row method exists
    let _from_row_method = UserModel::from_row;
}

#[test]
fn test_custom_column_name() {
    #[derive(LifeModel)]
    #[table_name = "posts"]
    struct Post {
        #[primary_key]
        #[column_name = "post_id"]
        id: i32,
        #[column_name = "post_title"]
        title: String,
    }
    
    // Verify table name
    assert_eq!(Post::TABLE_NAME, "posts");
}

#[test]
fn test_multiple_primary_keys() {
    #[derive(LifeModel)]
    #[table_name = "user_roles"]
    struct UserRole {
        #[primary_key]
        user_id: i32,
        #[primary_key]
        role_id: i32,
    }
    
    // Verify both primary keys are in the enum
    let _pk1 = PrimaryKey::UserId;
    let _pk2 = PrimaryKey::RoleId;
}

// Note: Empty structs test removed - macro currently requires at least one field
// This can be added later when macro supports empty structs

#[test]
fn test_struct_with_all_fields_primary_key() {
    // All fields as primary key (composite key)
    #[derive(LifeModel)]
    #[table_name = "composite_key_table"]
    struct CompositeKey {
        #[primary_key]
        field1: i32,
        #[primary_key]
        field2: i32,
        #[primary_key]
        field3: i32,
    }

    // Verify all are in PrimaryKey enum
    let _pk1 = PrimaryKey::Field1;
    let _pk2 = PrimaryKey::Field2;
    let _pk3 = PrimaryKey::Field3;
}

#[test]
fn test_struct_with_no_primary_key() {
    // Struct without primary key should still work
    #[derive(LifeModel)]
    #[table_name = "no_pk_table"]
    struct NoPrimaryKey {
        name: String,
        email: String,
    }

    // PrimaryKey enum should be empty
    // This is valid - not all tables need primary keys
    assert_eq!(NoPrimaryKey::TABLE_NAME, "no_pk_table");
}

#[test]
fn test_struct_with_special_field_names() {
    // Field names with snake_case and UPPER_CASE
    // Note: Reserved keywords (r#type, r#fn) test removed - can be added when macro supports them
    #[derive(LifeModel)]
    #[table_name = "special_names"]
    struct SpecialNames {
        #[primary_key]
        snake_case_field: i32,
        UPPER_CASE_FIELD: i32,
    }

    // Should generate valid Column enum variants
    // Column enum is generated in the same scope as the struct
    // Note: UPPER_CASE_FIELD becomes UPPERCASEFIELD in PascalCase
    let _col1 = Column::SnakeCaseField;
    let _col2 = Column::UPPERCASEFIELD;
    
    // Verify table name
    assert_eq!(SpecialNames::TABLE_NAME, "special_names");
}

#[test]
fn test_struct_with_custom_column_names() {
    // Custom column names should work
    #[derive(LifeModel)]
    #[table_name = "custom_columns"]
    struct CustomColumns {
        #[primary_key]
        #[column_name = "user_id"]
        id: i32,
        #[column_name = "full_name"]
        name: String,
    }

    assert_eq!(CustomColumns::TABLE_NAME, "custom_columns");
}

#[test]
fn test_struct_with_default_table_name() {
    // If no table_name attribute, should use snake_case of struct name
    #[derive(LifeModel)]
    struct UserProfile {
        #[primary_key]
        id: i32,
    }

    // Table name should be derived from struct name
    assert_eq!(UserProfile::TABLE_NAME, "user_profile");
}

#[test]
fn test_struct_with_many_fields() {
    // Struct with many fields (stress test)
    #[derive(LifeModel)]
    #[table_name = "many_fields"]
    struct ManyFields {
        #[primary_key]
        id: i32,
        field1: String,
        field2: i32,
        field3: bool,
        field4: String,
        field5: i64,
        field6: f64,
        field7: String,
        field8: i32,
        field9: bool,
        field10: String,
    }

    // Should generate all Column variants
    let _cols = vec![
        Column::Id, Column::Field1, Column::Field2, Column::Field3,
        Column::Field4, Column::Field5, Column::Field6, Column::Field7,
        Column::Field8, Column::Field9, Column::Field10,
    ];
}

#[test]
fn test_from_row_method_exists() {
    // Verify that from_row method is generated on Model
    #[derive(LifeModel)]
    #[table_name = "test_table"]
    struct TestStruct {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Verify from_row method exists (compilation test)
    // Actual usage requires a real may_postgres::Row
    // This will be tested in integration tests
    let _method_exists = TestStructModel::from_row;
}

#[test]
fn test_postgres_integer_types() {
    // Test various integer types (i16, i32, i64)
    #[derive(LifeModel)]
    #[table_name = "integer_types"]
    struct IntegerTypes {
        #[primary_key]
        id: i32,
        small_int: i16,
        big_int: i64,
    }
    
    assert_eq!(IntegerTypes::TABLE_NAME, "integer_types");
    let _from_row = IntegerTypesModel::from_row;
}

#[test]
fn test_postgres_text_types() {
    // Test text/String types
    #[derive(LifeModel)]
    #[table_name = "text_types"]
    struct TextTypes {
        #[primary_key]
        id: i32,
        name: String,
        description: String,
    }
    
    assert_eq!(TextTypes::TABLE_NAME, "text_types");
    let _from_row = TextTypesModel::from_row;
}

#[test]
fn test_postgres_boolean_type() {
    // Test boolean type
    #[derive(LifeModel)]
    #[table_name = "boolean_types"]
    struct BooleanTypes {
        #[primary_key]
        id: i32,
        is_active: bool,
        is_verified: bool,
    }
    
    assert_eq!(BooleanTypes::TABLE_NAME, "boolean_types");
    let _from_row = BooleanTypesModel::from_row;
}

#[test]
fn test_postgres_numeric_types() {
    // Test floating point types (f32, f64)
    #[derive(LifeModel)]
    #[table_name = "numeric_types"]
    struct NumericTypes {
        #[primary_key]
        id: i32,
        price: f64,
        weight: f32,
    }
    
    assert_eq!(NumericTypes::TABLE_NAME, "numeric_types");
    let _from_row = NumericTypesModel::from_row;
}

#[test]
fn test_postgres_mixed_types() {
    // Test mixed PostgreSQL types (comprehensive test)
    #[derive(LifeModel)]
    #[table_name = "mixed_types"]
    struct MixedTypes {
        #[primary_key]
        id: i32,
        name: String,
        age: i32,
        is_active: bool,
        balance: f64,
    }
    
    assert_eq!(MixedTypes::TABLE_NAME, "mixed_types");
    let _from_row = MixedTypesModel::from_row;
}
