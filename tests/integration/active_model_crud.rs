//! Integration tests for ActiveModel CRUD operations
//!
//! These tests validate that ActiveModelTrait CRUD methods work correctly
//! with a real PostgreSQL database.
//!
//! Note: These tests require a running PostgreSQL database. Set TEST_DATABASE_URL
//! environment variable or use the test infrastructure from test_helpers.

use lifeguard::{
    ActiveModelTrait, ActiveModelError, LifeModelTrait, LifeExecutor, MayPostgresExecutor,
    test_helpers::TestDatabase,
};
use lifeguard_derive::{LifeModel, LifeRecord};

// Test entity: Simple user table
#[derive(LifeModel, LifeRecord)]
#[table_name = "test_users"]
pub struct TestUser {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub age: Option<i32>,
}

// Helper function to set up test database schema
fn setup_test_schema(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER
        )
        "#,
        &[],
    )?;
    Ok(())
}

// Helper function to clean up test data
fn cleanup_test_data(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute("DELETE FROM test_users", &[])?;
    Ok(())
}

// Helper to query database and get count
fn query_count(executor: &MayPostgresExecutor, query: &str, params: &[&dyn may_postgres::types::ToSql]) -> Result<i64, lifeguard::executor::LifeError> {
    let rows = executor.query_all(query, params)?;
    if rows.is_empty() {
        Ok(0)
    } else {
        Ok(rows[0].get::<_, i64>(0))
    }
}

#[test]
fn test_active_model_insert() {
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a new record
    let mut record = TestUserRecord::new();
    record.set_name("John Doe".to_string());
    record.set_email("john@example.com".to_string());
    record.set_age(Some(30));

    // Insert the record
    let model = record.insert(&executor).expect("Failed to insert record");

    // Verify the inserted model
    assert_eq!(model.name, "John Doe");
    assert_eq!(model.email, "john@example.com");
    assert_eq!(model.age, Some(30));
    assert!(model.id > 0); // Auto-increment ID should be set

    // Verify in database using executor
    let rows = executor.query_all(
        "SELECT id, name, email, age FROM test_users WHERE id = $1",
        &[&model.id],
    ).expect("Failed to query database");
    
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.get::<_, i32>(0), model.id);
    assert_eq!(row.get::<_, String>(1), "John Doe");
    assert_eq!(row.get::<_, String>(2), "john@example.com");
    assert_eq!(row.get::<_, Option<i32>>(3), Some(30));
}

#[test]
fn test_active_model_insert_without_optional_fields() {
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a record without optional fields
    let mut record = TestUserRecord::new();
    record.set_name("Jane Doe".to_string());
    record.set_email("jane@example.com".to_string());
    // age is not set (None)

    // Insert the record
    let model = record.insert(&executor).expect("Failed to insert record");

    // Verify the inserted model
    assert_eq!(model.name, "Jane Doe");
    assert_eq!(model.email, "jane@example.com");
    assert_eq!(model.age, None);

    // Verify in database
    let rows = executor.query_all(
        "SELECT age FROM test_users WHERE id = $1",
        &[&model.id],
    ).expect("Failed to query database");
    
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.get::<_, Option<i32>>(0), None);
}

#[test]
fn test_active_model_update() {
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // First, insert a record
    let mut insert_record = TestUserRecord::new();
    insert_record.set_name("Original Name".to_string());
    insert_record.set_email("original@example.com".to_string());
    let original_model = insert_record.insert(&executor).expect("Failed to insert");

    // Now update it
    let mut update_record = TestUserRecord::from_model(&original_model);
    update_record.set_name("Updated Name".to_string());
    update_record.set_email("updated@example.com".to_string());
    update_record.set_age(Some(25));

    let updated_model = update_record.update(&executor).expect("Failed to update");

    // Verify the updated model
    assert_eq!(updated_model.id, original_model.id);
    assert_eq!(updated_model.name, "Updated Name");
    assert_eq!(updated_model.email, "updated@example.com");
    assert_eq!(updated_model.age, Some(25));

    // Verify in database
    let rows = executor.query_all(
        "SELECT name, email, age FROM test_users WHERE id = $1",
        &[&original_model.id],
    ).expect("Failed to query database");
    
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.get::<_, String>(0), "Updated Name");
    assert_eq!(row.get::<_, String>(1), "updated@example.com");
    assert_eq!(row.get::<_, Option<i32>>(2), Some(25));
}

#[test]
fn test_active_model_update_requires_primary_key() {
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a record without primary key
    let mut record = TestUserRecord::new();
    record.set_name("Test".to_string());
    record.set_email("test@example.com".to_string());

    // Update should fail because primary key is not set
    let result = record.update(&executor);
    assert!(result.is_err());
    
    match result.unwrap_err() {
        ActiveModelError::PrimaryKeyRequired => {
            // Expected error
        }
        e => panic!("Expected PrimaryKeyRequired, got: {:?}", e),
    }
}

#[test]
fn test_active_model_delete() {
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Insert a record
    let mut insert_record = TestUserRecord::new();
    insert_record.set_name("To Delete".to_string());
    insert_record.set_email("delete@example.com".to_string());
    let model = insert_record.insert(&executor).expect("Failed to insert");

    // Verify it exists
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users WHERE id = $1",
        &[&model.id],
    ).expect("Failed to query database");
    assert_eq!(count, 1);

    // Delete it
    let mut delete_record = TestUserRecord::from_model(&model);
    delete_record.delete(&executor).expect("Failed to delete");

    // Verify it's gone
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users WHERE id = $1",
        &[&model.id],
    ).expect("Failed to query database");
    assert_eq!(count, 0);
}

#[test]
fn test_active_model_delete_requires_primary_key() {
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a record without primary key
    let mut record = TestUserRecord::new();
    record.set_name("Test".to_string());
    record.set_email("test@example.com".to_string());

    // Delete should fail because primary key is not set
    let result = record.delete(&executor);
    assert!(result.is_err());
    
    match result.unwrap_err() {
        ActiveModelError::PrimaryKeyRequired => {
            // Expected error
        }
        e => panic!("Expected PrimaryKeyRequired, got: {:?}", e),
    }
}

#[test]
fn test_active_model_save_inserts_when_no_primary_key() {
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a new record (no primary key)
    let mut record = TestUserRecord::new();
    record.set_name("Save Test".to_string());
    record.set_email("save@example.com".to_string());

    // save() should insert because there's no primary key
    let model = record.save(&executor).expect("Failed to save");

    // Verify it was inserted
    assert!(model.id > 0);
    assert_eq!(model.name, "Save Test");
    assert_eq!(model.email, "save@example.com");

    // Verify in database
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users WHERE id = $1",
        &[&model.id],
    ).expect("Failed to query database");
    assert_eq!(count, 1);
}

#[test]
fn test_active_model_save_updates_when_primary_key_exists() {
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // First, insert a record
    let mut insert_record = TestUserRecord::new();
    insert_record.set_name("Original".to_string());
    insert_record.set_email("original@example.com".to_string());
    let original_model = insert_record.insert(&executor).expect("Failed to insert");

    // Now use save() to update it
    let mut save_record = TestUserRecord::from_model(&original_model);
    save_record.set_name("Updated via Save".to_string());
    save_record.set_email("updated@example.com".to_string());

    let saved_model = save_record.save(&executor).expect("Failed to save");

    // Verify it was updated (same ID, new values)
    assert_eq!(saved_model.id, original_model.id);
    assert_eq!(saved_model.name, "Updated via Save");
    assert_eq!(saved_model.email, "updated@example.com");

    // Verify in database
    let rows = executor.query_all(
        "SELECT name, email FROM test_users WHERE id = $1",
        &[&original_model.id],
    ).expect("Failed to query database");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, String>(0), "Updated via Save");
    assert_eq!(rows[0].get::<_, String>(1), "updated@example.com");
}

#[test]
fn test_entity_static_methods() {
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Test Entity::insert()
    let mut record = TestUserRecord::new();
    record.set_name("Static Insert".to_string());
    record.set_email("static@example.com".to_string());
    
    let model = TestUser::insert(record, &executor).expect("Failed to insert via static method");
    assert_eq!(model.name, "Static Insert");
    assert!(model.id > 0);

    // Test Entity::update()
    let mut update_record = TestUserRecord::from_model(&model);
    update_record.set_name("Static Update".to_string());
    
    let updated = TestUser::update(update_record, &executor).expect("Failed to update via static method");
    assert_eq!(updated.id, model.id);
    assert_eq!(updated.name, "Static Update");

    // Test Entity::delete()
    let delete_record = TestUserRecord::from_model(&updated);
    TestUser::delete(delete_record, &executor).expect("Failed to delete via static method");

    // Verify it's gone
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users WHERE id = $1",
        &[&model.id],
    ).expect("Failed to query database");
    assert_eq!(count, 0);
}

// ============================================================================
// TESTS FOR BUG FIX: Entities without primary keys
// ============================================================================
// These tests verify that entities without #[primary_key] attributes
// correctly handle CRUD operations:
// - update() and delete() should return errors (prevent mass updates/deletes)
// - insert() and save() should work (no primary key required)

// Test entity WITHOUT primary key
#[derive(LifeModel, LifeRecord)]
#[table_name = "test_no_pk_entities"]
pub struct TestNoPkEntity {
    pub name: String,
    pub email: String,
    pub age: Option<i32>,
}

fn setup_no_pk_schema(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_no_pk_entities (
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            age INTEGER
        )
        "#,
        &[],
    )?;
    Ok(())
}

fn cleanup_no_pk_data(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute("DELETE FROM test_no_pk_entities", &[])?;
    Ok(())
}

#[test]
fn test_no_primary_key_update_returns_error() {
    // BUG FIX TEST: update() should return error for entities without primary keys
    // This prevents mass updates that would affect all rows
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_no_pk_schema(&executor).expect("Failed to setup schema");
    cleanup_no_pk_data(&executor).expect("Failed to cleanup");

    // Insert some test data
    executor.execute(
        "INSERT INTO test_no_pk_entities (name, email) VALUES ($1, $2), ($3, $4)",
        &[&"User1".to_string(), &"user1@example.com".to_string(), 
          &"User2".to_string(), &"user2@example.com".to_string()],
    ).expect("Failed to insert test data");

    // Verify we have 2 rows
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_no_pk_entities",
        &[],
    ).expect("Failed to query database");
    assert_eq!(count, 2);

    // Try to update - should fail because entity has no primary key
    let mut record = TestNoPkEntityRecord::new();
    record.set_name("Updated Name".to_string());
    record.set_email("updated@example.com".to_string());

    let result = record.update(&executor);
    assert!(result.is_err(), "update() should fail for entities without primary keys");
    
    match result.unwrap_err() {
        ActiveModelError::Other(msg) => {
            assert!(msg.contains("without primary key"), 
                "Error message should mention 'without primary key', got: {}", msg);
        }
        e => panic!("Expected Other error with 'without primary key' message, got: {:?}", e),
    }

    // Verify no rows were updated (critical: prevents mass updates)
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_no_pk_entities WHERE name = 'Updated Name'",
        &[],
    ).expect("Failed to query database");
    assert_eq!(count, 0, "No rows should be updated when update() fails");
}

#[test]
fn test_no_primary_key_delete_returns_error() {
    // BUG FIX TEST: delete() should return error for entities without primary keys
    // This prevents mass deletes that would affect all rows
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_no_pk_schema(&executor).expect("Failed to setup schema");
    cleanup_no_pk_data(&executor).expect("Failed to cleanup");

    // Insert some test data
    executor.execute(
        "INSERT INTO test_no_pk_entities (name, email) VALUES ($1, $2), ($3, $4)",
        &[&"User1".to_string(), &"user1@example.com".to_string(), 
          &"User2".to_string(), &"user2@example.com".to_string()],
    ).expect("Failed to insert test data");

    // Verify we have 2 rows
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_no_pk_entities",
        &[],
    ).expect("Failed to query database");
    assert_eq!(count, 2);

    // Try to delete - should fail because entity has no primary key
    let record = TestNoPkEntityRecord::new();

    let result = record.delete(&executor);
    assert!(result.is_err(), "delete() should fail for entities without primary keys");
    
    match result.unwrap_err() {
        ActiveModelError::Other(msg) => {
            assert!(msg.contains("without primary key"), 
                "Error message should mention 'without primary key', got: {}", msg);
        }
        e => panic!("Expected Other error with 'without primary key' message, got: {:?}", e),
    }

    // Verify no rows were deleted (critical: prevents mass deletes)
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_no_pk_entities",
        &[],
    ).expect("Failed to query database");
    assert_eq!(count, 2, "No rows should be deleted when delete() fails");
}

#[test]
fn test_no_primary_key_save_always_inserts() {
    // BUG FIX TEST: save() should always do insert for entities without primary keys
    // Previously, save_pk_checks was empty, causing has_primary_key to always be true
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_no_pk_schema(&executor).expect("Failed to setup schema");
    cleanup_no_pk_data(&executor).expect("Failed to cleanup");

    // Create a new record
    let mut record = TestNoPkEntityRecord::new();
    record.set_name("Save Test".to_string());
    record.set_email("save@example.com".to_string());
    record.set_age(Some(25));

    // save() should insert (no primary key means always insert)
    let model = record.save(&executor).expect("Failed to save");

    // Verify it was inserted
    assert_eq!(model.name, "Save Test");
    assert_eq!(model.email, "save@example.com");
    assert_eq!(model.age, Some(25));

    // Verify in database
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_no_pk_entities WHERE name = $1 AND email = $2",
        &[&"Save Test".to_string(), &"save@example.com".to_string()],
    ).expect("Failed to query database");
    assert_eq!(count, 1, "Record should be inserted");
}

#[test]
fn test_no_primary_key_insert_works() {
    // POSITIVE TEST: insert() should work for entities without primary keys
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_no_pk_schema(&executor).expect("Failed to setup schema");
    cleanup_no_pk_data(&executor).expect("Failed to cleanup");

    // Create a new record
    let mut record = TestNoPkEntityRecord::new();
    record.set_name("Insert Test".to_string());
    record.set_email("insert@example.com".to_string());
    record.set_age(Some(30));

    // insert() should work
    let model = record.insert(&executor).expect("Failed to insert");

    // Verify it was inserted
    assert_eq!(model.name, "Insert Test");
    assert_eq!(model.email, "insert@example.com");
    assert_eq!(model.age, Some(30));

    // Verify in database
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_no_pk_entities WHERE name = $1 AND email = $2",
        &[&"Insert Test".to_string(), &"insert@example.com".to_string()],
    ).expect("Failed to query database");
    assert_eq!(count, 1, "Record should be inserted");
}

#[test]
fn test_with_primary_key_update_works() {
    // POSITIVE TEST: update() should work for entities WITH primary keys
    // This verifies the fix doesn't break existing functionality
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Insert a record
    let mut insert_record = TestUserRecord::new();
    insert_record.set_name("Original".to_string());
    insert_record.set_email("original@example.com".to_string());
    let original_model = insert_record.insert(&executor).expect("Failed to insert");

    // Update it
    let mut update_record = TestUserRecord::from_model(&original_model);
    update_record.set_name("Updated".to_string());
    update_record.set_email("updated@example.com".to_string());

    let updated_model = update_record.update(&executor).expect("Failed to update");

    // Verify it was updated (same ID, new values)
    assert_eq!(updated_model.id, original_model.id);
    assert_eq!(updated_model.name, "Updated");
    assert_eq!(updated_model.email, "updated@example.com");

    // Verify only one row was updated (WHERE clause works correctly)
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users WHERE name = 'Updated'",
        &[],
    ).expect("Failed to query database");
    assert_eq!(count, 1, "Only one row should be updated");
}

#[test]
fn test_with_primary_key_delete_works() {
    // POSITIVE TEST: delete() should work for entities WITH primary keys
    // This verifies the fix doesn't break existing functionality
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Insert two records
    let mut record1 = TestUserRecord::new();
    record1.set_name("User1".to_string());
    record1.set_email("user1@example.com".to_string());
    let model1 = record1.insert(&executor).expect("Failed to insert");

    let mut record2 = TestUserRecord::new();
    record2.set_name("User2".to_string());
    record2.set_email("user2@example.com".to_string());
    let model2 = record2.insert(&executor).expect("Failed to insert");

    // Verify both exist
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users",
        &[],
    ).expect("Failed to query database");
    assert_eq!(count, 2);

    // Delete only one
    let delete_record = TestUserRecord::from_model(&model1);
    delete_record.delete(&executor).expect("Failed to delete");

    // Verify only one was deleted (WHERE clause works correctly)
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users",
        &[],
    ).expect("Failed to query database");
    assert_eq!(count, 1, "Only one row should be deleted");

    // Verify the correct one was deleted
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users WHERE id = $1",
        &[&model1.id],
    ).expect("Failed to query database");
    assert_eq!(count, 0, "First record should be deleted");

    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users WHERE id = $1",
        &[&model2.id],
    ).expect("Failed to query database");
    assert_eq!(count, 1, "Second record should still exist");
}

#[test]
fn test_with_primary_key_save_works() {
    // POSITIVE TEST: save() should work correctly for entities WITH primary keys
    // This verifies the fix doesn't break existing functionality
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Test save() with no primary key set (should insert)
    let mut record = TestUserRecord::new();
    record.set_name("New User".to_string());
    record.set_email("new@example.com".to_string());
    
    let model = record.save(&executor).expect("Failed to save");
    assert!(model.id > 0);
    assert_eq!(model.name, "New User");

    // Test save() with primary key set (should update)
    let mut update_record = TestUserRecord::from_model(&model);
    update_record.set_name("Updated User".to_string());
    
    let updated = update_record.save(&executor).expect("Failed to save");
    assert_eq!(updated.id, model.id);
    assert_eq!(updated.name, "Updated User");

    // Verify in database
    let rows = executor.query_all(
        "SELECT name FROM test_users WHERE id = $1",
        &[&model.id],
    ).expect("Failed to query database");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, String>(0), "Updated User");
}

#[test]
fn test_active_model_insert_auto_increment_pk_not_set() {
    // POSITIVE TEST: insert() should work when auto-increment PK is not set
    // This test verifies the fix for the panic issue where to_model() would fail
    // when the auto-increment PK field was None after insert
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a new record WITHOUT setting the auto-increment PK
    let mut record = TestUserRecord::new();
    record.set_name("Auto Inc Test".to_string());
    record.set_email("autoinc@example.com".to_string());
    // Note: We explicitly do NOT set record.set_id() - the PK should be auto-generated

    // Insert should succeed and return a model with the generated PK
    // This should NOT panic even though id was None before insert
    let model = record.insert(&executor).expect("Insert should succeed without panicking");

    // Verify the inserted model has a generated PK
    assert!(model.id > 0, "Auto-increment PK should be generated");
    assert_eq!(model.name, "Auto Inc Test");
    assert_eq!(model.email, "autoinc@example.com");

    // Verify in database
    let rows = executor.query_all(
        "SELECT id, name, email FROM test_users WHERE id = $1",
        &[&model.id],
    ).expect("Failed to query database");
    
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.get::<_, i32>(0), model.id);
    assert_eq!(row.get::<_, String>(1), "Auto Inc Test");
    assert_eq!(row.get::<_, String>(2), "autoinc@example.com");
}

#[test]
fn test_active_model_insert_with_manual_auto_increment_pk() {
    // POSITIVE TEST: insert() should work when auto-increment PK is manually set
    // If the user explicitly sets the auto-increment PK, it should be used
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a new record WITH a manually set auto-increment PK
    let mut record = TestUserRecord::new();
    record.set_id(999); // Manually set the PK
    record.set_name("Manual PK Test".to_string());
    record.set_email("manualpk@example.com".to_string());

    // Insert should succeed and use the provided PK value
    let model = record.insert(&executor).expect("Insert should succeed with manual PK");

    // Verify the inserted model uses the provided PK
    assert_eq!(model.id, 999, "Should use manually set PK value");
    assert_eq!(model.name, "Manual PK Test");
    assert_eq!(model.email, "manualpk@example.com");

    // Verify in database
    let rows = executor.query_all(
        "SELECT id, name, email FROM test_users WHERE id = $1",
        &[&999i32],
    ).expect("Failed to query database");
    
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.get::<_, i32>(0), 999);
    assert_eq!(row.get::<_, String>(1), "Manual PK Test");
    assert_eq!(row.get::<_, String>(2), "manualpk@example.com");
}
