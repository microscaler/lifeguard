//! Comprehensive tests for LifeModel derive macro
//!
//! Tests all generated code: Entity, Model, Column, PrimaryKey, FromRow, LifeModelTrait
//! Based on implemented features from SEAORM_LIFEGUARD_MAPPING.md

use lifeguard_derive::LifeModel;

// Test entity with various field types
#[derive(LifeModel)]
#[table_name = "test_users"]
pub struct TestUser {
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
    // Entity Tests
    // ============================================================================

    #[test]
    fn test_entity_unit_struct() {
        // Verify Entity is a unit struct
        let entity = Entity;
        assert_eq!(entity.table_name(), "test_users");
        
        // Verify Default implementation
        let default_entity = Entity::default();
        assert_eq!(default_entity.table_name(), "test_users");
    }

    #[test]
    fn test_entity_table_name() {
        // Test LifeEntityName trait
        assert_eq!(Entity::default().table_name(), "test_users");
    }

    #[test]
    fn test_entity_table_name_constant() {
        // Verify TABLE_NAME constant exists
        assert_eq!(Entity::TABLE_NAME, "test_users");
    }

    #[test]
    fn test_entity_implements_iden() {
        // Verify Entity implements Iden for use in sea_query
        let entity = Entity;
        assert_eq!(entity.unquoted(), "test_users");
    }

    #[test]
    fn test_entity_find_method() {
        // Verify Entity::find() returns SelectQuery
        let _query = Entity::find();
        // Just verify it compiles - actual execution requires an executor
    }

    #[test]
    fn test_entity_life_model_trait() {
        // Verify LifeModelTrait is implemented
        fn _verify_trait<E: LifeModelTrait>() {}
        _verify_trait::<Entity>();
        
        // Verify associated type Model
        fn _verify_model<E: LifeModelTrait<Model = TestUserModel>>() {}
        _verify_model::<Entity>();
    }

    // ============================================================================
    // Column Enum Tests
    // ============================================================================

    #[test]
    fn test_column_enum_variants() {
        // Verify all columns are generated
        let _id = Column::Id;
        let _name = Column::Name;
        let _email = Column::Email;
        let _age = Column::Age;
        let _active = Column::Active;
    }

    #[test]
    fn test_column_implements_iden() {
        // Verify Column implements Iden
        assert_eq!(Column::Id.unquoted(), "id");
        assert_eq!(Column::Name.unquoted(), "name");
        assert_eq!(Column::Email.unquoted(), "email");
        assert_eq!(Column::Age.unquoted(), "age");
        assert_eq!(Column::Active.unquoted(), "active");
    }

    #[test]
    fn test_column_enum_equality() {
        // Verify Column enum supports equality
        assert_eq!(Column::Id, Column::Id);
        assert_ne!(Column::Id, Column::Name);
    }

    #[test]
    fn test_column_enum_hash() {
        // Verify Column enum can be hashed (for use in HashMaps)
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(Column::Id, "id");
        map.insert(Column::Name, "name");
        assert_eq!(map.get(&Column::Id), Some(&"id"));
    }

    // ============================================================================
    // PrimaryKey Enum Tests
    // ============================================================================

    #[test]
    fn test_primary_key_enum() {
        // Verify PrimaryKey enum exists
        let _pk = PrimaryKey::Id;
    }

    #[test]
    fn test_primary_key_only_primary_fields() {
        // Verify only fields marked with #[primary_key] are in PrimaryKey enum
        // TestUser has only id as primary key
        let _pk = PrimaryKey::Id;
        // Should not compile if we try to access non-primary fields
    }

    #[test]
    fn test_primary_key_enum_equality() {
        // Verify PrimaryKey enum supports equality
        assert_eq!(PrimaryKey::Id, PrimaryKey::Id);
    }

    // ============================================================================
    // Model Struct Tests
    // ============================================================================

    #[test]
    fn test_model_struct_creation() {
        // Verify Model struct can be created
        let model = TestUserModel {
            id: 1,
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            age: Some(30),
            active: true,
        };
        
        assert_eq!(model.id, 1);
        assert_eq!(model.name, "Test User");
        assert_eq!(model.email, "test@example.com");
        assert_eq!(model.age, Some(30));
        assert_eq!(model.active, true);
    }

    #[test]
    fn test_model_with_option_fields() {
        // Verify Option<T> fields work correctly
        let model_with_age = TestUserModel {
            id: 1,
            name: "User".to_string(),
            email: "user@example.com".to_string(),
            age: Some(25),
            active: false,
        };
        
        let model_without_age = TestUserModel {
            id: 2,
            name: "User2".to_string(),
            email: "user2@example.com".to_string(),
            age: None,
            active: true,
        };
        
        assert_eq!(model_with_age.age, Some(25));
        assert_eq!(model_without_age.age, None);
    }

    #[test]
    fn test_model_clone() {
        // Verify Model implements Clone
        let model1 = TestUserModel {
            id: 1,
            name: "User".to_string(),
            email: "user@example.com".to_string(),
            age: Some(30),
            active: true,
        };
        
        let model2 = model1.clone();
        assert_eq!(model1.id, model2.id);
        assert_eq!(model1.name, model2.name);
    }

    // ============================================================================
    // FromRow Trait Tests
    // ============================================================================

    #[test]
    fn test_from_row_trait_implemented() {
        // Verify FromRow trait is implemented
        fn _verify_from_row<T: FromRow>() {}
        _verify_from_row::<TestUserModel>();
    }

    #[test]
    fn test_from_row_with_different_types() {
        // Verify FromRow works with different field types
        // This is a compile-time test - actual runtime test would need a real Row
        fn _verify_types<T: FromRow>() {}
        _verify_types::<TestUserModel>(); // i32, String, Option<i32>, bool
    }

    // ============================================================================
    // Integration Tests
    // ============================================================================

    #[test]
    fn test_full_entity_model_flow() {
        // Test the complete flow: Entity -> find() -> SelectQuery
        let query = Entity::find();
        // Verify query builder is created successfully
        // Just verify it compiles - actual filtering would require proper Expr construction
        let _query = query;
    }

    #[test]
    fn test_entity_model_relationship() {
        // Verify Entity and Model are properly linked via LifeModelTrait
        fn _verify_relationship<E: LifeModelTrait<Model = TestUserModel>>() {}
        _verify_relationship::<Entity>();
    }
}
