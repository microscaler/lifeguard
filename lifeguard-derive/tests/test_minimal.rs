//! Minimal test to isolate E0223 error

use lifeguard_derive::{LifeModel, FromRow};

#[test]
fn test_minimal() {
    #[derive(LifeModel)]
    #[table_name = "test_minimal"]
    struct TestMinimal {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Apply FromRow to the generated Model (required for query execution)
    #[derive(FromRow)]
    struct TestMinimalModel {
        id: i32,
        name: String,
    }
    
    // LifeModel generates Entity, Model, Column, PrimaryKey, and LifeModelTrait
    // Just verify Entity exists and LifeModelTrait is implemented
    let _entity = Entity;
    
    // Verify we can use Entity::find() (requires LifeModelTrait)
    // This should compile now that LifeModelTrait is generated in the same expansion
    let _query = Entity::find();
}
