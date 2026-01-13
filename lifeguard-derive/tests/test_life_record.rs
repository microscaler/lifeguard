//! Tests for LifeRecord derive macro

use lifeguard_derive::{LifeModel, LifeRecord};

#[test]
fn test_basic_life_record() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_basic"]
    struct TestBasic {
        #[primary_key]
        id: i32,
        name: String,
    }

    // Test that Record struct is generated
    let _record = TestBasicRecord::new();
    
    // Test that Model struct is generated
    let model = TestBasicModel {
        id: 1,
        name: "Test".to_string(),
    };
    
    // Test from_model
    let record = TestBasicRecord::from_model(&model);
    assert_eq!(record.id, Some(1));
    assert_eq!(record.name, Some("Test".to_string()));
    
    // Test to_model
    let converted_model = record.to_model();
    assert_eq!(converted_model.id, 1);
    assert_eq!(converted_model.name, "Test".to_string());
}

#[test]
fn test_record_new() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_new"]
    struct TestNew {
        id: i32,
        name: String,
    }

    let record = TestNewRecord::new();
    assert_eq!(record.id, None);
    assert_eq!(record.name, None);
}

#[test]
fn test_record_setters() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_setters"]
    struct TestSetters {
        id: i32,
        name: String,
    }

    let mut record = TestSettersRecord::new();
    record.set_id(1).set_name("Test".to_string());
    
    assert_eq!(record.id, Some(1));
    assert_eq!(record.name, Some("Test".to_string()));
}

#[test]
fn test_dirty_fields() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_dirty"]
    struct TestDirty {
        id: i32,
        name: String,
        email: String,
    }

    let mut record = TestDirtyRecord::new();
    assert!(!record.is_dirty());
    assert_eq!(record.dirty_fields().len(), 0);
    
    record.set_name("Test".to_string());
    assert!(record.is_dirty());
    let dirty = record.dirty_fields();
    assert_eq!(dirty.len(), 1);
    assert!(dirty.contains(&"name".to_string()));
    
    record.set_email("test@example.com".to_string());
    let dirty = record.dirty_fields();
    assert_eq!(dirty.len(), 2);
    assert!(dirty.contains(&"name".to_string()));
    assert!(dirty.contains(&"email".to_string()));
}

#[test]
fn test_from_model_all_fields() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_from_model"]
    struct TestFromModel {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
        age: i32,
    }

    let model = TestFromModelModel {
        id: 1,
        name: "John".to_string(),
        email: "john@example.com".to_string(),
        age: 30,
    };
    
    let record = TestFromModelRecord::from_model(&model);
    assert_eq!(record.id, Some(1));
    assert_eq!(record.name, Some("John".to_string()));
    assert_eq!(record.email, Some("john@example.com".to_string()));
    assert_eq!(record.age, Some(30));
}

#[test]
fn test_to_model_with_none_fields() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_to_model"]
    struct TestToModel {
        id: i32,
        name: String,
    }

    let mut record = TestToModelRecord::new();
    record.set_id(1);
    
    // to_model should panic for required fields that are None
    let result = std::panic::catch_unwind(|| {
        record.to_model()
    });
    assert!(result.is_err());
}

#[test]
fn test_record_default() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_default"]
    struct TestDefault {
        id: i32,
        name: String,
    }

    let record = TestDefaultRecord::default();
    assert_eq!(record.id, None);
    assert_eq!(record.name, None);
}

#[test]
fn test_record_clone() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_clone"]
    struct TestClone {
        id: i32,
        name: String,
    }

    let mut record1 = TestCloneRecord::new();
    record1.set_id(1).set_name("Test".to_string());
    
    let record2 = record1.clone();
    assert_eq!(record2.id, Some(1));
    assert_eq!(record2.name, Some("Test".to_string()));
}

#[test]
fn test_record_with_nullable_field() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_nullable"]
    struct TestNullable {
        id: i32,
        #[nullable]
        name: Option<String>,
    }

    let mut record = TestNullableRecord::new();
    record.set_id(1);
    
    // Nullable fields should use Default::default() when None
    let model = record.to_model();
    assert_eq!(model.id, 1);
    assert_eq!(model.name, None);
    
    record.set_name(Some("Test".to_string()));
    let model = record.to_model();
    assert_eq!(model.name, Some("Test".to_string()));
}

#[test]
fn test_record_update_workflow() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_update"]
    struct TestUpdate {
        #[primary_key]
        id: i32,
        name: String,
        email: String,
    }

    // Simulate an update workflow
    let original_model = TestUpdateModel {
        id: 1,
        name: "John".to_string(),
        email: "john@example.com".to_string(),
    };
    
    // Create record from model
    let mut record = TestUpdateRecord::from_model(&original_model);
    
    // Update only the email
    record.set_email("newemail@example.com".to_string());
    
    // Check dirty fields - all fields are Some from from_model
    let dirty = record.dirty_fields();
    assert_eq!(dirty.len(), 3); // All fields are Some from from_model
    
    // Verify the change
    assert_eq!(record.email, Some("newemail@example.com".to_string()));
}

#[test]
fn test_record_insert_workflow() {
    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_insert"]
    struct TestInsert {
        id: i32,
        name: String,
        email: String,
    }

    // Simulate an insert workflow
    let mut record = TestInsertRecord::new();
    record.set_name("John".to_string()).set_email("john@example.com".to_string());
    
    // Check dirty fields (only set fields)
    let dirty = record.dirty_fields();
    assert_eq!(dirty.len(), 2);
    assert!(dirty.contains(&"name".to_string()));
    assert!(dirty.contains(&"email".to_string()));
    
    // Note: to_model() would panic because id is required but not set
    // This is expected behavior for inserts where you need to set all required fields
}
