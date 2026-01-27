//! Tests for build_script module

use lifeguard_migrate::build_script;
use std::path::PathBuf;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_discover_entities_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    let result = build_script::discover_entities(temp_dir.path());
    
    assert!(result.is_ok());
    let entities = result.unwrap();
    assert_eq!(entities.len(), 0);
}

#[test]
fn test_discover_entities_with_life_model() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path();
    
    // Create entity file with #[derive(LifeModel)]
    let entity_file = src_dir.join("test_entity.rs");
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
    
    let result = build_script::discover_entities(src_dir);
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    assert_eq!(entities.len(), 1);
    
    let entity = &entities[0];
    assert_eq!(entity.struct_name, "TestEntity");
    assert_eq!(entity.table_name, "test_table");
    assert_eq!(entity.file_path, entity_file);
}

#[test]
fn test_discover_entities_skips_target_directory() {
    let temp_dir = TempDir::new().unwrap();
    let root_dir = temp_dir.path();
    
    // Create target directory (should be skipped)
    let target_dir = root_dir.join("target");
    fs::create_dir_all(&target_dir).unwrap();
    let entity_in_target = target_dir.join("entity.rs");
    fs::write(&entity_in_target, r#"
        #[derive(LifeModel)]
        pub struct Entity { pub id: i32; }
    "#).unwrap();
    
    // Create entity in root (should be found)
    let entity_file = root_dir.join("entity.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        #[table_name = "table"]
        pub struct Entity { pub id: i32; }
    "#).unwrap();
    
    let result = build_script::discover_entities(root_dir);
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    // Should only find the entity in root, not in target/
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].file_path, entity_file);
}

#[test]
fn test_discover_entities_skips_git_directory() {
    let temp_dir = TempDir::new().unwrap();
    let root_dir = temp_dir.path();
    
    // Create .git directory (should be skipped)
    let git_dir = root_dir.join(".git");
    fs::create_dir_all(&git_dir).unwrap();
    let entity_in_git = git_dir.join("entity.rs");
    fs::write(&entity_in_git, r#"
        #[derive(LifeModel)]
        pub struct Entity { pub id: i32; }
    "#).unwrap();
    
    // Create entity in root (should be found)
    let entity_file = root_dir.join("entity.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        #[table_name = "table"]
        pub struct Entity { pub id: i32; }
    "#).unwrap();
    
    let result = build_script::discover_entities(root_dir);
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    // Should only find the entity in root, not in .git/
    assert_eq!(entities.len(), 1);
}

#[test]
fn test_discover_entities_recursive() {
    let temp_dir = TempDir::new().unwrap();
    let root_dir = temp_dir.path();
    
    // Create nested directory structure
    let subdir = root_dir.join("accounting").join("general_ledger");
    fs::create_dir_all(&subdir).unwrap();
    
    // Create entity in subdirectory
    let entity_file = subdir.join("chart_of_accounts.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        #[table_name = "chart_of_accounts"]
        pub struct ChartOfAccount {
            pub id: uuid::Uuid,
        }
    "#).unwrap();
    
    let result = build_script::discover_entities(root_dir);
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].struct_name, "ChartOfAccount");
}

#[test]
fn test_discover_entities_extract_table_name() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path();
    
    // Create entity with explicit table_name
    let entity_file = src_dir.join("entity.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        #[table_name = "custom_table"]
        pub struct MyEntity {
            pub id: i32,
        }
    "#).unwrap();
    
    let result = build_script::discover_entities(src_dir);
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].table_name, "custom_table");
}

#[test]
fn test_discover_entities_fallback_table_name() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path();
    
    // Create entity without table_name (should use snake_case of struct name)
    let entity_file = src_dir.join("entity.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        pub struct ChartOfAccount {
            pub id: i32,
        }
    "#).unwrap();
    
    let result = build_script::discover_entities(src_dir);
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    assert_eq!(entities.len(), 1);
    // Should fallback to snake_case of struct name
    assert_eq!(entities[0].table_name, "chart_of_account");
}

#[test]
fn test_discover_entities_multiple_entities() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path();
    
    // Create multiple entity files
    for i in 1..=3 {
        let entity_file = src_dir.join(format!("entity_{}.rs", i));
        fs::write(&entity_file, format!(r#"
            #[derive(LifeModel)]
            #[table_name = "table_{}"]
            pub struct Entity{} {{
                pub id: i32,
            }}
        "#, i, i)).unwrap();
    }
    
    let result = build_script::discover_entities(src_dir);
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    assert_eq!(entities.len(), 3);
    
    // Verify all entities are discovered
    let struct_names: Vec<&String> = entities.iter().map(|e| &e.struct_name).collect();
    assert!(struct_names.contains(&&"Entity1".to_string()));
    assert!(struct_names.contains(&&"Entity2".to_string()));
    assert!(struct_names.contains(&&"Entity3".to_string()));
}

#[test]
fn test_discover_entities_skips_non_life_model_files() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path();
    
    // Create file without LifeModel derive (should be skipped)
    let non_entity_file = src_dir.join("helper.rs");
    fs::write(&non_entity_file, r#"
        pub struct Helper {
            pub value: i32,
        }
    "#).unwrap();
    
    // Create entity file (should be found)
    let entity_file = src_dir.join("entity.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        #[table_name = "table"]
        pub struct Entity {
            pub id: i32,
        }
    "#).unwrap();
    
    let result = build_script::discover_entities(src_dir);
    assert!(result.is_ok());
    
    let entities = result.unwrap();
    // Should only find the entity with LifeModel
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].struct_name, "Entity");
}

#[test]
fn test_generate_registry_module_empty_entities() {
    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("entity_registry.rs");
    
    let entities = Vec::new();
    let result = build_script::generate_registry_module(&entities, &output_file);
    
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("No entities found"));
}

#[test]
fn test_generate_registry_module_single_entity() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();
    
    // Create entity file
    let entity_file = src_dir.join("test_entity.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        #[table_name = "test_table"]
        pub struct TestEntity {
            pub id: i32,
        }
    "#).unwrap();
    
    // Discover entities
    let entities = build_script::discover_entities(&src_dir).unwrap();
    assert_eq!(entities.len(), 1);
    
    // Generate registry
    let output_file = temp_dir.path().join("out").join("entity_registry.rs");
    fs::create_dir_all(output_file.parent().unwrap()).unwrap();
    
    let result = build_script::generate_registry_module(&entities, &output_file);
    assert!(result.is_ok());
    
    // Verify registry file was created
    assert!(output_file.exists());
    
    // Verify registry content
    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("Auto-generated entity registry"));
    // The registry includes entity modules using snake_case of struct name
    assert!(content.contains("test_entity")); // Module name from "TestEntity"
    // The registry references the entity via module path (sanitized)
    assert!(content.contains("crate::test_entity"));
}

#[test]
fn test_generate_registry_module_with_service_path() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    
    // Create service directory structure
    let service_dir = src_dir.join("accounting").join("general_ledger");
    fs::create_dir_all(&service_dir).unwrap();
    
    // Create entity file in service directory
    let entity_file = service_dir.join("chart_of_accounts.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        #[table_name = "chart_of_accounts"]
        pub struct ChartOfAccount {
            pub id: uuid::Uuid,
        }
    "#).unwrap();
    
    // Discover entities
    let entities = build_script::discover_entities(&src_dir).unwrap();
    assert_eq!(entities.len(), 1);
    
    // Generate registry
    let output_file = temp_dir.path().join("out").join("entity_registry.rs");
    fs::create_dir_all(output_file.parent().unwrap()).unwrap();
    
    let result = build_script::generate_registry_module(&entities, &output_file);
    assert!(result.is_ok());
    
    // Verify registry file was created
    assert!(output_file.exists());
    
    // Verify registry content includes service path
    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("ChartOfAccount"));
    assert!(content.contains("chart_of_accounts"));
}

#[test]
fn test_generate_registry_module_sanitizes_hyphens() {
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    
    // Create service directory structure with hyphens (should be sanitized to underscores)
    let service_dir = src_dir.join("my-feature").join("sub-module");
    fs::create_dir_all(&service_dir).unwrap();
    
    // Create entity file with hyphen in name (should be sanitized)
    let entity_file = service_dir.join("my-entity.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        #[table_name = "my_entity"]
        pub struct MyEntity {
            pub id: i32,
        }
    "#).unwrap();
    
    // Discover entities
    let entities = build_script::discover_entities(&src_dir).unwrap();
    assert_eq!(entities.len(), 1);
    
    // Generate registry
    let output_file = temp_dir.path().join("out").join("entity_registry.rs");
    fs::create_dir_all(output_file.parent().unwrap()).unwrap();
    
    let result = build_script::generate_registry_module(&entities, &output_file);
    assert!(result.is_ok());
    
    // Verify registry file was created
    assert!(output_file.exists());
    
    // Verify registry content - hyphens should be converted to underscores in module paths
    let content = fs::read_to_string(&output_file).unwrap();
    // Module path should use underscores, not hyphens: "crate::my_feature::sub_module::my_entity"
    assert!(content.contains("crate::my_feature::sub_module::my_entity"));
    // Should NOT contain hyphens in module paths (would be invalid Rust)
    assert!(!content.contains("crate::my-feature"));
    assert!(!content.contains("my-entity::Entity"));
}
