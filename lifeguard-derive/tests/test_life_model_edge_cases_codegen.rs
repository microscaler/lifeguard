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
}
