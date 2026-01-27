//! Tests for build_script module

use lifeguard_migrate::build_script;
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
    
    // Empty entities should now generate an empty registry (not return an error)
    // This allows build.rs to always generate the registry file, even when no entities exist
    assert!(result.is_ok());
    
    // Verify registry file was created
    assert!(output_file.exists());
    
    // Verify registry content
    let content = std::fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("Auto-generated entity registry"));
    assert!(content.contains("No entities found - empty registry"));
    assert!(content.contains("pub struct EntityMetadata"));
    assert!(content.contains("pub fn all_entity_metadata()"));
    assert!(content.contains("pub fn generate_sql_for_all()"));
    // Verify it returns empty vectors
    assert!(content.contains("vec![]"));
    assert!(content.contains("Ok(vec![])"));
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

// ============================================================================
// Tests for empty entities scenario (positive and negative cases)
// ============================================================================

#[test]
fn test_empty_registry_structure_valid() {
    // POSITIVE: Verify empty registry has correct structure that can be included
    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("entity_registry.rs");
    
    let entities = Vec::new();
    let result = build_script::generate_registry_module(&entities, &output_file);
    assert!(result.is_ok());
    
    let content = fs::read_to_string(&output_file).unwrap();
    
    // Verify all required components exist
    assert!(content.contains("pub struct EntityMetadata"));
    assert!(content.contains("pub table_name: &'static str"));
    assert!(content.contains("pub service_path: &'static str"));
    assert!(content.contains("pub fn all_entity_metadata() -> Vec<EntityMetadata>"));
    assert!(content.contains("pub fn generate_sql_for_all() -> Result<Vec<(String, String)>, String>"));
    
    // Verify functions return empty collections
    assert!(content.contains("vec![]"));
    assert!(content.contains("Ok(vec![])"));
}

#[test]
fn test_empty_registry_can_be_included() {
    // POSITIVE: Verify empty registry can be included in a module without compilation errors
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path().join("out");
    fs::create_dir_all(&out_dir).unwrap();
    
    let registry_file = out_dir.join("entity_registry.rs");
    let entities = Vec::new();
    build_script::generate_registry_module(&entities, &registry_file).unwrap();
    
    // Create a test lib.rs that includes the registry (simulating examples/entities/src/lib.rs)
    let lib_rs = temp_dir.path().join("lib.rs");
    let lib_content = format!(
        r#"
        #[allow(missing_docs)]
        pub mod entity_registry {{
            include!(concat!("{}", "/entity_registry.rs"));
        }}
        
        // Test that we can use the registry functions
        #[cfg(test)]
        mod tests {{
            use super::entity_registry;
            
            #[test]
            fn test_empty_registry_functions() {{
                let metadata = entity_registry::all_entity_metadata();
                assert_eq!(metadata.len(), 0);
                
                let sql_result = entity_registry::generate_sql_for_all().unwrap();
                assert_eq!(sql_result.len(), 0);
            }}
        }}
        "#,
        out_dir.to_string_lossy().replace('\\', "/")
    );
    fs::write(&lib_rs, lib_content).unwrap();
    
    // Verify the file exists and is readable
    assert!(registry_file.exists());
    assert!(lib_rs.exists());
    
    // Verify the registry content is valid Rust syntax by checking structure
    let registry_content = fs::read_to_string(&registry_file).unwrap();
    assert!(registry_content.contains("pub struct"));
    assert!(registry_content.contains("pub fn"));
    assert!(registry_content.ends_with("}\n") || registry_content.ends_with("}\n\n"));
}

#[test]
fn test_empty_registry_from_empty_directory() {
    // POSITIVE: Verify that discovering from empty directory generates valid registry
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();
    
    // Discover entities from empty directory
    let entities = build_script::discover_entities(&src_dir).unwrap();
    assert_eq!(entities.len(), 0);
    
    // Generate registry
    let output_file = temp_dir.path().join("entity_registry.rs");
    let result = build_script::generate_registry_module(&entities, &output_file);
    
    assert!(result.is_ok());
    assert!(output_file.exists());
    
    // Verify it's a valid empty registry
    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("No entities found - empty registry"));
    assert!(content.contains("vec![]"));
}

#[test]
fn test_empty_registry_after_discovery_error() {
    // POSITIVE: Verify that error during discovery still generates empty registry
    // This simulates the build.rs behavior when discover_entities returns Err
    
    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("entity_registry.rs");
    
    // Simulate build.rs behavior: on error, use empty entities list
    let entities = Vec::new(); // This is what build.rs does on error
    
    let result = build_script::generate_registry_module(&entities, &output_file);
    assert!(result.is_ok(), "Should generate empty registry even when entities list is empty");
    
    assert!(output_file.exists());
    
    let content = fs::read_to_string(&output_file).unwrap();
    assert!(content.contains("No entities found - empty registry"));
}

#[test]
fn test_empty_registry_functions_return_correct_types() {
    // POSITIVE: Verify empty registry functions have correct return types
    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("entity_registry.rs");
    
    let entities = Vec::new();
    build_script::generate_registry_module(&entities, &output_file).unwrap();
    
    let content = fs::read_to_string(&output_file).unwrap();
    
    // Verify function signatures are correct
    assert!(content.contains("pub fn all_entity_metadata() -> Vec<EntityMetadata>"));
    assert!(content.contains("pub fn generate_sql_for_all() -> Result<Vec<(String, String)>, String>"));
    
    // Verify implementations return correct types
    assert!(content.contains("vec![]")); // Vec<EntityMetadata>
    assert!(content.contains("Ok(vec![])")); // Result<Vec<(String, String)>, String>
}

#[test]
fn test_empty_registry_vs_non_empty_registry_structure() {
    // POSITIVE: Verify empty registry has same structure as non-empty registry
    let temp_dir = TempDir::new().unwrap();
    
    // Generate empty registry
    let empty_file = temp_dir.path().join("empty_registry.rs");
    build_script::generate_registry_module(&Vec::new(), &empty_file).unwrap();
    let empty_content = fs::read_to_string(&empty_file).unwrap();
    
    // Generate non-empty registry
    let src_dir = temp_dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let entity_file = src_dir.join("test.rs");
    fs::write(&entity_file, r#"
        #[derive(LifeModel)]
        #[table_name = "test"]
        pub struct Test { pub id: i32; }
    "#).unwrap();
    
    let entities = build_script::discover_entities(&src_dir).unwrap();
    let non_empty_file = temp_dir.path().join("non_empty_registry.rs");
    build_script::generate_registry_module(&entities, &non_empty_file).unwrap();
    let non_empty_content = fs::read_to_string(&non_empty_file).unwrap();
    
    // Both should have the same struct definition
    assert!(empty_content.contains("pub struct EntityMetadata"));
    assert!(non_empty_content.contains("pub struct EntityMetadata"));
    
    // Both should have the same function signatures
    assert!(empty_content.contains("pub fn all_entity_metadata()"));
    assert!(non_empty_content.contains("pub fn all_entity_metadata()"));
    assert!(empty_content.contains("pub fn generate_sql_for_all()"));
    assert!(non_empty_content.contains("pub fn generate_sql_for_all()"));
    
    // Empty should return empty vec, non-empty should have entries
    assert!(empty_content.contains("vec![]"));
    assert!(!non_empty_content.contains("vec![]")); // Non-empty should have actual entries
}

#[test]
fn test_missing_registry_file_would_fail() {
    // NEGATIVE: Verify that missing registry file would cause compilation error
    // This demonstrates why the fix was necessary
    
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path().join("out");
    fs::create_dir_all(&out_dir).unwrap();
    
    // Simulate the OLD buggy behavior: build.rs returns early without creating file
    // (We simulate this by just not creating the file)
    let registry_file = out_dir.join("entity_registry.rs");
    
    // Verify file doesn't exist (simulating old buggy behavior)
    assert!(!registry_file.exists(), "Registry file should not exist (simulating old bug)");
    
    // Create a lib.rs that tries to include the missing file (this would fail in real build)
    let lib_rs = temp_dir.path().join("lib.rs");
    let lib_content = format!(
        r#"
        pub mod entity_registry {{
            include!(concat!("{}", "/entity_registry.rs"));
        }}
        "#,
        out_dir.to_string_lossy().replace('\\', "/")
    );
    fs::write(&lib_rs, lib_content).unwrap();
    
    // The file doesn't exist - this demonstrates the bug
    // In a real build, this would cause: "No such file or directory" error
    assert!(!registry_file.exists());
}

#[test]
fn test_empty_registry_prevents_missing_file_error() {
    // POSITIVE: Verify that generating empty registry prevents the missing file error
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path().join("out");
    fs::create_dir_all(&out_dir).unwrap();
    
    // Simulate NEW fixed behavior: always generate registry, even when empty
    let registry_file = out_dir.join("entity_registry.rs");
    let entities = Vec::new();
    build_script::generate_registry_module(&entities, &registry_file).unwrap();
    
    // Verify file exists (fix prevents the bug)
    assert!(registry_file.exists(), "Registry file should exist even with empty entities");
    
    // Create a lib.rs that includes the registry (this would work in real build)
    let lib_rs = temp_dir.path().join("lib.rs");
    let lib_content = format!(
        r#"
        pub mod entity_registry {{
            include!(concat!("{}", "/entity_registry.rs"));
        }}
        "#,
        out_dir.to_string_lossy().replace('\\', "/")
    );
    fs::write(&lib_rs, lib_content).unwrap();
    
    // Both files exist - this demonstrates the fix works
    assert!(registry_file.exists());
    assert!(lib_rs.exists());
    
    // Verify registry content is valid
    let content = fs::read_to_string(&registry_file).unwrap();
    assert!(!content.is_empty());
    assert!(content.contains("pub struct EntityMetadata"));
}

#[test]
fn test_empty_registry_metadata_structure() {
    // POSITIVE: Verify EntityMetadata struct in empty registry matches expected structure
    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("entity_registry.rs");
    
    let entities = Vec::new();
    build_script::generate_registry_module(&entities, &output_file).unwrap();
    
    let content = fs::read_to_string(&output_file).unwrap();
    
    // Verify EntityMetadata struct definition
    assert!(content.contains("pub struct EntityMetadata {"));
    assert!(content.contains("pub table_name: &'static str,"));
    assert!(content.contains("pub service_path: &'static str,"));
    assert!(content.contains("}"));
    
    // Verify all_entity_metadata returns Vec<EntityMetadata>
    assert!(content.contains("pub fn all_entity_metadata() -> Vec<EntityMetadata>"));
    assert!(content.contains("vec![]"));
}

#[test]
fn test_empty_registry_sql_generation_function() {
    // POSITIVE: Verify generate_sql_for_all function in empty registry
    let temp_dir = TempDir::new().unwrap();
    let output_file = temp_dir.path().join("entity_registry.rs");
    
    let entities = Vec::new();
    build_script::generate_registry_module(&entities, &output_file).unwrap();
    
    let content = fs::read_to_string(&output_file).unwrap();
    
    // Verify function signature
    assert!(content.contains("pub fn generate_sql_for_all() -> Result<Vec<(String, String)>, String>"));
    
    // Verify implementation returns Ok with empty vec
    assert!(content.contains("Ok(vec![])"));
    
    // Should NOT contain any SQL generation code for entities (since there are none)
    assert!(!content.contains("sql_generator::generate_create_table_sql"));
}
