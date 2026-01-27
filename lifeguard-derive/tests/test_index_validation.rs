//! Comprehensive tests for index validation in LifeModel macro
//!
//! Tests cover:
//! - Positive cases: Valid indexes on existing columns should compile successfully
//! - Negative cases: Invalid indexes on non-existent columns should fail to compile
//! - Edge cases: Multiple indexes, composite indexes, indexes with WHERE clauses
//! - Composite unique constraints validation
//!
//! This test suite ensures that the bug where indexes were created on non-existent
//! columns (e.g., parent table columns on child entities) is fixed.

use lifeguard_derive::LifeModel;
use lifeguard::LifeModelTrait;

// ============================================================================
// Positive Test Cases: Valid Indexes
// ============================================================================

#[test]
fn test_valid_single_column_index() {
    #[derive(LifeModel)]
    #[table_name = "test_valid_index"]
    #[index = "idx_test_valid_index_name(name)"]
    pub struct TestValidIndex {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub email: String,
    }
    
    // Should compile successfully - verify table definition includes the index
    let entity = Entity::default();
    let table_def = Entity::table_definition();
    assert_eq!(table_def.indexes.len(), 1);
    assert_eq!(table_def.indexes[0].name, "idx_test_valid_index_name");
    assert_eq!(table_def.indexes[0].columns, vec!["name"]);
}

#[test]
fn test_valid_multi_column_index() {
    #[derive(LifeModel)]
    #[table_name = "test_valid_multi_index"]
    #[index = "idx_test_multi_name_email(name, email)"]
    pub struct TestValidMultiIndex {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub email: String,
        pub age: i32,
    }
    
    // Should compile successfully
    let table_def = Entity::table_definition();
    assert_eq!(table_def.indexes.len(), 1);
    assert_eq!(table_def.indexes[0].columns, vec!["name", "email"]);
}

#[test]
fn test_valid_multiple_indexes() {
    #[derive(LifeModel)]
    #[table_name = "test_valid_multiple_indexes"]
    #[index = "idx_test_name(name)"]
    #[index = "idx_test_email(email)"]
    #[index = "idx_test_age(age)"]
    pub struct TestValidMultipleIndexes {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub email: String,
        pub age: i32,
    }
    
    // Should compile successfully with all three indexes
    let table_def = Entity::table_definition();
    assert_eq!(table_def.indexes.len(), 3);
    
    let index_names: Vec<&str> = table_def.indexes.iter().map(|i| i.name.as_str()).collect();
    assert!(index_names.contains(&"idx_test_name"));
    assert!(index_names.contains(&"idx_test_email"));
    assert!(index_names.contains(&"idx_test_age"));
}

#[test]
fn test_valid_index_with_where_clause() {
    #[derive(LifeModel)]
    #[table_name = "test_valid_index_where"]
    #[index = "idx_test_active_name(name) WHERE active = true"]
    pub struct TestValidIndexWhere {
        #[primary_key]
        pub id: i32,
        pub name: String,
        pub active: bool,
    }
    
    // Should compile successfully
    let table_def = Entity::table_definition();
    assert_eq!(table_def.indexes.len(), 1);
    assert_eq!(table_def.indexes[0].partial_where, Some("active = true".to_string()));
}

#[test]
fn test_valid_unique_index() {
    #[derive(LifeModel)]
    #[table_name = "test_valid_unique_index"]
    #[index = "UNIQUE idx_test_unique_email(email)"]
    pub struct TestValidUniqueIndex {
        #[primary_key]
        pub id: i32,
        pub email: String,
    }
    
    // Should compile successfully
    let table_def = Entity::table_definition();
    assert_eq!(table_def.indexes.len(), 1);
    assert!(table_def.indexes[0].unique);
}

#[test]
fn test_valid_composite_unique_constraint() {
    #[derive(LifeModel)]
    #[table_name = "test_valid_composite_unique"]
    #[composite_unique = "tenant_id, user_id"]
    pub struct TestValidCompositeUnique {
        #[primary_key]
        pub id: i32,
        pub tenant_id: i32,
        pub user_id: i32,
        pub name: String,
    }
    
    // Should compile successfully
    let table_def = Entity::table_definition();
    assert_eq!(table_def.composite_unique.len(), 1);
    assert_eq!(table_def.composite_unique[0], vec!["tenant_id", "user_id"]);
}

#[test]
fn test_valid_index_on_foreign_key_column() {
    #[derive(LifeModel)]
    #[table_name = "test_valid_fk_index"]
    #[index = "idx_test_user_id(user_id)"]
    pub struct TestValidFkIndex {
        #[primary_key]
        pub id: i32,
        #[foreign_key = "users(id) ON DELETE CASCADE"]
        pub user_id: i32,
        pub name: String,
    }
    
    // Should compile successfully - indexing foreign keys is common
    let table_def = Entity::table_definition();
    assert_eq!(table_def.indexes.len(), 1);
    assert_eq!(table_def.indexes[0].columns, vec!["user_id"]);
}

#[test]
fn test_valid_index_with_column_name_attribute() {
    #[derive(LifeModel)]
    #[table_name = "test_valid_index_renamed"]
    #[index = "idx_test_email_addr(email_address)"]
    pub struct TestValidIndexRenamed {
        #[primary_key]
        pub id: i32,
        #[column_name = "email_address"]
        pub email: String,
    }
    
    // Should compile successfully - index uses the column_name, not field name
    let table_def = Entity::table_definition();
    assert_eq!(table_def.indexes.len(), 1);
    assert_eq!(table_def.indexes[0].columns, vec!["email_address"]);
}

#[test]
fn test_valid_index_on_child_entity_own_columns() {
    // This simulates the CustomerInvoice/VendorInvoice pattern
    // Child entities should only index their own columns, not parent table columns
    #[derive(LifeModel)]
    #[table_name = "child_invoices"]
    #[index = "idx_child_invoices_customer_id(customer_id)"]
    pub struct ChildInvoice {
        #[primary_key]
        pub id: i32,
        #[foreign_key = "invoices(id) ON DELETE CASCADE"]
        pub invoice_id: i32,
        #[foreign_key = "customers(id) ON DELETE RESTRICT"]
        pub customer_id: i32,
        pub outstanding_amount: i64,
    }
    
    // Should compile successfully - only indexing own columns
    let table_def = Entity::table_definition();
    assert_eq!(table_def.indexes.len(), 1);
    assert_eq!(table_def.indexes[0].columns, vec!["customer_id"]);
}

// ============================================================================
// Edge Cases: Complex Scenarios
// ============================================================================

#[test]
fn test_valid_index_with_all_column_types() {
    #[derive(LifeModel)]
    #[table_name = "test_all_types_index"]
    #[index = "idx_test_string(string_col)"]
    #[index = "idx_test_int(int_col)"]
    #[index = "idx_test_bool(bool_col)"]
    #[index = "idx_test_option(option_col)"]
    pub struct TestAllTypesIndex {
        #[primary_key]
        pub id: i32,
        pub string_col: String,
        pub int_col: i32,
        pub bool_col: bool,
        pub option_col: Option<String>,
    }
    
    // Should compile successfully
    let table_def = Entity::table_definition();
    assert_eq!(table_def.indexes.len(), 4);
}

#[test]
fn test_valid_index_with_snake_case_columns() {
    #[derive(LifeModel)]
    #[table_name = "test_snake_case_index"]
    #[index = "idx_test_first_name(first_name)"]
    #[index = "idx_test_last_name(last_name)"]
    pub struct TestSnakeCaseIndex {
        #[primary_key]
        pub id: i32,
        pub first_name: String,
        pub last_name: String,
    }
    
    // Should compile successfully
    let table_def = Entity::table_definition();
    assert_eq!(table_def.indexes.len(), 2);
}

#[test]
fn test_valid_composite_unique_with_multiple_constraints() {
    #[derive(LifeModel)]
    #[table_name = "test_multiple_composite_unique"]
    #[composite_unique = "tenant_id, user_id"]
    #[composite_unique = "organization_id, project_id"]
    pub struct TestMultipleCompositeUnique {
        #[primary_key]
        pub id: i32,
        pub tenant_id: i32,
        pub user_id: i32,
        pub organization_id: i32,
        pub project_id: i32,
    }
    
    // Should compile successfully
    let table_def = Entity::table_definition();
    assert_eq!(table_def.composite_unique.len(), 2);
}

// ============================================================================
// Note: Negative test cases (compile errors) are in tests/ui/
// ============================================================================
// See:
// - test_compile_error_index_nonexistent_column.rs
// - test_compile_error_index_parent_table_column.rs
// - test_compile_error_composite_unique_nonexistent_column.rs
// - test_compile_error_index_multiple_nonexistent_columns.rs
