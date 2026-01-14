//! Comprehensive tests for the code generation writer
//!
//! Tests verify that generated code correctly handles:
//! - Different field types (i32, i64, String, bool, Option<T>)
//! - Option<T> extraction (the bug we just fixed)
//! - Primary key handling
//! - ModelTrait::get() method generation

use lifeguard_codegen::{EntityDefinition, EntityWriter, FieldDefinition};
use syn::parse_str;

fn create_test_entity() -> EntityDefinition {
    EntityDefinition {
        name: parse_str::<syn::Ident>("TestEntity").unwrap(),
        table_name: "test_entities".to_string(),
        fields: vec![
            FieldDefinition {
                name: parse_str::<syn::Ident>("id").unwrap(),
                ty: parse_str::<syn::Type>("i32").unwrap(),
                is_primary_key: true,
                column_name: None,
                is_nullable: false,
                is_auto_increment: true,
            },
            FieldDefinition {
                name: parse_str::<syn::Ident>("age").unwrap(),
                ty: parse_str::<syn::Type>("Option<i32>").unwrap(),
                is_primary_key: false,
                column_name: None,
                is_nullable: true,
                is_auto_increment: false,
            },
            FieldDefinition {
                name: parse_str::<syn::Ident>("name").unwrap(),
                ty: parse_str::<syn::Type>("Option<String>").unwrap(),
                is_primary_key: false,
                column_name: None,
                is_nullable: true,
                is_auto_increment: false,
            },
        ],
    }
}

#[test]
fn test_option_i32_generation() {
    let entity = create_test_entity();
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify Option<i32> is handled correctly in ModelTrait::get()
    // Should generate: self.age.map(|v| sea_query::Value::Int(Some(v))).unwrap_or(sea_query::Value::Int(None))
    // Find the get() method section
    let get_start = code.find("fn get(&self").expect("Should find get method");
    let get_end = code[get_start..].find("fn get_primary_key_value").unwrap_or(code.len() - get_start);
    let get_method = &code[get_start..get_start + get_end];
    
    // Verify it uses Int, not String for Option<i32>
    // Check for key components (allowing for multi-line formatting)
    // Note: The field might be referenced as just "age" in some contexts, so check for both
    let has_age_field = get_method.contains("self.age") || get_method.contains(".age");
    assert!(
        has_age_field && 
        get_method.contains("Int(Some") &&
        get_method.contains("Int(None)"),
        "Option<i32> should generate Int mapping in ModelTrait::get(). get_method snippet: {}",
        &get_method.chars().take(200).collect::<String>()
    );
    
    // The critical check: verify it does NOT use String(None) for Option<i32> (the bug we fixed)
    // Extract the Column::Age section to verify it uses Int, not String
    if let Some(age_start) = get_method.find("Column::Age") {
        // Find the end of the age match arm (next Column:: or closing brace)
        let age_end = get_method[age_start..]
            .find("Column::Name")
            .or_else(|| get_method[age_start..].find("},"))
            .unwrap_or(200);
        let age_section = &get_method[age_start..age_start + age_end.min(300)];
        
        // The bug was generating String(None) for Option<i32> - verify this is fixed
        assert!(
            age_section.contains("Int(None)") && !age_section.contains("String(None)"),
            "Option<i32> should use Int(None), not String(None) - this was the bug! Found:\n{}",
            age_section
        );
    }
}

#[test]
fn test_option_string_generation() {
    let entity = create_test_entity();
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify Option<String> is handled correctly in ModelTrait::get()
    let get_start = code.find("fn get(&self").expect("Should find get method");
    let get_end = code[get_start..].find("fn get_primary_key_value").unwrap_or(code.len() - get_start);
    let get_method = &code[get_start..get_start + get_end];
    
    // Verify it uses String with as_ref() for Option<String>
    let has_name = get_method.contains("self.name") || get_method.contains(".name");
    assert!(
        has_name && 
        get_method.contains("as_ref()") &&
        get_method.contains("String(Some") &&
        get_method.contains("v.clone()") &&
        get_method.contains("String(None)"),
        "Option<String> should generate String mapping with as_ref() in ModelTrait::get()"
    );
}

#[test]
fn test_primary_key_i32_generation() {
    let entity = create_test_entity();
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify primary key i32 is handled correctly
    assert!(
        code.contains("sea_query::Value::Int(Some(self.id))"),
        "Primary key i32 should generate Int(Some(value))"
    );
}

#[test]
fn test_all_field_types() {
    use syn::parse_str;
    
    let entity = EntityDefinition {
        name: parse_str::<syn::Ident>("AllTypes").unwrap(),
        table_name: "all_types".to_string(),
        fields: vec![
            FieldDefinition {
                name: parse_str::<syn::Ident>("id").unwrap(),
                ty: parse_str::<syn::Type>("i32").unwrap(),
                is_primary_key: true,
                column_name: None,
                is_nullable: false,
                is_auto_increment: false,
            },
            FieldDefinition {
                name: parse_str::<syn::Ident>("big_id").unwrap(),
                ty: parse_str::<syn::Type>("i64").unwrap(),
                is_primary_key: false,
                column_name: None,
                is_nullable: false,
                is_auto_increment: false,
            },
            FieldDefinition {
                name: parse_str::<syn::Ident>("small_id").unwrap(),
                ty: parse_str::<syn::Type>("i16").unwrap(),
                is_primary_key: false,
                column_name: None,
                is_nullable: false,
                is_auto_increment: false,
            },
            FieldDefinition {
                name: parse_str::<syn::Ident>("name").unwrap(),
                ty: parse_str::<syn::Type>("String").unwrap(),
                is_primary_key: false,
                column_name: None,
                is_nullable: false,
                is_auto_increment: false,
            },
            FieldDefinition {
                name: parse_str::<syn::Ident>("active").unwrap(),
                ty: parse_str::<syn::Type>("bool").unwrap(),
                is_primary_key: false,
                column_name: None,
                is_nullable: false,
                is_auto_increment: false,
            },
            FieldDefinition {
                name: parse_str::<syn::Ident>("age").unwrap(),
                ty: parse_str::<syn::Type>("Option<i32>").unwrap(),
                is_primary_key: false,
                column_name: None,
                is_nullable: true,
                is_auto_increment: false,
            },
            FieldDefinition {
                name: parse_str::<syn::Ident>("email").unwrap(),
                ty: parse_str::<syn::Type>("Option<String>").unwrap(),
                is_primary_key: false,
                column_name: None,
                is_nullable: true,
                is_auto_increment: false,
            },
            FieldDefinition {
                name: parse_str::<syn::Ident>("score").unwrap(),
                ty: parse_str::<syn::Type>("Option<f64>").unwrap(),
                is_primary_key: false,
                column_name: None,
                is_nullable: true,
                is_auto_increment: false,
            },
        ],
    };
    
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify all types are handled correctly
    assert!(code.contains("sea_query::Value::Int(Some(self.id))"), "i32 should generate Int");
    assert!(code.contains("sea_query::Value::BigInt(Some(self.big_id))"), "i64 should generate BigInt");
    assert!(code.contains("sea_query::Value::SmallInt(Some(self.small_id))"), "i16 should generate SmallInt");
    assert!(code.contains("sea_query::Value::String(Some(self.name.clone()))"), "String should generate String");
    assert!(code.contains("sea_query::Value::Bool(Some(self.active))"), "bool should generate Bool");
    
    // Verify Option types in ModelTrait::get() method
    let get_start = code.find("fn get(&self").expect("Should find get method");
    let get_end = code[get_start..].find("fn get_primary_key_value").unwrap_or(code.len() - get_start);
    let get_method = &code[get_start..get_start + get_end];
    
    assert!(
        (get_method.contains("self.age") || get_method.contains(".age")) && get_method.contains("Int(Some"),
        "Option<i32> should map to Int"
    );
    assert!(
        (get_method.contains("self.email") || get_method.contains(".email")) && get_method.contains("as_ref()") && get_method.contains("String(Some"),
        "Option<String> should map to String"
    );
    assert!(
        (get_method.contains("self.score") || get_method.contains(".score")) && get_method.contains("Double(Some"),
        "Option<f64> should map to Double"
    );
}

#[test]
fn test_column_enum_generation() {
    let entity = create_test_entity();
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify Column enum is generated with correct variants
    assert!(code.contains("pub enum Column"), "Should generate Column enum");
    assert!(code.contains("Id,"), "Should have Id variant");
    assert!(code.contains("Age,"), "Should have Age variant");
    assert!(code.contains("Name,"), "Should have Name variant");
}

#[test]
fn test_model_struct_generation() {
    let entity = create_test_entity();
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify Model struct is generated with correct fields
    assert!(code.contains("pub struct TestEntityModel"), "Should generate Model struct");
    assert!(code.contains("pub id: i32,"), "Should have id field");
    assert!(code.contains("pub age: Option<i32>,"), "Should have age field as Option<i32>");
    assert!(code.contains("pub name: Option<String>,"), "Should have name field as Option<String>");
}

#[test]
fn test_from_row_generation() {
    let entity = create_test_entity();
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify FromRow implementation uses try_get()? for ALL fields (matching proc-macro behavior)
    // This ensures graceful error handling instead of panics on NULL values, missing columns, or type mismatches
    assert!(code.contains("impl FromRow for TestEntityModel"), "Should implement FromRow");
    assert!(code.contains("row.try_get::<&str, i32>(\"id\")?"), "Required field should use try_get()? (not get() which panics)");
    assert!(code.contains("row.try_get::<&str, Option<i32>>(\"age\")?"), "Option<i32> should use try_get()?");
    assert!(code.contains("row.try_get::<&str, Option<String>>(\"name\")?"), "Option<String> should use try_get()?");
    
    // Verify it does NOT use row.get() which panics on errors
    let from_row_start = code.find("impl FromRow").expect("Should find FromRow impl");
    let from_row_end = code[from_row_start..].find("impl ModelTrait").unwrap_or(code.len() - from_row_start);
    let from_row_impl = &code[from_row_start..from_row_start + from_row_end];
    
    assert!(
        !from_row_impl.contains("row.get::<&str, i32>") && !from_row_impl.contains("row.get::<&str, String>"),
        "FromRow should NOT use row.get() which panics - should use row.try_get()? for all fields"
    );
}

#[test]
fn test_life_model_trait_generation() {
    let entity = create_test_entity();
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify LifeModelTrait is implemented correctly
    assert!(code.contains("impl LifeModelTrait for TestEntity"), "Should implement LifeModelTrait");
    assert!(code.contains("type Model = TestEntityModel"), "Should set Model type");
    assert!(code.contains("type Column = Column"), "Should set Column type");
}

#[test]
fn test_primary_key_enum_generation() {
    let entity = create_test_entity();
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify PrimaryKey enum is generated
    assert!(code.contains("pub enum PrimaryKey"), "Should generate PrimaryKey enum");
    assert!(code.contains("Id,"), "Should have Id variant in PrimaryKey");
}

#[test]
fn test_table_name_constant() {
    let entity = create_test_entity();
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify TABLE_NAME constant is generated
    assert!(code.contains("pub const TABLE_NAME: &'static str"), "Should generate TABLE_NAME constant");
    assert!(code.contains("\"test_entities\""), "Should have correct table name");
}

#[test]
fn test_entity_name_implementation() {
    let entity = create_test_entity();
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify LifeEntityName is implemented
    assert!(code.contains("impl LifeEntityName for TestEntity"), "Should implement LifeEntityName");
    assert!(code.contains("fn table_name(&self) -> &'static str"), "Should have table_name method");
}

#[test]
fn test_iden_implementations() {
    let entity = create_test_entity();
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify Iden is implemented for Entity and Column
    assert!(code.contains("impl sea_query::Iden for TestEntity"), "Should implement Iden for Entity");
    assert!(code.contains("impl sea_query::Iden for Column"), "Should implement Iden for Column");
}

#[test]
fn test_option_f64_generation() {
    use syn::parse_str;
    
    let entity = EntityDefinition {
        name: parse_str::<syn::Ident>("TestFloat").unwrap(),
        table_name: "test_floats".to_string(),
        fields: vec![
            FieldDefinition {
                name: parse_str::<syn::Ident>("id").unwrap(),
                ty: parse_str::<syn::Type>("i32").unwrap(),
                is_primary_key: true,
                column_name: None,
                is_nullable: false,
                is_auto_increment: false,
            },
            FieldDefinition {
                name: parse_str::<syn::Ident>("price").unwrap(),
                ty: parse_str::<syn::Type>("Option<f64>").unwrap(),
                is_primary_key: false,
                column_name: None,
                is_nullable: true,
                is_auto_increment: false,
            },
        ],
    };
    
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify Option<f64> is handled correctly in ModelTrait::get()
    let get_start = code.find("fn get(&self").expect("Should find get method");
    let get_end = code[get_start..].find("fn get_primary_key_value").unwrap_or(code.len() - get_start);
    let get_method = &code[get_start..get_start + get_end];
    
    let has_price = get_method.contains("self.price") || get_method.contains(".price");
    assert!(
        has_price && 
        get_method.contains("Double(Some") &&
        get_method.contains("Double(None)"),
        "Option<f64> should generate Double value mapping"
    );
}

#[test]
fn test_option_bool_generation() {
    use syn::parse_str;
    
    let entity = EntityDefinition {
        name: parse_str::<syn::Ident>("TestBool").unwrap(),
        table_name: "test_bools".to_string(),
        fields: vec![
            FieldDefinition {
                name: parse_str::<syn::Ident>("id").unwrap(),
                ty: parse_str::<syn::Type>("i32").unwrap(),
                is_primary_key: true,
                column_name: None,
                is_nullable: false,
                is_auto_increment: false,
            },
            FieldDefinition {
                name: parse_str::<syn::Ident>("verified").unwrap(),
                ty: parse_str::<syn::Type>("Option<bool>").unwrap(),
                is_primary_key: false,
                column_name: None,
                is_nullable: true,
                is_auto_increment: false,
            },
        ],
    };
    
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // Verify Option<bool> is handled correctly in ModelTrait::get()
    let get_start = code.find("fn get(&self").expect("Should find get method");
    let get_end = code[get_start..].find("fn get_primary_key_value").unwrap_or(code.len() - get_start);
    let get_method = &code[get_start..get_start + get_end];
    
    let has_verified = get_method.contains("self.verified") || get_method.contains(".verified");
    assert!(
        has_verified && 
        get_method.contains("Bool(Some") &&
        get_method.contains("Bool(None)"),
        "Option<bool> should generate Bool value mapping"
    );
}

#[test]
fn test_code_generation_does_not_contain_bug() {
    // This test specifically verifies the bug we fixed:
    // Option<T> fields should NOT return String(None) for all Option types
    let entity = create_test_entity();
    let writer = EntityWriter::new();
    let code = writer.generate_entity_code(&entity, true).unwrap();
    
    // The bug was: Option<i32> was generating String(None) instead of Int(None)
    // Verify this is fixed by checking the ModelTrait::get() method
    let get_start = code.find("fn get(&self").expect("Should find get method");
    let get_end = code[get_start..].find("fn get_primary_key_value").unwrap_or(code.len() - get_start);
    let get_method = &code[get_start..get_start + get_end];
    
    // Find the Column::Age match arm section
    if get_method.contains("Column::Age") {
        let age_section: String = get_method
            .lines()
            .skip_while(|l| !l.contains("Column::Age"))
            .take_while(|l| !l.contains("Column::Name") && (!l.trim().is_empty() || l.contains("unwrap_or")))
            .take(6)
            .collect::<Vec<_>>()
            .join("\n");
        
        // Verify it uses Int, not String
        let has_age = age_section.contains("self.age") || age_section.contains(".age");
        assert!(
            age_section.contains("Int") && has_age,
            "Column::Age in ModelTrait::get() should use Int, not String. Found:\n{}",
            age_section
        );
        // The critical check: verify it does NOT use String(None) for Option<i32>
        // We check that the age section specifically uses Int(None), not String(None)
        assert!(
            age_section.contains("Int(None)") && !age_section.contains("String(None)"),
            "Column::Age should use Int(None), not String(None) - this was the bug! Found:\n{}",
            age_section
        );
    } else {
        panic!("Should find Column::Age in ModelTrait::get()");
    }
}
