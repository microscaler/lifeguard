//! Edge cases and error handling tests for LifeRecord derive macro
//!
//! Tests error conditions, boundary cases, and unusual inputs

use lifeguard_derive::{LifeModel, LifeRecord};

// Test entity with various edge case scenarios
#[derive(LifeModel, LifeRecord)]
#[table_name = "edge_case_users"]
pub struct EdgeCaseUser {
    #[primary_key]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub age: Option<i32>,
    pub active: bool,
}

#[cfg(test)]
mod tests {
    use super::*;


    // ============================================================================
    // Edge Cases: Required Fields in to_model()
    // ============================================================================

    #[test]
    #[should_panic] // Panic message may vary, just verify it panics
    fn test_to_model_panics_on_missing_required_field() {
        // Verify to_model panics when required field is None
        let record = EdgeCaseUserRecord::new();
        // id, name, email, active are required (not nullable)
        // Should panic when trying to convert
        let _model = record.to_model();
    }

    #[test]
    fn test_to_model_with_all_required_fields_set() {
        // Verify to_model works when all required fields are set
        let mut record = EdgeCaseUserRecord::new();
        record.set_id(1);
        record.set_name("Test".to_string());
        record.set_email("test@example.com".to_string());
        record.set_active(true);
        // age is optional, can be None
        
        let model = record.to_model();
        assert_eq!(model.id, 1);
        assert_eq!(model.name, "Test");
        assert_eq!(model.email, "test@example.com");
        assert_eq!(model.active, true);
    }

    // ============================================================================
    // Edge Cases: Option<T> Fields in Record
    // ============================================================================

    #[test]
    fn test_option_field_becomes_option_option() {
        // Verify Option<T> fields in Model become Option<Option<T>> in Record
        let model = EdgeCaseUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: Some(30),  // Option<i32>
            active: true,
        };
        
        let record = EdgeCaseUserRecord::from_model(&model);
        assert_eq!(record.age, Some(Some(30))); // Option<Option<i32>>
    }

    #[test]
    fn test_option_field_none_becomes_some_none() {
        // Verify None in Option<T> becomes Some(None) in Record
        let model = EdgeCaseUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: None,  // None
            active: true,
        };
        
        let record = EdgeCaseUserRecord::from_model(&model);
        assert_eq!(record.age, Some(None)); // Some(None)
    }

    // ============================================================================
    // Edge Cases: dirty_fields() Behavior
    // ============================================================================

    #[test]
    fn test_dirty_fields_empty_record() {
        // Verify dirty_fields returns empty for new record
        let record = EdgeCaseUserRecord::new();
        assert_eq!(record.dirty_fields().len(), 0);
        assert!(!record.is_dirty());
    }

    #[test]
    fn test_dirty_fields_all_fields_set() {
        // Verify dirty_fields returns all fields when all are set
        let model = EdgeCaseUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: Some(30),
            active: true,
        };
        
        let record = EdgeCaseUserRecord::from_model(&model);
        let dirty = record.dirty_fields();
        assert_eq!(dirty.len(), 5); // All 5 fields
    }

    #[test]
    fn test_dirty_fields_partial_set() {
        // Verify dirty_fields only returns set fields
        let mut record = EdgeCaseUserRecord::new();
        record.set_id(1);
        record.set_name("Test".to_string());
        // email, age, active not set
        
        let dirty = record.dirty_fields();
        assert_eq!(dirty.len(), 2);
        assert!(dirty.contains(&"id".to_string()));
        assert!(dirty.contains(&"name".to_string()));
    }

    #[test]
    fn test_dirty_fields_includes_none_values() {
        // Verify dirty_fields includes fields set to Some(None)
        let mut record = EdgeCaseUserRecord::new();
        record.set_age(None); // Explicitly set to None
        // This creates Some(None), which should be in dirty_fields
        
        let dirty = record.dirty_fields();
        assert!(dirty.contains(&"age".to_string()));
    }

    // ============================================================================
    // Edge Cases: Setter Methods
    // ============================================================================

    #[test]
    fn test_setter_chaining() {
        // Verify setter methods can be chained
        let mut record = EdgeCaseUserRecord::new();
        record
            .set_id(1)
            .set_name("Test".to_string())
            .set_email("test@example.com".to_string())
            .set_active(true);
        
        assert_eq!(record.id, Some(1));
        assert_eq!(record.name, Some("Test".to_string()));
    }

    #[test]
    fn test_setter_overwrites_previous_value() {
        // Verify setter overwrites previous value
        let mut record = EdgeCaseUserRecord::new();
        record.set_name("First".to_string());
        record.set_name("Second".to_string());
        
        assert_eq!(record.name, Some("Second".to_string()));
        assert_eq!(record.dirty_fields().len(), 1); // Still only one field
    }

    #[test]
    fn test_setter_with_option_none() {
        // Verify setter works with None for Option<T> fields
        let mut record = EdgeCaseUserRecord::new();
        record.set_age(None);
        assert_eq!(record.age, Some(None));
    }

    #[test]
    fn test_setter_with_option_some() {
        // Verify setter works with Some(value) for Option<T> fields
        let mut record = EdgeCaseUserRecord::new();
        record.set_age(Some(30));
        assert_eq!(record.age, Some(Some(30)));
    }

    // ============================================================================
    // Edge Cases: from_model() Roundtrip
    // ============================================================================

    #[test]
    fn test_from_model_to_model_roundtrip() {
        // Verify roundtrip: Model -> Record -> Model preserves all values
        let original = EdgeCaseUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: Some(30),
            active: true,
        };
        
        let record = EdgeCaseUserRecord::from_model(&original);
        let converted = record.to_model();
        
        assert_eq!(original.id, converted.id);
        assert_eq!(original.name, converted.name);
        assert_eq!(original.email, converted.email);
        assert_eq!(original.age, converted.age);
        assert_eq!(original.active, converted.active);
    }

    #[test]
    fn test_from_model_to_model_with_none() {
        // Verify roundtrip preserves None values
        let original = EdgeCaseUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: None,
            active: false,
        };
        
        let record = EdgeCaseUserRecord::from_model(&original);
        let converted = record.to_model();
        
        assert_eq!(original.age, converted.age); // Both None
    }

    // ============================================================================
    // Edge Cases: Clone Behavior
    // ============================================================================

    #[test]
    fn test_record_clone_preserves_state() {
        // Verify clone preserves all field values and dirty state
        let mut record1 = EdgeCaseUserRecord::new();
        record1.set_id(1);
        record1.set_name("Test".to_string());
        
        let record2 = record1.clone();
        assert_eq!(record1.id, record2.id);
        assert_eq!(record1.name, record2.name);
        assert_eq!(record1.dirty_fields(), record2.dirty_fields());
    }

    #[test]
    fn test_record_clone_independent_mutations() {
        // Verify cloned records can be mutated independently
        let mut record1 = EdgeCaseUserRecord::new();
        record1.set_id(1);
        
        let mut record2 = record1.clone();
        record2.set_id(2);
        
        assert_eq!(record1.id, Some(1));
        assert_eq!(record2.id, Some(2));
    }
}
