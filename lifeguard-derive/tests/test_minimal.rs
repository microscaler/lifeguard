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
    // Step 1: Just verify Entity exists (no trait usage yet)
    let _entity = Entity;
    
    // Step 2: Verify Model exists
    let _model = TestMinimalModel {
        id: 1,
        name: "test".to_string(),
    };
    
    // Step 3: Verify Column enum exists
    let _column = Column::Id;
    
    // Step 4: Verify PrimaryKey enum exists
    let _pk = PrimaryKey::Id;
}
