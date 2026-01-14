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
    
    // Just verify Entity exists
    let _entity = Entity;
}
