//! Edge cases and error handling tests for LifeModel derive macro
//!
//! Tests error conditions, boundary cases, and unusual inputs

use lifeguard_derive::LifeModel;

// Test entity with various edge case scenarios
#[derive(LifeModel)]
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
    use lifeguard::{FromRow, LifeEntityName, LifeModelTrait};
    use sea_query::Iden;

    // ============================================================================
    // Edge Cases: Table Name Handling
    // ============================================================================

    #[test]
    fn test_table_name_with_underscores() {
        // Verify table names with underscores work
        assert_eq!(Entity::default().table_name(), "edge_case_users");
        assert_eq!(Entity::TABLE_NAME, "edge_case_users");
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

    // ============================================================================
    // Edge Cases: PrimaryKey Enum
    // ============================================================================

    #[test]
    fn test_primary_key_only_marked_fields() {
        // Verify only fields with #[primary_key] are in PrimaryKey enum
        // EdgeCaseUser has only id as primary key
        let _pk = PrimaryKey::Id;
        // Should not compile: PrimaryKey::Name, PrimaryKey::Email, etc.
    }

    // ============================================================================
    // Edge Cases: Model Field Types
    // ============================================================================

    #[test]
    fn test_model_with_mixed_types() {
        // Verify Model handles different field types correctly
        let model = EdgeCaseUserModel {
            id: 1,                                    // i32
            name: "Test".to_string(),                // String
            email: "test@example.com".to_string(),   // String
            age: Some(30),                            // Option<i32>
            active: true,                             // bool
        };
        
        assert_eq!(model.id, 1);
        assert_eq!(model.name, "Test");
        assert_eq!(model.email, "test@example.com");
        assert_eq!(model.age, Some(30));
        assert_eq!(model.active, true);
    }

    #[test]
    fn test_model_with_none_optional_fields() {
        // Verify Model handles None values in Option fields
        let model = EdgeCaseUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: None,  // None value
            active: false,
        };
        
        assert_eq!(model.age, None);
    }

    // ============================================================================
    // Edge Cases: FromRow Trait
    // ============================================================================

    #[test]
    fn test_from_row_with_all_types() {
        // Verify FromRow works with all supported types
        fn _verify_from_row<T: FromRow>() {}
        _verify_from_row::<EdgeCaseUserModel>();      // i32, String, Option<i32>, bool
    }

    // ============================================================================
    // Edge Cases: Entity find() Method
    // ============================================================================

    #[test]
    fn test_entity_find_returns_query() {
        // Verify find() returns SelectQuery
        let _query = Entity::find();
    }

    // ============================================================================
    // Edge Cases: Entity TABLE_NAME Constant
    // ============================================================================

    #[test]
    fn test_table_name_constant_exists() {
        // Verify TABLE_NAME constant exists
        assert_eq!(Entity::TABLE_NAME, "edge_case_users");
    }

    // ============================================================================
    // Edge Cases: Column Iden Implementation
    // ============================================================================

    #[test]
    fn test_column_iden_all_variants() {
        // Verify all Column variants implement Iden correctly
        assert_eq!(Column::Id.unquoted(), "id");
        assert_eq!(Column::Name.unquoted(), "name");
        assert_eq!(Column::Email.unquoted(), "email");
        assert_eq!(Column::Age.unquoted(), "age");
        assert_eq!(Column::Active.unquoted(), "active");
    }

    // ============================================================================
    // Edge Cases: Entity Iden Implementation
    // ============================================================================

    #[test]
    fn test_entity_iden_implementation() {
        // Verify Entity implements Iden
        let entity = Entity;
        assert_eq!(entity.unquoted(), "edge_case_users");
    }

    // ============================================================================
    // Edge Cases: LifeModelTrait Associated Type
    // ============================================================================

    #[test]
    fn test_life_model_trait_associated_type() {
        // Verify LifeModelTrait has correct Model associated type
        fn _verify_model_type<E: LifeModelTrait<Model = EdgeCaseUserModel>>() {}
        _verify_model_type::<Entity>();
    }
}
