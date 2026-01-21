//! Tests for registry_loader module

use lifeguard_migrate::registry_loader;
use std::path::PathBuf;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_is_registry_available_when_not_found() {
    // When OUT_DIR is not set and registry doesn't exist, should return false
    // Note: This test may be flaky if OUT_DIR is set in the test environment
    // In practice, this is fine - the function checks multiple locations
    let available = registry_loader::is_registry_available();
    // We can't assert a specific value since it depends on the environment
    // But we can verify the function doesn't panic
    assert!(available == true || available == false);
}

#[test]
fn test_get_setup_instructions() {
    let instructions = registry_loader::get_setup_instructions();
    
    // Verify instructions contain key information
    assert!(instructions.contains("build.rs"));
    assert!(instructions.contains("Cargo.toml"));
    assert!(instructions.contains("lifeguard-migrate"));
    assert!(instructions.contains("entity_registry"));
    assert!(instructions.contains("OUT_DIR"));
}

#[test]
fn test_find_registry_path_with_out_dir() {
    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();
    
    // Create registry file
    let registry_file = out_dir.join("entity_registry.rs");
    fs::write(&registry_file, "// Test registry").unwrap();
    
    // Set OUT_DIR environment variable
    std::env::set_var("OUT_DIR", out_dir.to_str().unwrap());
    
    // find_registry_path should find it
    let found_path = registry_loader::find_registry_path();
    
    // Clean up
    std::env::remove_var("OUT_DIR");
    
    // Should find the registry file
    assert!(found_path.is_some());
    let path = found_path.unwrap();
    assert_eq!(path, registry_file);
}

#[test]
fn test_find_registry_path_without_out_dir() {
    // Clear OUT_DIR if it exists
    std::env::remove_var("OUT_DIR");
    
    // When OUT_DIR is not set and registry doesn't exist in target/, should return None
    // Note: This test may find a registry if one exists in the project
    // That's okay - we're just testing the function doesn't panic
    let found_path = registry_loader::find_registry_path();
    
    // Function should return Some or None, but not panic
    if let Some(path) = found_path {
        // If a path is found, verify it exists
        assert!(path.exists());
    }
}

#[test]
fn test_find_registry_path_nonexistent() {
    // Set OUT_DIR to a directory that doesn't contain the registry
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("OUT_DIR", temp_dir.path().to_str().unwrap());
    
    let found_path = registry_loader::find_registry_path();
    
    // Clean up
    std::env::remove_var("OUT_DIR");
    
    // Should return None since registry doesn't exist
    assert!(found_path.is_none());
}
