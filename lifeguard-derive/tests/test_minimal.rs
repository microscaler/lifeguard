//! Minimal test to isolate E0223 error

use lifeguard_derive::{LifeModel, FromRow, DeriveLifeModelTrait};

#[test]
fn test_minimal() {
    #[derive(LifeModel)]
    #[table_name = "test_minimal"]
    struct TestMinimal {
        #[primary_key]
        id: i32,
        name: String,
    }
    
    // Apply FromRow to Model
    #[derive(FromRow)]
    struct TestMinimalModel {
        id: i32,
        name: String,
    }
    
    // Apply LifeModelTrait to Entity
    #[derive(DeriveLifeModelTrait)]
    struct Entity;
    
    // Just verify Entity exists
    let _entity = Entity;
}
