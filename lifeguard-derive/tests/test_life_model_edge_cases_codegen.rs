//! Edge cases and error handling tests using codegen (not procedural macros)
//!
//! This test uses lifeguard-codegen to generate Entity, Model, Column, etc.
//! before compilation, avoiding E0223 errors from procedural macros.

// Include generated code
include!("generated/edgecaseuser.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use lifeguard::LifeEntityName;

    // ============================================================================
    // Edge Cases: Table Name Handling
    // ============================================================================

    #[test]
    fn test_table_name_with_underscores() {
        // Verify table names with underscores work
        assert_eq!(EdgeCaseUser::default().table_name(), "edge_case_users");
        assert_eq!(EdgeCaseUser::TABLE_NAME, "edge_case_users");
    }

    // ============================================================================
    // Edge Cases: Column Enum
    // ============================================================================

    #[test]
    fn test_column_enum_all_variants() {
        // Verify all column variants exist
        let _id = Column::Id;
        let _name = Column::Name;
        let _email = Column::Email;
        let _age = Column::Age;
        let _active = Column::Active;
    }

    #[test]
    fn test_column_enum_equality() {
        // Verify Column enum supports equality
        assert_eq!(Column::Id, Column::Id);
        assert_ne!(Column::Id, Column::Name);
    }

    #[test]
    fn test_column_enum_hash() {
        // Verify Column enum can be hashed
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(Column::Id, "id");
        map.insert(Column::Name, "name");
        assert_eq!(map.get(&Column::Id), Some(&"id"));
    }

    // ============================================================================
    // Edge Cases: Model with Option Fields
    // ============================================================================

    #[test]
    fn test_model_with_all_option_none() {
        // Verify Model handles all Option fields as None
        let model = EdgeCaseUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: None,
            active: false,
        };

        assert_eq!(model.age, None);
    }

    #[test]
    fn test_model_with_all_option_some() {
        // Verify Model handles all Option fields as Some
        let model = EdgeCaseUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: Some(30),
            active: true,
        };

        assert_eq!(model.age, Some(30));
    }

    // ============================================================================
    // Edge Cases: ModelTrait
    // ============================================================================

    #[test]
    fn test_model_trait_get_all_columns() {
        // Verify ModelTrait::get() works for all columns
        use lifeguard::ModelTrait;

        let model = EdgeCaseUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: Some(30),
            active: true,
        };

        let _id_value = model.get(Column::Id);
        let _name_value = model.get(Column::Name);
        let _email_value = model.get(Column::Email);
        let _age_value = model.get(Column::Age);
        let _active_value = model.get(Column::Active);

        // Just verify it compiles
    }

    #[test]
    fn test_model_trait_primary_key() {
        // Verify get_primary_key_value() works
        use lifeguard::ModelTrait;

        let model = EdgeCaseUserModel {
            id: 42,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: None,
            active: false,
        };

        let pk_value = model.get_primary_key_value();
        assert!(matches!(pk_value, sea_query::Value::Int(_)));
    }

    // ============================================================================
    // Option Type Detection Tests - Verifies the fix for Option<T> field handling
    // ============================================================================

    #[test]
    fn test_option_i32_detection_some() {
        // CRITICAL TEST: Verify Option<i32> generates Int values, not String
        // This test verifies the fix for the bug where Option<T> fields were
        // incorrectly generating String(None) instead of properly-typed values
        use lifeguard::ModelTrait;

        let model = EdgeCaseUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: Some(42),
            active: true,
        };

        let age_value = model.get(Column::Age);
        
        // Verify it's Int(Some(42)), not String(None)
        match age_value {
            sea_query::Value::Int(Some(42)) => {
                // Correct! Option<i32> with Some(42) generates Int(Some(42))
            }
            sea_query::Value::String(_) => {
                panic!("BUG: Option<i32> generated String value instead of Int! This indicates the Option detection fix is broken.");
            }
            sea_query::Value::Int(Some(v)) => {
                panic!("Option<i32> generated Int(Some({})) but expected Int(Some(42))", v);
            }
            _ => {
                panic!("Option<i32> generated unexpected value type: {:?}", age_value);
            }
        }
    }

    #[test]
    fn test_option_i32_detection_none() {
        // CRITICAL TEST: Verify Option<i32> with None generates Int(None), not String(None)
        use lifeguard::ModelTrait;

        let model = EdgeCaseUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: None,
            active: true,
        };

        let age_value = model.get(Column::Age);
        
        // Verify it's Int(None), not String(None)
        match age_value {
            sea_query::Value::Int(None) => {
                // Correct! Option<i32> with None generates Int(None)
            }
            sea_query::Value::String(_) => {
                panic!("BUG: Option<i32> with None generated String(None) instead of Int(None)! This indicates the Option detection fix is broken.");
            }
            _ => {
                panic!("Option<i32> with None generated unexpected value type: {:?}", age_value);
            }
        }
    }

    #[test]
    fn test_option_detection_uses_correct_type() {
        // Verify that Option types are detected using segments.last() (the fix)
        // and generate correctly-typed values based on the inner type
        use lifeguard::ModelTrait;

        // Test with Some value
        let model_some = EdgeCaseUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: Some(100),
            active: true,
        };

        let age_value_some = model_some.get(Column::Age);
        assert!(matches!(age_value_some, sea_query::Value::Int(Some(100))), 
            "Option<i32> with Some(100) should generate Int(Some(100)), got: {:?}", age_value_some);

        // Test with None value
        let model_none = EdgeCaseUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: None,
            active: true,
        };

        let age_value_none = model_none.get(Column::Age);
        assert!(matches!(age_value_none, sea_query::Value::Int(None)), 
            "Option<i32> with None should generate Int(None), got: {:?}", age_value_none);
    }

    #[test]
    fn test_non_option_types_still_work() {
        // Regression test: Verify non-Option types still work correctly
        use lifeguard::ModelTrait;

        let model = EdgeCaseUserModel {
            id: 42,
            name: "Test Name".to_string(),
            email: "test@example.com".to_string(),
            age: Some(30),
            active: true,
        };

        // Verify i32 (non-Option) generates Int
        let id_value = model.get(Column::Id);
        assert!(matches!(id_value, sea_query::Value::Int(Some(42))), 
            "i32 field should generate Int(Some(42)), got: {:?}", id_value);

        // Verify String (non-Option) generates String
        let name_value = model.get(Column::Name);
        assert!(matches!(name_value, sea_query::Value::String(Some(_))), 
            "String field should generate String(Some(_)), got: {:?}", name_value);

        // Verify bool (non-Option) generates Bool
        let active_value = model.get(Column::Active);
        assert!(matches!(active_value, sea_query::Value::Bool(Some(true))), 
            "bool field should generate Bool(Some(true)), got: {:?}", active_value);
    }

    #[test]
    fn test_option_detection_does_not_affect_primary_key() {
        // Verify that Option detection fix doesn't break primary key handling
        use lifeguard::ModelTrait;

        let model = EdgeCaseUserModel {
            id: 999,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: Some(25),
            active: false,
        };

        let pk_value = model.get_primary_key_value();
        
        // Primary key is i32 (non-Option), should generate Int
        assert!(matches!(pk_value, sea_query::Value::Int(Some(999))), 
            "Primary key i32 should generate Int(Some(999)), got: {:?}", pk_value);
    }
}
