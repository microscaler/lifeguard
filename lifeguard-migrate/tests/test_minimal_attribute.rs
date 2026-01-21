//! Minimal test to debug attribute parsing issue

use lifeguard_derive::LifeModel;
use lifeguard::LifeModelTrait;

#[test]
fn test_minimal_attribute_parsing() {
    #[derive(LifeModel)]
    #[table_name = "test_minimal"]
    pub struct TestMinimal {
        #[primary_key]
        pub id: i32,
        
        #[column_type = "VARCHAR(50)"]
        pub name: String,
        
        #[default_value = "0"]
        pub count: i32,
    }
    
    // Access Entity directly (macro generates it in the same scope)
    use lifeguard::LifeModelTrait;
    
    // Check if attributes are parsed
    let name_col = <Entity as LifeModelTrait>::Column::Name;
    let name_def = name_col.column_def();
    
    println!("name column_type = {:?}", name_def.column_type);
    println!("name should be Some(\"VARCHAR(50)\")");
    
    assert_eq!(name_def.column_type, Some("VARCHAR(50)".to_string()), 
        "column_type attribute should be parsed");
    
    let count_col = <Entity as LifeModelTrait>::Column::Count;
    let count_def = count_col.column_def();
    
    println!("count default_value = {:?}", count_def.default_value);
    println!("count should be Some(\"0\")");
    
    assert_eq!(count_def.default_value, Some("0".to_string()),
        "default_value attribute should be parsed");
}
