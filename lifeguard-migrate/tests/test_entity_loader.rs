//! Tests for entity_loader module

use lifeguard_migrate::entity_loader;
use std::path::PathBuf;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_load_entities_nonexistent_directory() {
    let nonexistent_dir = PathBuf::from("/nonexistent/path/that/does/not/exist");
    let result = entity_loader::load_entities(&nonexistent_dir);
    
    assert!(result.is_err());
    // Check error message without unwrap_err (which requires Debug)
    match result {
        Err(e) => {
            let error_msg = e.to_string();
            assert!(error_msg.contains("does not exist"));
        }
        Ok(_) => panic!("Expected error for nonexistent directory"),
    }
}

#[test]
fn test_load_entities_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    let result = entity_loader::load_entities(&temp_dir.path().to_path_buf());
    
    assert!(result.is_ok());
    let entities = result.unwrap();
    assert_eq!(entities.len(), 0);
}

#[test]
fn test_load_entities_with_entity_file() {
    let temp_dir = TempDir::new().unwrap();
    let entities_dir = temp_dir.path();
    
    // Create a test entity file
    let entity_file = entities_dir.join("test_entity.rs");
    fs::write(&entity_file, r#"
        use lifeguard_derive::LifeModel;
        
        #[derive(LifeModel)]
        #[table_name = "test_table"]
        pub struct TestEntity {
            #[primary_key]
            pub id: i32,
            pub name: String,
        }
    "#).unwrap();
    
    let result = entity_loader::load_entities(&entities_dir.to_path_buf());
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    assert_eq!(entities.len(), 1);
    
    let entity = &entities[0];
    assert_eq!(entity.name, "test_entity");
    assert_eq!(entity.table_name, "test_table");
    assert_eq!(entity.file_path, entity_file);
    assert_eq!(entity.service_path, None); // No subdirectory
}

#[test]
fn test_load_entities_with_service_path() {
    let temp_dir = TempDir::new().unwrap();
    let entities_dir = temp_dir.path();
    
    // Create service directory structure
    let service_dir = entities_dir.join("accounting").join("general-ledger");
    fs::create_dir_all(&service_dir).unwrap();
    
    // Create entity file in service directory
    let entity_file = service_dir.join("chart_of_accounts.rs");
    fs::write(&entity_file, r#"
        use lifeguard_derive::LifeModel;
        
        #[derive(LifeModel)]
        #[table_name = "chart_of_accounts"]
        pub struct ChartOfAccount {
            #[primary_key]
            pub id: uuid::Uuid,
        }
    "#).unwrap();
    
    let result = entity_loader::load_entities(&entities_dir.to_path_buf());
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    assert_eq!(entities.len(), 1);
    
    let entity = &entities[0];
    assert_eq!(entity.name, "chart_of_accounts");
    assert_eq!(entity.table_name, "chart_of_accounts");
    assert_eq!(entity.file_path, entity_file);
    // Service path should be "accounting/general-ledger" (or "accounting\\general-ledger" on Windows)
    assert!(entity.service_path.is_some());
    let service_path = entity.service_path.as_ref().unwrap();
    assert!(service_path.contains("accounting"));
    assert!(service_path.contains("general"));
}

#[test]
fn test_load_entities_skips_readme_files() {
    let temp_dir = TempDir::new().unwrap();
    let entities_dir = temp_dir.path();
    
    // Create README file (should be skipped)
    let readme_file = entities_dir.join("README.md");
    fs::write(&readme_file, "# Documentation").unwrap();
    
    // Create entity file
    let entity_file = entities_dir.join("test_entity.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        #[table_name = "test_table"]
        pub struct TestEntity {
            pub id: i32,
        }
    "#).unwrap();
    
    let result = entity_loader::load_entities(&entities_dir.to_path_buf());
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    // Should only have the entity file, not the README
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].name, "test_entity");
}

#[test]
fn test_load_entities_skips_implementation_files() {
    let temp_dir = TempDir::new().unwrap();
    let entities_dir = temp_dir.path();
    
    // Create implementation status file (should be skipped)
    let impl_file = entities_dir.join("IMPLEMENTATION_STATUS.md");
    fs::write(&impl_file, "# Status").unwrap();
    
    // Create migration gaps file (should be skipped)
    let gaps_file = entities_dir.join("MIGRATION_GAPS.md");
    fs::write(&gaps_file, "# Gaps").unwrap();
    
    // Create entity file
    let entity_file = entities_dir.join("test_entity.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        #[table_name = "test_table"]
        pub struct TestEntity {
            pub id: i32,
        }
    "#).unwrap();
    
    let result = entity_loader::load_entities(&entities_dir.to_path_buf());
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    // Should only have the entity file
    assert_eq!(entities.len(), 1);
}

#[test]
fn test_load_entities_multiple_entities() {
    let temp_dir = TempDir::new().unwrap();
    let entities_dir = temp_dir.path();
    
    // Create multiple entity files
    for i in 1..=3 {
        let entity_file = entities_dir.join(format!("entity_{}.rs", i));
        fs::write(&entity_file, format!(r#"
            #[derive(LifeModel)]
            #[table_name = "table_{}"]
            pub struct Entity{} {{
                pub id: i32,
            }}
        "#, i, i)).unwrap();
    }
    
    let result = entity_loader::load_entities(&entities_dir.to_path_buf());
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    assert_eq!(entities.len(), 3);
    
    // Verify all entities are loaded
    let table_names: Vec<&String> = entities.iter().map(|e| &e.table_name).collect();
    assert!(table_names.contains(&&"table_1".to_string()));
    assert!(table_names.contains(&&"table_2".to_string()));
    assert!(table_names.contains(&&"table_3".to_string()));
}

#[test]
fn test_load_entities_extract_table_name_from_attribute() {
    let temp_dir = TempDir::new().unwrap();
    let entities_dir = temp_dir.path();
    
    // Create entity file with explicit table_name attribute
    let entity_file = entities_dir.join("my_entity.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        #[table_name = "custom_table_name"]
        pub struct MyEntity {
            pub id: i32,
        }
    "#).unwrap();
    
    let result = entity_loader::load_entities(&entities_dir.to_path_buf());
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].table_name, "custom_table_name");
}

#[test]
fn test_load_entities_fallback_table_name() {
    let temp_dir = TempDir::new().unwrap();
    let entities_dir = temp_dir.path();
    
    // Create entity file without table_name attribute
    let entity_file = entities_dir.join("my_entity.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        pub struct MyEntity {
            pub id: i32,
        }
    "#).unwrap();
    
    let result = entity_loader::load_entities(&entities_dir.to_path_buf());
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    assert_eq!(entities.len(), 1);
    // Should fallback to file name (my_entity)
    assert_eq!(entities[0].table_name, "my_entity");
}

#[test]
fn test_load_entities_recursive_subdirectories() {
    let temp_dir = TempDir::new().unwrap();
    let entities_dir = temp_dir.path();
    
    // Create nested directory structure
    let subdir1 = entities_dir.join("service1");
    let subdir2 = entities_dir.join("service2");
    fs::create_dir_all(&subdir1).unwrap();
    fs::create_dir_all(&subdir2).unwrap();
    
    // Create entities in different directories
    let entity1 = subdir1.join("entity1.rs");
    fs::write(&entity1, r#"
        #[derive(LifeModel)]
        #[table_name = "table1"]
        pub struct Entity1 { pub id: i32; }
    "#).unwrap();
    
    let entity2 = subdir2.join("entity2.rs");
    fs::write(&entity2, r#"
        #[derive(LifeModel)]
        #[table_name = "table2"]
        pub struct Entity2 { pub id: i32; }
    "#).unwrap();
    
    let result = entity_loader::load_entities(&entities_dir.to_path_buf());
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    assert_eq!(entities.len(), 2);
    
    // Verify both entities are found
    let table_names: Vec<&String> = entities.iter().map(|e| &e.table_name).collect();
    assert!(table_names.contains(&&"table1".to_string()));
    assert!(table_names.contains(&&"table2".to_string()));
}

#[test]
fn test_load_entities_with_lifemodel_not_first() {
    let temp_dir = TempDir::new().unwrap();
    let entities_dir = temp_dir.path();
    
    // Test case 1: #[derive(Clone, LifeModel)]
    let entity1 = entities_dir.join("entity1.rs");
    fs::write(&entity1, r#"
        #[derive(Clone, LifeModel)]
        #[table_name = "table1"]
        pub struct Entity1 { pub id: i32; }
    "#).unwrap();
    
    // Test case 2: #[derive(Debug, Serialize, LifeModel)]
    let entity2 = entities_dir.join("entity2.rs");
    fs::write(&entity2, r#"
        #[derive(Debug, Serialize, LifeModel)]
        #[table_name = "table2"]
        pub struct Entity2 { pub id: i32; }
    "#).unwrap();
    
    // Test case 3: #[derive(LifeModel, Clone)] - should also work
    let entity3 = entities_dir.join("entity3.rs");
    fs::write(&entity3, r#"
        #[derive(LifeModel, Clone)]
        #[table_name = "table3"]
        pub struct Entity3 { pub id: i32; }
    "#).unwrap();
    
    let result = entity_loader::load_entities(&entities_dir.to_path_buf());
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    // Should find all 3 entities
    assert_eq!(entities.len(), 3);
    
    // Verify all entities are found
    let table_names: Vec<&String> = entities.iter().map(|e| &e.table_name).collect();
    assert!(table_names.contains(&&"table1".to_string()));
    assert!(table_names.contains(&&"table2".to_string()));
    assert!(table_names.contains(&&"table3".to_string()));
}

#[test]
fn test_load_entities_skips_non_lifemodel_derives() {
    let temp_dir = TempDir::new().unwrap();
    let entities_dir = temp_dir.path();
    
    // Create file with derive but NOT LifeModel - should be skipped
    let non_entity = entities_dir.join("not_an_entity.rs");
    fs::write(&non_entity, r#"
        #[derive(Clone, Debug)]
        pub struct NotAnEntity { pub id: i32; }
    "#).unwrap();
    
    // Create file with LifeModel - should be found
    let entity = entities_dir.join("entity.rs");
    fs::write(&entity, r#"
        #[derive(LifeModel)]
        #[table_name = "entity_table"]
        pub struct Entity { pub id: i32; }
    "#).unwrap();
    
    let result = entity_loader::load_entities(&entities_dir.to_path_buf());
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    // Should only find the entity with LifeModel
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].table_name, "entity_table");
}
