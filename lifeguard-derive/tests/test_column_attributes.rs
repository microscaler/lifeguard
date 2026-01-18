//! Comprehensive tests for column attributes in LifeModel macro
//!
//! Tests cover:
//! - Individual attributes (auto_increment, column_type, default_value, unique, indexed, enum_name, nullable)
//! - Combinations of multiple attributes
//! - Edge cases (Option<T>, nullable with Option<T>, etc.)
//! - Negative cases (invalid values, missing attributes)

use lifeguard_derive::LifeModel;
use lifeguard::ColumnDefinition;

// ============================================================================
// Positive Test Cases: Individual Attributes
// ============================================================================

#[test]
fn test_auto_increment_attribute() {
    #[derive(LifeModel)]
    #[table_name = "test_auto_increment"]
    pub struct TestAutoIncrement {
        #[primary_key]
        #[auto_increment]
        pub id: i32,
        pub name: String,
    }
    
    // Verify auto_increment is set in column_def()
    let def = TestAutoIncrement::Column::Id.column_def();
    assert_eq!(def.auto_increment, true);
    assert_eq!(def.unique, false);
    assert_eq!(def.indexed, false);
    
    // Verify non-auto-increment column
    let def = TestAutoIncrement::Column::Name.column_def();
    assert_eq!(def.auto_increment, false);
}

#[test]
fn test_column_type_attribute() {
    #[derive(LifeModel)]
    #[table_name = "test_column_type"]
    pub struct TestColumnType {
        #[primary_key]
        pub id: i32,
        #[column_type = "VARCHAR(255)"]
        pub name: String,
        #[column_type = "BIGINT"]
        pub count: i64,
    }
    
    // Verify column_type is set
    let def = TestColumnType::Column::Name.column_def();
    assert_eq!(def.column_type, Some("VARCHAR(255)".to_string()));
    
    let def = TestColumnType::Column::Count.column_def();
    assert_eq!(def.column_type, Some("BIGINT".to_string()));
    
    // Verify default (no column_type attribute)
    let def = TestColumnType::Column::Id.column_def();
    assert_eq!(def.column_type, None);
}

#[test]
fn test_default_value_attribute() {
    #[derive(LifeModel)]
    #[table_name = "test_default_value"]
    pub struct TestDefaultValue {
        #[primary_key]
        pub id: i32,
        #[default_value = "''"]
        pub name: String,
        #[default_value = "0"]
        pub count: i32,
        #[default_value = "NOW()"]
        pub created_at: String,
    }
    
    // Verify default_value is set
    let def = TestDefaultValue::Column::Name.column_def();
    assert_eq!(def.default_value, Some("''".to_string()));
    
    let def = TestDefaultValue::Column::Count.column_def();
    assert_eq!(def.default_value, Some("0".to_string()));
    
    let def = TestDefaultValue::Column::CreatedAt.column_def();
    assert_eq!(def.default_value, Some("NOW()".to_string()));
    
    // Verify no default_value
    let def = TestDefaultValue::Column::Id.column_def();
    assert_eq!(def.default_value, None);
}

#[test]
fn test_unique_attribute() {
    #[derive(LifeModel)]
    #[table_name = "test_unique"]
    pub struct TestUnique {
        #[primary_key]
        pub id: i32,
        #[unique]
        pub email: String,
        pub name: String,
    }
    
    // Verify unique is set
    let def = TestUnique::Column::Email.column_def();
    assert_eq!(def.unique, true);
    assert_eq!(def.indexed, false);
    
    // Verify non-unique column
    let def = TestUnique::Column::Name.column_def();
    assert_eq!(def.unique, false);
}

#[test]
fn test_indexed_attribute() {
    #[derive(LifeModel)]
    #[table_name = "test_indexed"]
    pub struct TestIndexed {
        #[primary_key]
        pub id: i32,
        #[indexed]
        pub username: String,
        pub name: String,
    }
    
    // Verify indexed is set
    let def = TestIndexed::Column::Username.column_def();
    assert_eq!(def.indexed, true);
    assert_eq!(def.unique, false);
    
    // Verify non-indexed column
    let def = TestIndexed::Column::Name.column_def();
    assert_eq!(def.indexed, false);
}

#[test]
fn test_enum_name_attribute() {
    #[derive(LifeModel)]
    #[table_name = "test_enum_name"]
    pub struct TestEnumName {
        #[primary_key]
        pub id: i32,
        #[enum_name = "user_status_enum"]
        pub status: String,
        pub name: String,
    }
    
    // Verify enum_type_name returns the enum name
    let enum_name = TestEnumName::Column::Status.column_enum_type_name();
    assert_eq!(enum_name, Some("user_status_enum".to_string()));
    
    // Verify non-enum column returns None
    let enum_name = TestEnumName::Column::Name.column_enum_type_name();
    assert_eq!(enum_name, None);
}

#[test]
fn test_nullable_attribute() {
    #[derive(LifeModel)]
    #[table_name = "test_nullable"]
    pub struct TestNullable {
        #[primary_key]
        pub id: i32,
        #[nullable]
        pub description: String,
        pub name: String,
    }
    
    // Verify nullable is set
    let def = TestNullable::Column::Description.column_def();
    assert_eq!(def.nullable, true);
    
    // Verify non-nullable column
    let def = TestNullable::Column::Name.column_def();
    assert_eq!(def.nullable, false);
}

// ============================================================================
// Edge Cases: Option<T> Types
// ============================================================================

#[test]
fn test_option_type_automatically_nullable() {
    #[derive(LifeModel)]
    #[table_name = "test_option_nullable"]
    pub struct TestOptionNullable {
        #[primary_key]
        pub id: i32,
        pub name: Option<String>,
        pub age: Option<i32>,
    }
    
    // Option<T> should automatically be nullable
    let def = TestOptionNullable::Column::Name.column_def();
    assert_eq!(def.nullable, true);
    
    let def = TestOptionNullable::Column::Age.column_def();
    assert_eq!(def.nullable, true);
}

#[test]
fn test_option_type_with_nullable_attribute() {
    #[derive(LifeModel)]
    #[table_name = "test_option_with_nullable"]
    pub struct TestOptionWithNullable {
        #[primary_key]
        pub id: i32,
        #[nullable]
        pub name: Option<String>,
    }
    
    // Option<T> with #[nullable] should still be nullable
    let def = TestOptionWithNullable::Column::Name.column_def();
    assert_eq!(def.nullable, true);
}

#[test]
fn test_non_option_with_nullable_attribute() {
    #[derive(LifeModel)]
    #[table_name = "test_non_option_nullable"]
    pub struct TestNonOptionNullable {
        #[primary_key]
        pub id: i32,
        #[nullable]
        pub name: String,
    }
    
    // Non-Option<T> with #[nullable] should be nullable
    let def = TestNonOptionNullable::Column::Name.column_def();
    assert_eq!(def.nullable, true);
}

// ============================================================================
// Combinations: Multiple Attributes
// ============================================================================

#[test]
fn test_auto_increment_with_unique() {
    #[derive(LifeModel)]
    #[table_name = "test_auto_increment_unique"]
    pub struct TestAutoIncrementUnique {
        #[primary_key]
        #[auto_increment]
        #[unique]
        pub id: i32,
    }
    
    let def = TestAutoIncrementUnique::Column::Id.column_def();
    assert_eq!(def.auto_increment, true);
    assert_eq!(def.unique, true);
}

#[test]
fn test_unique_with_indexed() {
    #[derive(LifeModel)]
    #[table_name = "test_unique_indexed"]
    pub struct TestUniqueIndexed {
        #[primary_key]
        pub id: i32,
        #[unique]
        #[indexed]
        pub email: String,
    }
    
    let def = TestUniqueIndexed::Column::Email.column_def();
    assert_eq!(def.unique, true);
    assert_eq!(def.indexed, true);
}

#[test]
fn test_column_type_with_default_value() {
    #[derive(LifeModel)]
    #[table_name = "test_column_type_default"]
    pub struct TestColumnTypeDefault {
        #[primary_key]
        pub id: i32,
        #[column_type = "VARCHAR(100)"]
        #[default_value = "''"]
        pub name: String,
    }
    
    let def = TestColumnTypeDefault::Column::Name.column_def();
    assert_eq!(def.column_type, Some("VARCHAR(100)".to_string()));
    assert_eq!(def.default_value, Some("''".to_string()));
}

#[test]
fn test_all_attributes_combined() {
    #[derive(LifeModel)]
    #[table_name = "test_all_attributes"]
    pub struct TestAllAttributes {
        #[primary_key]
        pub id: i32,
        #[column_type = "VARCHAR(255)"]
        #[default_value = "''"]
        #[unique]
        #[indexed]
        #[nullable]
        #[enum_name = "user_role_enum"]
        pub role: String,
    }
    
    let def = TestAllAttributes::Column::Role.column_def();
    assert_eq!(def.column_type, Some("VARCHAR(255)".to_string()));
    assert_eq!(def.default_value, Some("''".to_string()));
    assert_eq!(def.unique, true);
    assert_eq!(def.indexed, true);
    assert_eq!(def.nullable, true);
    assert_eq!(def.auto_increment, false);
    
    let enum_name = TestAllAttributes::Column::Role.column_enum_type_name();
    assert_eq!(enum_name, Some("user_role_enum".to_string()));
}

#[test]
fn test_primary_key_with_auto_increment() {
    #[derive(LifeModel)]
    #[table_name = "test_pk_auto_increment"]
    pub struct TestPkAutoIncrement {
        #[primary_key]
        #[auto_increment]
        pub id: i32,
        pub name: String,
    }
    
    let def = TestPkAutoIncrement::Column::Id.column_def();
    assert_eq!(def.auto_increment, true);
}

// ============================================================================
// Edge Cases: Special Values
// ============================================================================

#[test]
fn test_empty_string_default_value() {
    #[derive(LifeModel)]
    #[table_name = "test_empty_default"]
    pub struct TestEmptyDefault {
        #[primary_key]
        pub id: i32,
        #[default_value = ""]
        pub name: String,
    }
    
    let def = TestEmptyDefault::Column::Name.column_def();
    assert_eq!(def.default_value, Some("".to_string()));
}

#[test]
fn test_sql_function_default_value() {
    #[derive(LifeModel)]
    #[table_name = "test_sql_default"]
    pub struct TestSqlDefault {
        #[primary_key]
        pub id: i32,
        #[default_value = "CURRENT_TIMESTAMP"]
        pub created_at: String,
        #[default_value = "uuid_generate_v4()"]
        pub uuid: String,
    }
    
    let def = TestSqlDefault::Column::CreatedAt.column_def();
    assert_eq!(def.default_value, Some("CURRENT_TIMESTAMP".to_string()));
    
    let def = TestSqlDefault::Column::Uuid.column_def();
    assert_eq!(def.default_value, Some("uuid_generate_v4()".to_string()));
}

#[test]
fn test_long_enum_name() {
    #[derive(LifeModel)]
    #[table_name = "test_long_enum"]
    pub struct TestLongEnum {
        #[primary_key]
        pub id: i32,
        #[enum_name = "very_long_enum_name_that_might_be_used_in_postgresql_schema"]
        pub status: String,
    }
    
    let enum_name = TestLongEnum::Column::Status.column_enum_type_name();
    assert_eq!(enum_name, Some("very_long_enum_name_that_might_be_used_in_postgresql_schema".to_string()));
}

#[test]
fn test_special_characters_in_column_type() {
    #[derive(LifeModel)]
    #[table_name = "test_special_chars"]
    pub struct TestSpecialChars {
        #[primary_key]
        pub id: i32,
        #[column_type = "VARCHAR(255) COLLATE \"C\""]
        pub name: String,
    }
    
    let def = TestSpecialChars::Column::Name.column_def();
    assert_eq!(def.column_type, Some("VARCHAR(255) COLLATE \"C\"".to_string()));
}

// ============================================================================
// Edge Cases: Multiple Fields with Different Attributes
// ============================================================================

#[test]
fn test_multiple_fields_different_attributes() {
    #[derive(LifeModel)]
    #[table_name = "test_multiple_fields"]
    pub struct TestMultipleFields {
        #[primary_key]
        #[auto_increment]
        pub id: i32,
        #[unique]
        pub email: String,
        #[indexed]
        pub username: String,
        #[nullable]
        pub description: Option<String>,
        #[default_value = "0"]
        pub count: i32,
        #[enum_name = "status_enum"]
        pub status: String,
        #[column_type = "TEXT"]
        pub bio: String,
    }
    
    // Verify each field has correct attributes
    let def_id = TestMultipleFields::Column::Id.column_def();
    assert_eq!(def_id.auto_increment, true);
    
    let def_email = TestMultipleFields::Column::Email.column_def();
    assert_eq!(def_email.unique, true);
    assert_eq!(def_email.indexed, false);
    
    let def_username = TestMultipleFields::Column::Username.column_def();
    assert_eq!(def_username.indexed, true);
    assert_eq!(def_username.unique, false);
    
    let def_description = TestMultipleFields::Column::Description.column_def();
    assert_eq!(def_description.nullable, true);
    
    let def_count = TestMultipleFields::Column::Count.column_def();
    assert_eq!(def_count.default_value, Some("0".to_string()));
    
    let enum_name = TestMultipleFields::Column::Status.column_enum_type_name();
    assert_eq!(enum_name, Some("status_enum".to_string()));
    
    let def_bio = TestMultipleFields::Column::Bio.column_def();
    assert_eq!(def_bio.column_type, Some("TEXT".to_string()));
}

// ============================================================================
// Edge Cases: Default Values (No Attributes)
// ============================================================================

#[test]
fn test_default_column_definition() {
    #[derive(LifeModel)]
    #[table_name = "test_defaults"]
    pub struct TestDefaults {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub age: i32,
    }
    
    // Verify default values when no attributes are specified
    let def_id = TestDefaults::Column::Id.column_def();
    assert_eq!(def_id.column_type, None);
    assert_eq!(def_id.nullable, false);
    assert_eq!(def_id.default_value, None);
    assert_eq!(def_id.unique, false);
    assert_eq!(def_id.indexed, false);
    assert_eq!(def_id.auto_increment, false);
    
    let def_name = TestDefaults::Column::Name.column_def();
    assert_eq!(def_name.column_type, None);
    assert_eq!(def_name.nullable, false);
    
    let enum_name = TestDefaults::Column::Name.column_enum_type_name();
    assert_eq!(enum_name, None);
}

// ============================================================================
// Edge Cases: Composite Primary Keys
// ============================================================================

#[test]
fn test_composite_primary_key_with_attributes() {
    #[derive(LifeModel)]
    #[table_name = "test_composite_pk"]
    pub struct TestCompositePk {
        #[primary_key]
        #[auto_increment]
        pub id: i32,
        #[primary_key]
        #[unique]
        pub tenant_id: i32,
        pub name: String,
    }
    
    // Both primary key fields should have their attributes
    let def_id = TestCompositePk::Column::Id.column_def();
    assert_eq!(def_id.auto_increment, true);
    
    let def_tenant_id = TestCompositePk::Column::TenantId.column_def();
    assert_eq!(def_tenant_id.unique, true);
}

// ============================================================================
// Edge Cases: Numeric Types with Attributes
// ============================================================================

#[test]
fn test_numeric_types_with_attributes() {
    #[derive(LifeModel)]
    #[table_name = "test_numeric"]
    pub struct TestNumeric {
        #[primary_key]
        pub id: i32,
        #[column_type = "BIGINT"]
        #[default_value = "0"]
        pub count: i64,
        #[column_type = "DECIMAL(10,2)"]
        #[default_value = "0.00"]
        pub price: f64,
    }
    
    let def_count = TestNumeric::Column::Count.column_def();
    assert_eq!(def_count.column_type, Some("BIGINT".to_string()));
    assert_eq!(def_count.default_value, Some("0".to_string()));
    
    let def_price = TestNumeric::Column::Price.column_def();
    assert_eq!(def_price.column_type, Some("DECIMAL(10,2)".to_string()));
    assert_eq!(def_price.default_value, Some("0.00".to_string()));
}

// ============================================================================
// Edge Cases: Boolean Types
// ============================================================================

#[test]
fn test_boolean_with_default() {
    #[derive(LifeModel)]
    #[table_name = "test_boolean"]
    pub struct TestBoolean {
        #[primary_key]
        pub id: i32,
        #[default_value = "false"]
        pub active: bool,
        #[default_value = "true"]
        pub verified: bool,
    }
    
    let def_active = TestBoolean::Column::Active.column_def();
    assert_eq!(def_active.default_value, Some("false".to_string()));
    
    let def_verified = TestBoolean::Column::Verified.column_def();
    assert_eq!(def_verified.default_value, Some("true".to_string()));
}

// ============================================================================
// Edge Cases: JSON Types
// ============================================================================

#[test]
fn test_json_type_with_attributes() {
    #[derive(LifeModel)]
    #[table_name = "test_json"]
    pub struct TestJson {
        #[primary_key]
        pub id: i32,
        #[column_type = "JSONB"]
        #[nullable]
        pub metadata: Option<serde_json::Value>,
    }
    
    let def = TestJson::Column::Metadata.column_def();
    assert_eq!(def.column_type, Some("JSONB".to_string()));
    assert_eq!(def.nullable, true);
}
