//! Comprehensive tests using codegen (not procedural macros)
//!
//! This test uses lifeguard-codegen to generate Entity, Model, Column, etc.
//! before compilation, avoiding E0223 errors from procedural macros.

// Include generated code (filename is lowercase, no underscores)
include!("generated/testuser.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use lifeguard::{FromRow, LifeEntityName, LifeModelTrait};

    // ============================================================================
    // Entity Tests
    // ============================================================================

    #[test]
    fn test_entity_unit_struct() {
        // Verify TestUser (Entity) is a unit struct
        let entity = TestUser;
        assert_eq!(entity.table_name(), "test_users");

        // Verify Default implementation
        let default_entity = TestUser::default();
        assert_eq!(default_entity.table_name(), "test_users");
    }

    #[test]
    fn test_entity_table_name_constant() {
        // Verify TABLE_NAME constant exists
        assert_eq!(TestUser::TABLE_NAME, "test_users");
    }

    #[test]
    fn test_entity_implements_life_entity_name() {
        // Verify LifeEntityName trait is implemented
        fn _verify_trait<E: LifeEntityName>() {}
        _verify_trait::<TestUser>();
    }

    #[test]
    fn test_entity_implements_iden() {
        // Verify Iden trait is implemented for Entity
        use sea_query::Iden;
        assert_eq!(TestUser.unquoted(), "test_users");
    }

    // ============================================================================
    // Column Enum Tests
    // ============================================================================

    #[test]
    fn test_column_enum_variants() {
        // Verify all column variants exist
        use sea_query::Iden;
        let _id = Column::Id;
        let _name = Column::Name;
        let _email = Column::Email;
        let _age = Column::Age;
        let _active = Column::Active;
    }

    #[test]
    fn test_column_implements_iden() {
        // Verify Column implements Iden
        use sea_query::Iden;
        assert_eq!(Column::Id.unquoted(), "id");
        assert_eq!(Column::Name.unquoted(), "name");
        assert_eq!(Column::Email.unquoted(), "email");
        assert_eq!(Column::Age.unquoted(), "age");
        assert_eq!(Column::Active.unquoted(), "active");
    }

    // ============================================================================
    // PrimaryKey Enum Tests
    // ============================================================================

    #[test]
    fn test_primary_key_enum() {
        // Verify PrimaryKey enum exists with correct variant
        let _pk = PrimaryKey::Id;
    }

    // ============================================================================
    // Model Struct Tests
    // ============================================================================

    #[test]
    fn test_model_struct_fields() {
        // Verify Model struct has all fields
        let model = TestUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: Some(30),
            active: true,
        };

        assert_eq!(model.id, 1);
        assert_eq!(model.name, "Test");
        assert_eq!(model.email, "test@example.com");
        assert_eq!(model.age, Some(30));
        assert_eq!(model.active, true);
    }

    #[test]
    fn test_model_with_option_none() {
        // Verify Model handles Option::None correctly
        let model = TestUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: None,
            active: false,
        };

        assert_eq!(model.age, None);
    }

    #[test]
    fn test_model_implements_from_row() {
        // Verify FromRow trait is implemented
        fn _verify_from_row<T: FromRow>() {}
        _verify_from_row::<TestUserModel>();
    }

    #[test]
    fn test_model_implements_model_trait() {
        // Verify ModelTrait is implemented
        use lifeguard::ModelTrait;

        let model = TestUserModel {
            id: 1,
            name: "Test".to_string(),
            email: "test@example.com".to_string(),
            age: Some(30),
            active: true,
        };

        // Test get() method
        let id_value = model.get(Column::Id);
        let name_value = model.get(Column::Name);

        // Test get_primary_key_value()
        let pk_value = model.get_primary_key_value();

        // Just verify methods exist and compile
        assert!(matches!(id_value, sea_query::Value::Int(_)));
        assert!(matches!(name_value, sea_query::Value::String(_)));
        assert!(matches!(pk_value, sea_query::Value::Int(_)));
    }

    // ============================================================================
    // LifeModelTrait Tests
    // ============================================================================

    #[test]
    fn test_life_model_trait_implemented() {
        // Verify LifeModelTrait is implemented
        fn _verify_trait<E: LifeModelTrait>() {}
        _verify_trait::<TestUser>();
    }

    #[test]
    fn test_life_model_trait_associated_types() {
        // Verify associated types are correct
        fn _verify_model_type<E: LifeModelTrait<Model = TestUserModel>>() {}
        _verify_model_type::<TestUser>();

        // Verify Column associated type
        let _column: <TestUser as LifeModelTrait>::Column = Column::Id;
    }

    #[test]
    fn test_find_method() {
        // Verify find() method exists and returns SelectQuery
        let _query = TestUser::find();
        // Just verify it compiles
    }
}
