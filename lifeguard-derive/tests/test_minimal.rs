//! Minimal test to isolate E0223 error

use lifeguard_derive::LifeModel;

#[test]
fn test_minimal() {
    #[derive(LifeModel)]
    #[table_name = "test_minimal"]
    struct TestMinimal {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // LifeModel generates Entity, Model, Column, PrimaryKey, and LifeModelTrait
    // Just verify Entity exists and LifeModelTrait is implemented
    let _entity = Entity;
    
    // Verify we can use Entity::find() (requires LifeModelTrait)
    let _query = Entity::find();
}
