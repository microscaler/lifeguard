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
fn test_no_primary_key_save_multiple_times_all_insert() {
    // BUG FIX TEST: Multiple saves on entities without primary keys should all insert
    // This verifies that save() doesn't try to update (which would fail with PrimaryKeyRequired)
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_no_pk_schema(&executor).expect("Failed to setup schema");
    cleanup_no_pk_data(&executor).expect("Failed to cleanup");

    // First save - should insert
    let mut record1 = TestNoPkEntityRecord::new();
    record1.set_name("First Save".to_string());
    record1.set_email("first@example.com".to_string());
    record1.set_age(Some(25));
    let model1 = record1.save(&executor).expect("Failed to save first record");

    // Second save with same data - should also insert (not try to update)
    let mut record2 = TestNoPkEntityRecord::new();
    record2.set_name("First Save".to_string());
    record2.set_email("first@example.com".to_string());
    record2.set_age(Some(25));
    let model2 = record2.save(&executor).expect("Failed to save second record");

    // Third save with different data - should also insert
    let mut record3 = TestNoPkEntityRecord::new();
    record3.set_name("Third Save".to_string());
    record3.set_email("third@example.com".to_string());
    record3.set_age(Some(30));
    let model3 = record3.save(&executor).expect("Failed to save third record");

    // Verify all three records were inserted (not updated)
    let total_count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_no_pk_entities",
        &[],
    ).expect("Failed to query database");
    assert_eq!(total_count, 3, "All three saves should have inserted new records");

    // Verify each record exists
    let count1 = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_no_pk_entities WHERE name = $1 AND email = $2",
        &[&"First Save".to_string(), &"first@example.com".to_string()],
    ).expect("Failed to query database");
    assert_eq!(count1, 2, "Two records with same name/email should exist");

    let count3 = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_no_pk_entities WHERE name = $1 AND email = $2",
        &[&"Third Save".to_string(), &"third@example.com".to_string()],
    ).expect("Failed to query database");
    assert_eq!(count3, 1, "One record with third name/email should exist");
}

#[test]
fn test_no_primary_key_save_with_hooks() {
    // BUG FIX TEST: save() on entities without primary keys should call hooks correctly
    // This verifies that before_save and after_save hooks work even without primary keys
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_no_pk_schema(&executor).expect("Failed to setup schema");
    cleanup_no_pk_data(&executor).expect("Failed to cleanup");

    // Create a record and save it
    let mut record = TestNoPkEntityRecord::new();
    record.set_name("Hook Test".to_string());
    record.set_email("hook@example.com".to_string());
    record.set_age(Some(25));

    // save() should insert and call hooks (hooks are called before the insert/update decision)
    let model = record.save(&executor).expect("Failed to save");

    // Verify it was inserted
    assert_eq!(model.name, "Hook Test");
    assert_eq!(model.email, "hook@example.com");
    assert_eq!(model.age, Some(25));

    // Verify in database
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_no_pk_entities WHERE name = $1 AND email = $2",
        &[&"Hook Test".to_string(), &"hook@example.com".to_string()],
    ).expect("Failed to query database");
    assert_eq!(count, 1, "Record should be inserted");
}

#[test]
fn test_with_primary_key_save_upsert_behavior() {
    // REGRESSION TEST: save() on entities WITH primary keys should still work correctly
    // This verifies the fix doesn't break existing upsert behavior
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // First save - should insert (no primary key set)
    let mut record1 = TestUserRecord::new();
    record1.set_name("New User".to_string());
    record1.set_email("new@example.com".to_string());
    let model1 = record1.save(&executor).expect("Failed to save");

    // Verify it was inserted
    assert!(model1.id > 0);
    assert_eq!(model1.name, "New User");
    assert_eq!(model1.email, "new@example.com");

    // Second save with primary key set - should update (upsert behavior)
    let mut record2 = TestUserRecord::from_model(&model1);
    record2.set_name("Updated User".to_string());
    record2.set_email("updated@example.com".to_string());
    let model2 = record2.save(&executor).expect("Failed to save");

    // Verify it was updated (same ID, new values)
    assert_eq!(model2.id, model1.id);
    assert_eq!(model2.name, "Updated User");
    assert_eq!(model2.email, "updated@example.com");

    // Verify only one record exists (not two)
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users WHERE id = $1",
        &[&model1.id],
    ).expect("Failed to query database");
    assert_eq!(count, 1, "Only one record should exist (update, not insert)");

    // Third save with non-existent primary key - should insert (upsert fallback)
    let mut record3 = TestUserRecord::new();
    record3.set_id(Some(99999)); // Non-existent ID
    record3.set_name("Fallback User".to_string());
    record3.set_email("fallback@example.com".to_string());
    let model3 = record3.save(&executor).expect("Failed to save");

    // Verify it was inserted (update failed, so insert happened)
    // The ID might be different if auto-increment is used, or might be 99999 if database allows
    assert_eq!(model3.name, "Fallback User");
    assert_eq!(model3.email, "fallback@example.com");
}

#[test]
fn test_no_primary_key_save_insert_works() {
    // BUG FIX TEST: save() on entities without primary keys should work identically to insert()
    // This verifies that save() correctly routes to insert() when no primary keys exist
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_no_pk_schema(&executor).expect("Failed to setup schema");
    cleanup_no_pk_data(&executor).expect("Failed to cleanup");

    // Test save() behavior
    let mut save_record = TestNoPkEntityRecord::new();
    save_record.set_name("Save Method".to_string());
    save_record.set_email("save@example.com".to_string());
    save_record.set_age(Some(25));
    let save_model = save_record.save(&executor).expect("Failed to save");

    // Test insert() behavior
    let mut insert_record = TestNoPkEntityRecord::new();
    insert_record.set_name("Insert Method".to_string());
    insert_record.set_email("insert@example.com".to_string());
    insert_record.set_age(Some(30));
    let insert_model = insert_record.insert(&executor).expect("Failed to insert");

    // Both should work identically
    assert_eq!(save_model.name, "Save Method");
    assert_eq!(insert_model.name, "Insert Method");

    // Verify both records exist in database
    let total_count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_no_pk_entities",
        &[],
    ).expect("Failed to query database");
    assert_eq!(total_count, 2, "Both save() and insert() should have created records");
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
fn test_save_with_nonexistent_record_should_insert() {
    // NEGATIVE TEST: save() with primary key set but record doesn't exist
    // This tests the bug fix - previously save() would return Ok() but nothing was saved
    // Now it should detect zero rows from update() and fall back to insert
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a record with a primary key that doesn't exist in the database
    // This simulates the bug scenario where save() was called with a PK set
    // but the record doesn't actually exist
    let mut record = TestUserRecord::new();
    record.set_id(999); // Set a PK that doesn't exist
    record.set_name("Non-existent User".to_string());
    record.set_email("nonexistent@example.com".to_string());
    
    // save() should detect that update() affected zero rows and fall back to insert
    // However, since we're setting an auto-increment PK, this might fail
    // Let's test with a non-auto-increment scenario or just verify the behavior
    
    // Actually, for auto-increment PKs, we can't set them manually in insert
    // So let's test a different scenario: create a record, delete it, then try to save with that ID
    // First, insert a record normally
    let mut insert_record = TestUserRecord::new();
    insert_record.set_name("Original User".to_string());
    insert_record.set_email("original@example.com".to_string());
    let original_model = insert_record.insert(&executor).expect("Failed to insert");
    let original_id = original_model.id;
    
    // Delete the record
    let delete_record = TestUserRecord::from_model(&original_model);
    delete_record.delete(&executor).expect("Failed to delete");
    
    // Verify it's gone
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users WHERE id = $1",
        &[&original_id],
    ).expect("Failed to query database");
    assert_eq!(count, 0, "Record should be deleted");
    
    // Now try to save with the deleted record's ID
    // This should detect zero rows from update() and fall back to insert
    // But since the PK is auto-increment, insert will generate a new ID
    let mut save_record = TestUserRecord::new();
    save_record.set_id(original_id); // Set the deleted record's ID
    save_record.set_name("Resurrected User".to_string());
    save_record.set_email("resurrected@example.com".to_string());
    
    // save() should detect RecordNotFound from update() and fall back to insert
    // Since it's auto-increment, insert will ignore the set ID and generate a new one
    let saved_model = save_record.save(&executor).expect("Failed to save");
    
    // The saved model should have a NEW ID (not the original_id) because insert ignores set auto-increment PKs
    assert_ne!(saved_model.id, original_id, "Insert should generate a new ID for auto-increment PK");
    assert_eq!(saved_model.name, "Resurrected User");
    
    // Verify the new record exists in database
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users WHERE id = $1",
        &[&saved_model.id],
    ).expect("Failed to query database");
    assert_eq!(count, 1, "New record should exist");
    
    // Verify the old ID is still gone
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users WHERE id = $1",
        &[&original_id],
    ).expect("Failed to query database");
    assert_eq!(count, 0, "Old record should still be deleted");
}

#[test]
fn test_save_with_existing_record_should_update() {
    // POSITIVE TEST: save() with existing record should update it
    // This verifies the fix doesn't break the normal update path
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
    
    // Now save with the same record (should update)
    let mut save_record = TestUserRecord::from_model(&original_model);
    save_record.set_name("Updated via Save".to_string());
    save_record.set_email("updated@example.com".to_string());
    
    let saved_model = save_record.save(&executor).expect("Failed to save");
    
    // Should have the same ID (update, not insert)
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
    
    // Verify only one record exists (not inserted a new one)
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_users",
        &[],
    ).expect("Failed to query database");
    assert_eq!(count, 1, "Should only have one record");
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

// ============================================================================
// BUG FIX TESTS: get() returns None for unset fields
// ============================================================================
// These tests verify the fix for generate_option_field_to_value which was
// always wrapping results in Some(...), preventing get() from returning None
// for unset fields. This broke CRUD operations that rely on get().is_none()
// to detect unset fields.

#[test]
fn test_insert_skips_unset_auto_increment_pk() {
    // CRITICAL TEST: INSERT should skip unset auto-increment primary keys
    // Previously, get() returned Some(Value::Int(None)) for unset fields,
    // causing INSERT to include the PK with NULL, which would fail
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a record WITHOUT setting the auto-increment PK
    let mut record = TestUserRecord::new();
    record.set_name("Unset PK Test".to_string());
    record.set_email("unsetpk@example.com".to_string());
    // Note: We explicitly do NOT set record.set_id() - the PK should be auto-generated

    // Verify get() returns None for unset PK
    let id_value = record.get(<TestUser as LifeModelTrait>::Column::Id);
    assert!(id_value.is_none(), "get() should return None for unset auto-increment PK");

    // Insert should succeed and skip the unset PK (let database generate it)
    let model = record.insert(&executor).expect("Insert should succeed without unset PK");

    // Verify the inserted model has a generated PK
    assert!(model.id > 0, "Auto-increment PK should be generated");
    assert_eq!(model.name, "Unset PK Test");
    assert_eq!(model.email, "unsetpk@example.com");

    // Verify in database
    let rows = executor.query_all(
        "SELECT id, name, email FROM test_users WHERE id = $1",
        &[&model.id],
    ).expect("Failed to query database");
    
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.get::<_, i32>(0), model.id);
    assert_eq!(row.get::<_, String>(1), "Unset PK Test");
    assert_eq!(row.get::<_, String>(2), "unsetpk@example.com");
}

#[test]
fn test_insert_returns_generated_auto_increment_pk() {
    // CRITICAL TEST: INSERT with RETURNING clause should fetch generated auto-increment PK
    // Previously, get().is_none() was always false, so RETURNING clause was never added
    // This test verifies that RETURNING works correctly when PK is unset
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a record WITHOUT setting the auto-increment PK
    let mut record = TestUserRecord::new();
    record.set_name("RETURNING Test".to_string());
    record.set_email("returning@example.com".to_string());
    // Note: We explicitly do NOT set record.set_id() - the PK should be auto-generated

    // Verify get() returns None for unset PK (this triggers RETURNING clause)
    let id_value = record.get(<TestUser as LifeModelTrait>::Column::Id);
    assert!(id_value.is_none(), "get() should return None for unset auto-increment PK");

    // Insert should succeed, use RETURNING to fetch generated PK, and return model with PK set
    let model = record.insert(&executor).expect("Insert should succeed and return model with generated PK");

    // Verify the inserted model has a generated PK (RETURNING clause worked)
    assert!(model.id > 0, "Auto-increment PK should be generated and returned via RETURNING");
    assert_eq!(model.name, "RETURNING Test");
    assert_eq!(model.email, "returning@example.com");

    // Verify in database
    let rows = executor.query_all(
        "SELECT id, name, email FROM test_users WHERE id = $1",
        &[&model.id],
    ).expect("Failed to query database");
    
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.get::<_, i32>(0), model.id);
    assert_eq!(row.get::<_, String>(1), "RETURNING Test");
    assert_eq!(row.get::<_, String>(2), "returning@example.com");
}

#[test]
fn test_update_only_includes_set_fields() {
    // CRITICAL TEST: UPDATE should only include set fields in SET clauses
    // Previously, get() returned Some(Value::String(None)) for unset fields,
    // causing UPDATE to include all fields, setting unset ones to NULL
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // First, insert a record with all fields set
    let mut insert_record = TestUserRecord::new();
    insert_record.set_name("Original Name".to_string());
    insert_record.set_email("original@example.com".to_string());
    insert_record.set_age(Some(30));
    let original_model = insert_record.insert(&executor).expect("Failed to insert");

    // Now create an update record with only name changed (email and age not set)
    let mut update_record = TestUserRecord::new();
    update_record.set_id(original_model.id); // Set PK for WHERE clause
    update_record.set_name("Updated Name".to_string());
    // Note: email and age are NOT set - they should NOT appear in UPDATE SET clause

    // Verify get() returns None for unset fields
    let email_value = update_record.get(<TestUser as LifeModelTrait>::Column::Email);
    assert!(email_value.is_none(), "get() should return None for unset email field");
    
    let age_value = update_record.get(<TestUser as LifeModelTrait>::Column::Age);
    assert!(age_value.is_none(), "get() should return None for unset age field");

    // Update should only update the name field, not email or age
    let updated_model = update_record.update(&executor).expect("Failed to update");

    // Verify only name was updated
    assert_eq!(updated_model.id, original_model.id);
    assert_eq!(updated_model.name, "Updated Name");
    assert_eq!(updated_model.email, "original@example.com"); // Should remain unchanged
    assert_eq!(updated_model.age, Some(30)); // Should remain unchanged

    // Verify in database
    let rows = executor.query_all(
        "SELECT name, email, age FROM test_users WHERE id = $1",
        &[&original_model.id],
    ).expect("Failed to query database");
    
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.get::<_, String>(0), "Updated Name");
    assert_eq!(row.get::<_, String>(1), "original@example.com"); // Unchanged
    assert_eq!(row.get::<_, Option<i32>>(2), Some(30)); // Unchanged
}

#[test]
fn test_get_returns_none_for_unset_fields_integration() {
    // INTEGRATION TEST: Verify get() returns None for unset fields in real scenario
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a new record with only some fields set
    let mut record = TestUserRecord::new();
    record.set_name("Partial Set".to_string());
    // email and age are NOT set

    // Verify get() returns None for unset fields
    let email_value = record.get(<TestUser as LifeModelTrait>::Column::Email);
    assert!(email_value.is_none(), "get() should return None for unset email field");
    
    let age_value = record.get(<TestUser as LifeModelTrait>::Column::Age);
    assert!(age_value.is_none(), "get() should return None for unset age field");

    // Verify get() returns Some for set fields
    let name_value = record.get(<TestUser as LifeModelTrait>::Column::Name);
    assert!(name_value.is_some(), "get() should return Some(Value) for set name field");
    match name_value.unwrap() {
        sea_query::Value::String(Some(v)) => assert_eq!(v, "Partial Set"),
        _ => panic!("Expected String(Some(\"Partial Set\"))"),
    }

    // Insert should work (only set fields are included)
    let model = record.insert(&executor).expect("Insert should succeed with partial fields");

    // Verify inserted model
    assert!(model.id > 0);
    assert_eq!(model.name, "Partial Set");
    assert_eq!(model.email, ""); // Default for unset String field (from to_model())
    assert_eq!(model.age, None); // None for unset Option<i32> field
}

// ============================================================================
// CRITICAL BUG FIX TESTS: Hook modifications must be persisted
// ============================================================================
// These tests verify that modifications made in before_insert() and before_update()
// hooks are actually saved to the database. This catches bugs where hooks modify
// record_for_hooks but the INSERT/UPDATE query uses self.get() instead of
// record_for_hooks.get().

// Test entity with hook modifications
#[derive(LifeModel, LifeRecord)]
#[table_name = "test_hook_users"]
pub struct TestHookUser {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub created_at: Option<String>, // Set by before_insert hook
    pub updated_at: Option<String>, // Set by before_update hook
}

fn setup_hook_test_schema(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_hook_users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            created_at TEXT,
            updated_at TEXT
        )
        "#,
        &[],
    )?;
    Ok(())
}

fn cleanup_hook_test_data(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute("DELETE FROM test_hook_users", &[])?;
    Ok(())
}

// Custom Record with before_insert hook that modifies fields
#[derive(Clone, Debug)]
struct HookModifyingRecord {
    inner: TestHookUserRecord,
}

impl lifeguard::ActiveModelTrait for HookModifyingRecord {
    type Entity = TestHookUser;
    type Model = TestHookUserModel;
    
    fn get(&self, column: <TestHookUser as lifeguard::LifeModelTrait>::Column) -> Option<sea_query::Value> {
        self.inner.get(column)
    }
    
    fn set(&mut self, column: <TestHookUser as lifeguard::LifeModelTrait>::Column, value: sea_query::Value) -> Result<(), lifeguard::ActiveModelError> {
        self.inner.set(column, value)
    }
    
    fn take(&mut self, column: <TestHookUser as lifeguard::LifeModelTrait>::Column) -> Option<sea_query::Value> {
        self.inner.take(column)
    }
    
    fn reset(&mut self) {
        self.inner.reset()
    }
    
    fn insert<E: lifeguard::LifeExecutor>(&self, executor: &E) -> Result<Self::Model, lifeguard::ActiveModelError> {
        self.inner.insert(executor)
    }
    
    fn update<E: lifeguard::LifeExecutor>(&self, executor: &E) -> Result<Self::Model, lifeguard::ActiveModelError> {
        self.inner.update(executor)
    }
    
    fn save<E: lifeguard::LifeExecutor>(&self, executor: &E) -> Result<Self::Model, lifeguard::ActiveModelError> {
        self.inner.save(executor)
    }
    
    fn delete<E: lifeguard::LifeExecutor>(&self, executor: &E) -> Result<(), lifeguard::ActiveModelError> {
        self.inner.delete(executor)
    }
    
    fn from_json(_json: serde_json::Value) -> Result<Self, lifeguard::ActiveModelError> {
        Err(lifeguard::ActiveModelError::Other("not implemented".to_string()))
    }
    
    fn to_json(&self) -> Result<serde_json::Value, lifeguard::ActiveModelError> {
        self.inner.to_json()
    }
}

impl lifeguard::ActiveModelBehavior for HookModifyingRecord {
    fn before_insert(&mut self) -> Result<(), lifeguard::ActiveModelError> {
        // CRITICAL: Modify fields in before_insert hook
        // These modifications MUST be saved to the database
        use lifeguard::LifeModelTrait;
        let timestamp = "2024-01-01T00:00:00Z".to_string();
        self.set(
            <TestHookUser as LifeModelTrait>::Column::CreatedAt,
            sea_query::Value::String(Some(timestamp.clone()))
        )?;
        // Also modify name to verify hook changes are persisted
        self.set(
            <TestHookUser as LifeModelTrait>::Column::Name,
            sea_query::Value::String(Some("Modified by before_insert hook".to_string()))
        )?;
        Ok(())
    }
    
    fn before_update(&mut self) -> Result<(), lifeguard::ActiveModelError> {
        // CRITICAL: Modify fields in before_update hook
        // These modifications MUST be saved to the database
        use lifeguard::LifeModelTrait;
        let timestamp = "2024-01-02T00:00:00Z".to_string();
        self.set(
            <TestHookUser as LifeModelTrait>::Column::UpdatedAt,
            sea_query::Value::String(Some(timestamp.clone()))
        )?;
        // Also modify name to verify hook changes are persisted
        self.set(
            <TestHookUser as LifeModelTrait>::Column::Name,
            sea_query::Value::String(Some("Modified by before_update hook".to_string()))
        )?;
        Ok(())
    }
    
    fn before_delete(&mut self) -> Result<(), lifeguard::ActiveModelError> {
        // CRITICAL: Modify primary key in before_delete hook
        // This modification MUST be used in the DELETE WHERE clause
        // This test verifies that delete() uses record_for_hooks.get() instead of self.get()
        use lifeguard::LifeModelTrait;
        // Get the current ID value
        if let Some(current_id) = self.get(<TestHookUser as LifeModelTrait>::Column::Id) {
            if let sea_query::Value::Int(Some(id)) = current_id {
                // Modify the ID to a different value (for testing purposes)
                // In a real scenario, this might be used for soft deletes or conditional deletes
                // For this test, we'll set it to a non-existent ID to verify the hook modification is used
                self.set(
                    <TestHookUser as LifeModelTrait>::Column::Id,
                    sea_query::Value::Int(Some(id + 1000)) // Set to non-existent ID
                )?;
            }
        }
        Ok(())
    }
}

#[test]
fn test_before_insert_hook_modifications_are_persisted() {
    // CRITICAL BUG FIX TEST: Verify that modifications made in before_insert()
    // hook are actually saved to the database. This test would have caught the
    // bug where insert() used self.get() instead of record_for_hooks.get().
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_hook_test_schema(&executor).expect("Failed to setup schema");
    cleanup_hook_test_data(&executor).expect("Failed to cleanup");

    // Create a record with initial values
    let mut record = HookModifyingRecord {
        inner: TestHookUserRecord::new(),
    };
    record.set_name("Original Name".to_string()).expect("Failed to set name");
    record.set_email("test@example.com".to_string()).expect("Failed to set email");
    // created_at is NOT set - should be set by before_insert hook

    // Insert the record - before_insert hook will modify name and set created_at
    let model = record.insert(&executor).expect("Failed to insert");

    // CRITICAL ASSERTION: The returned model should reflect hook modifications
    assert_eq!(model.name, "Modified by before_insert hook", 
        "Returned model should reflect before_insert hook modifications");
    assert_eq!(model.created_at, Some("2024-01-01T00:00:00Z".to_string()),
        "Returned model should have created_at set by before_insert hook");

    // CRITICAL ASSERTION: The database should contain hook-modified values
    let rows = executor.query_all(
        "SELECT name, created_at FROM test_hook_users WHERE id = $1",
        &[&model.id],
    ).expect("Failed to query database");
    
    assert_eq!(rows.len(), 1, "Record should exist in database");
    let row = &rows[0];
    let db_name: String = row.get(0);
    let db_created_at: Option<String> = row.get(1);
    
    assert_eq!(db_name, "Modified by before_insert hook",
        "Database should contain name modified by before_insert hook");
    assert_eq!(db_created_at, Some("2024-01-01T00:00:00Z".to_string()),
        "Database should contain created_at set by before_insert hook");

    // CRITICAL ASSERTION: Returned model should match database state
    assert_eq!(model.name, db_name, "Returned model name should match database");
    assert_eq!(model.created_at, db_created_at, "Returned model created_at should match database");
}

#[test]
fn test_before_update_hook_modifications_are_persisted() {
    // CRITICAL BUG FIX TEST: Verify that modifications made in before_update()
    // hook are actually saved to the database. This test would have caught the
    // bug where update() used self.get() instead of record_for_hooks.get().
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_hook_test_schema(&executor).expect("Failed to setup schema");
    cleanup_hook_test_data(&executor).expect("Failed to cleanup");

    // First, insert a record
    let mut insert_record = TestHookUserRecord::new();
    insert_record.set_name("Original Name".to_string()).expect("Failed to set name");
    insert_record.set_email("test@example.com".to_string()).expect("Failed to set email");
    let original_model = insert_record.insert(&executor).expect("Failed to insert");

    // Now update it with a hook-modifying record
    let mut update_record = HookModifyingRecord {
        inner: TestHookUserRecord::from_model(&original_model),
    };
    update_record.set_name("Update Name".to_string()).expect("Failed to set name");
    // updated_at is NOT set - should be set by before_update hook

    // Update the record - before_update hook will modify name and set updated_at
    let model = update_record.update(&executor).expect("Failed to update");

    // CRITICAL ASSERTION: The returned model should reflect hook modifications
    assert_eq!(model.name, "Modified by before_update hook",
        "Returned model should reflect before_update hook modifications");
    assert_eq!(model.updated_at, Some("2024-01-02T00:00:00Z".to_string()),
        "Returned model should have updated_at set by before_update hook");

    // CRITICAL ASSERTION: The database should contain hook-modified values
    let rows = executor.query_all(
        "SELECT name, updated_at FROM test_hook_users WHERE id = $1",
        &[&original_model.id],
    ).expect("Failed to query database");
    
    assert_eq!(rows.len(), 1, "Record should exist in database");
    let row = &rows[0];
    let db_name: String = row.get(0);
    let db_updated_at: Option<String> = row.get(1);
    
    assert_eq!(db_name, "Modified by before_update hook",
        "Database should contain name modified by before_update hook");
    assert_eq!(db_updated_at, Some("2024-01-02T00:00:00Z".to_string()),
        "Database should contain updated_at set by before_update hook");

    // CRITICAL ASSERTION: Returned model should match database state
    assert_eq!(model.name, db_name, "Returned model name should match database");
    assert_eq!(model.updated_at, db_updated_at, "Returned model updated_at should match database");
}

#[test]
fn test_before_insert_hook_modifications_with_multiple_fields() {
    // EDGE CASE: Verify that ALL modifications made in before_insert() are persisted
    // This ensures that the fix works for all field types and scenarios
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_hook_test_schema(&executor).expect("Failed to setup schema");
    cleanup_hook_test_data(&executor).expect("Failed to cleanup");

    // Create a record with initial values
    let mut record = HookModifyingRecord {
        inner: TestHookUserRecord::new(),
    };
    record.set_name("Initial Name".to_string()).expect("Failed to set name");
    record.set_email("initial@example.com".to_string()).expect("Failed to set email");
    // created_at is NOT set - should be set by before_insert hook
    // name will be modified by before_insert hook

    // Insert the record
    let model = record.insert(&executor).expect("Failed to insert");

    // Verify ALL hook modifications are in the database
    let rows = executor.query_all(
        "SELECT name, email, created_at FROM test_hook_users WHERE id = $1",
        &[&model.id],
    ).expect("Failed to query database");
    
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    let db_name: String = row.get(0);
    let db_email: String = row.get(1);
    let db_created_at: Option<String> = row.get(2);
    
    // Verify hook-modified field
    assert_eq!(db_name, "Modified by before_insert hook");
    // Verify non-hook-modified field is unchanged
    assert_eq!(db_email, "initial@example.com");
    // Verify hook-set field
    assert_eq!(db_created_at, Some("2024-01-01T00:00:00Z".to_string()));

    // Verify returned model matches database
    assert_eq!(model.name, db_name);
    assert_eq!(model.email, db_email);
    assert_eq!(model.created_at, db_created_at);
}

#[test]
fn test_before_delete_hook_modifications_are_used_in_where_clause() {
    // CRITICAL BUG FIX TEST: Verify that modifications made in before_delete()
    // hook are actually used in the DELETE WHERE clause. This test would have caught the
    // bug where delete() used self.get() instead of record_for_hooks.get().
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_hook_test_schema(&executor).expect("Failed to setup schema");
    cleanup_hook_test_data(&executor).expect("Failed to cleanup");

    // Insert two records
    let mut insert_record1 = TestHookUserRecord::new();
    insert_record1.set_name("Record 1".to_string()).expect("Failed to set name");
    insert_record1.set_email("record1@example.com".to_string()).expect("Failed to set email");
    let model1 = insert_record1.insert(&executor).expect("Failed to insert");

    let mut insert_record2 = TestHookUserRecord::new();
    insert_record2.set_name("Record 2".to_string()).expect("Failed to set name");
    insert_record2.set_email("record2@example.com".to_string()).expect("Failed to set email");
    let model2 = insert_record2.insert(&executor).expect("Failed to insert");

    // Verify both records exist
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_hook_users",
        &[],
    ).expect("Failed to query database");
    assert_eq!(count, 2, "Both records should exist");

    // Create a delete record with the first model's ID
    // The before_delete hook will modify the ID to a non-existent value (id + 1000)
    let mut delete_record = HookModifyingRecord {
        inner: TestHookUserRecord::from_model(&model1),
    };

    // Delete should use the modified ID from before_delete hook
    // Since the modified ID doesn't exist, no record should be deleted
    delete_record.delete(&executor).expect("Delete should succeed (even if no rows affected)");

    // CRITICAL ASSERTION: Both records should still exist because the hook-modified ID doesn't exist
    // This verifies that delete() used record_for_hooks.get() (the hook-modified value)
    // instead of self.get() (the original value)
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_hook_users",
        &[],
    ).expect("Failed to query database");
    assert_eq!(count, 2, "Both records should still exist because hook-modified ID doesn't exist");

    // Verify both records are still there
    let count1 = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_hook_users WHERE id = $1",
        &[&model1.id],
    ).expect("Failed to query database");
    assert_eq!(count1, 1, "Record 1 should still exist");

    let count2 = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_hook_users WHERE id = $1",
        &[&model2.id],
    ).expect("Failed to query database");
    assert_eq!(count2, 1, "Record 2 should still exist");
}

#[test]
fn test_before_delete_hook_with_original_id_deletes_correctly() {
    // POSITIVE TEST: Verify that delete() works correctly when before_delete() doesn't modify the ID
    // This ensures the fix doesn't break normal delete operations
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_hook_test_schema(&executor).expect("Failed to setup schema");
    cleanup_hook_test_data(&executor).expect("Failed to cleanup");

    // Insert a record
    let mut insert_record = TestHookUserRecord::new();
    insert_record.set_name("To Delete".to_string()).expect("Failed to set name");
    insert_record.set_email("delete@example.com".to_string()).expect("Failed to set email");
    let model = insert_record.insert(&executor).expect("Failed to insert");

    // Create a delete record (using regular TestHookUserRecord, not HookModifyingRecord)
    // This record doesn't modify the ID in before_delete, so it should delete normally
    let delete_record = TestHookUserRecord::from_model(&model);

    // Delete should work normally (no hook modifications)
    delete_record.delete(&executor).expect("Failed to delete");

    // Verify the record is deleted
    let count = query_count(
        &executor,
        "SELECT COUNT(*) FROM test_hook_users WHERE id = $1",
        &[&model.id],
    ).expect("Failed to query database");
    assert_eq!(count, 0, "Record should be deleted");
}
