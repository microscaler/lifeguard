//! Unit tests for column attribute parsing (doesn't require LifeModel macro expansion)
//!
//! These tests verify that the `parse_column_attributes()` function correctly
//! extracts all column attributes from field definitions. This allows us to test
//! the attribute parsing logic without triggering the E0223 macro expansion issues.

// Note: We can't directly import attributes module as it's not public.
// Instead, we'll test the attribute parsing indirectly by checking that
// the macro correctly uses parse_column_attributes().
// For now, we'll skip these unit tests and rely on the integration tests
// in test_column_attributes.rs once E0223 is fixed.

// TODO: Make attributes module public or create a test helper that exposes
// parse_column_attributes for testing purposes.

// For now, this file documents the test cases that should be verified
// once the E0223 issue is resolved.

#[test]
fn test_parse_primary_key() {
    let field: Field = parse_quote! {
        #[primary_key]
        pub id: i32
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.is_primary_key, true);
    assert_eq!(attrs.is_auto_increment, false);
}

#[test]
fn test_parse_auto_increment() {
    let field: Field = parse_quote! {
        #[auto_increment]
        pub id: i32
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.is_auto_increment, true);
    assert_eq!(attrs.is_primary_key, false);
}

#[test]
fn test_parse_column_type() {
    let field: Field = parse_quote! {
        #[column_type = "VARCHAR(255)"]
        pub name: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.column_type, Some("VARCHAR(255)".to_string()));
}

#[test]
fn test_parse_default_value() {
    let field: Field = parse_quote! {
        #[default_value = "''"]
        pub name: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.default_value, Some("''".to_string()));
}

#[test]
fn test_parse_unique() {
    let field: Field = parse_quote! {
        #[unique]
        pub email: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.is_unique, true);
    assert_eq!(attrs.is_indexed, false);
}

#[test]
fn test_parse_indexed() {
    let field: Field = parse_quote! {
        #[indexed]
        pub username: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.is_indexed, true);
    assert_eq!(attrs.is_unique, false);
}

#[test]
fn test_parse_nullable() {
    let field: Field = parse_quote! {
        #[nullable]
        pub description: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.is_nullable, true);
}

#[test]
fn test_parse_enum_name() {
    let field: Field = parse_quote! {
        #[enum_name = "user_status_enum"]
        pub status: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.enum_name, Some("user_status_enum".to_string()));
}

#[test]
fn test_parse_column_name() {
    let field: Field = parse_quote! {
        #[column_name = "full_name"]
        pub firstName: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.column_name, Some("full_name".to_string()));
}

#[test]
fn test_parse_multiple_attributes() {
    let field: Field = parse_quote! {
        #[column_type = "VARCHAR(255)"]
        #[default_value = "''"]
        #[unique]
        #[indexed]
        #[nullable]
        #[enum_name = "user_role_enum"]
        pub role: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.column_type, Some("VARCHAR(255)".to_string()));
    assert_eq!(attrs.default_value, Some("''".to_string()));
    assert_eq!(attrs.is_unique, true);
    assert_eq!(attrs.is_indexed, true);
    assert_eq!(attrs.is_nullable, true);
    assert_eq!(attrs.enum_name, Some("user_role_enum".to_string()));
}

#[test]
fn test_parse_no_attributes() {
    let field: Field = parse_quote! {
        pub name: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.is_primary_key, false);
    assert_eq!(attrs.is_auto_increment, false);
    assert_eq!(attrs.column_type, None);
    assert_eq!(attrs.default_value, None);
    assert_eq!(attrs.is_unique, false);
    assert_eq!(attrs.is_indexed, false);
    assert_eq!(attrs.is_nullable, false);
    assert_eq!(attrs.enum_name, None);
}

#[test]
fn test_parse_primary_key_with_auto_increment() {
    let field: Field = parse_quote! {
        #[primary_key]
        #[auto_increment]
        pub id: i32
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.is_primary_key, true);
    assert_eq!(attrs.is_auto_increment, true);
}

#[test]
fn test_parse_unique_with_indexed() {
    let field: Field = parse_quote! {
        #[unique]
        #[indexed]
        pub email: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.is_unique, true);
    assert_eq!(attrs.is_indexed, true);
}

#[test]
fn test_parse_sql_function_default() {
    let field: Field = parse_quote! {
        #[default_value = "NOW()"]
        pub created_at: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.default_value, Some("NOW()".to_string()));
}

#[test]
fn test_parse_empty_string_default() {
    let field: Field = parse_quote! {
        #[default_value = ""]
        pub name: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.default_value, Some("".to_string()));
}

#[test]
fn test_parse_long_enum_name() {
    let field: Field = parse_quote! {
        #[enum_name = "very_long_enum_name_that_might_be_used_in_postgresql_schema"]
        pub status: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.enum_name, Some("very_long_enum_name_that_might_be_used_in_postgresql_schema".to_string()));
}

#[test]
fn test_parse_special_characters_in_column_type() {
    let field: Field = parse_quote! {
        #[column_type = "VARCHAR(255) COLLATE \"C\""]
        pub name: String
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.column_type, Some("VARCHAR(255) COLLATE \"C\"".to_string()));
}

#[test]
fn test_parse_numeric_column_type() {
    let field: Field = parse_quote! {
        #[column_type = "DECIMAL(10,2)"]
        pub price: f64
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.column_type, Some("DECIMAL(10,2)".to_string()));
}

#[test]
fn test_parse_boolean_default() {
    let field: Field = parse_quote! {
        #[default_value = "false"]
        pub active: bool
    };
    
    let attrs = attributes::parse_column_attributes(&field);
    assert_eq!(attrs.default_value, Some("false".to_string()));
}
