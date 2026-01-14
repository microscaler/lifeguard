//! Integration test: Verify generated code compiles with lifeguard

use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn test_generated_code_compiles_with_lifeguard() {
    let test_dir = Path::new("/tmp/lifeguard-codegen-test");
    
    // Clean up previous test
    if test_dir.exists() {
        fs::remove_dir_all(test_dir).unwrap();
    }
    fs::create_dir_all(test_dir).unwrap();
    
    // Generate entity code
    let output = Command::new("cargo")
        .args(&["run", "--", "generate", "--input", "input", "--output", test_dir.to_str().unwrap()])
        .current_dir("lifeguard-codegen")
        .output()
        .expect("Failed to run codegen");
    
    if !output.status.success() {
        eprintln!("Codegen failed:");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        panic!("Codegen failed");
    }
    
    // Create a test crate that uses the generated code
    let cargo_toml = format!(r#"
[package]
name = "test-generated-code"
version = "0.1.0"
edition = "2021"

[dependencies]
lifeguard = {{ path = "{}" }}
may-postgres = {{ git = "https://github.com/Xudong-Huang/may_postgres", branch = "master" }}
sea-query = {{ version = "1.0.0-rc.29", features = ["with-json"] }}
"#, 
        Path::new("lifeguard-codegen").canonicalize().unwrap().parent().unwrap().display()
    );
    
    fs::write(test_dir.join("Cargo.toml"), cargo_toml).unwrap();
    
    // Create a mod.rs that includes the generated code
    let mod_rs = r#"
mod user;

pub use user::*;
"#;
    
    fs::write(test_dir.join("mod.rs"), mod_rs).unwrap();
    
    // Create a simple test that uses the generated code
    let test_rs = r#"
use test_generated_code::*;

#[test]
fn test_entity_compiles() {
    // Verify Entity exists and implements LifeEntityName
    let entity = User;
    assert_eq!(entity.table_name(), "users");
    assert_eq!(User::TABLE_NAME, "users");
}

#[test]
fn test_column_enum() {
    // Verify Column enum exists
    let _id = Column::Id;
    let _email = Column::Email;
    let _name = Column::Name;
}

#[test]
fn test_life_model_trait() {
    // Verify LifeModelTrait is implemented
    // This is where E0223 would occur in proc-macro approach
    fn _test_trait<E: LifeModelTrait>() {}
    _test_trait::<User>();
    
    // Verify Column associated type is set
    let _column_type: User::Column = Column::Id;
}
"#;
    
    fs::write(test_dir.join("lib.rs"), test_rs).unwrap();
    
    // Try to compile the test crate
    let compile_output = Command::new("cargo")
        .args(&["check", "--lib"])
        .current_dir(test_dir)
        .output();
    
    match compile_output {
        Ok(output) => {
            if !output.status.success() {
                eprintln!("Compilation failed:");
                eprintln!("{}", String::from_utf8_lossy(&output.stdout));
                eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                panic!("Generated code failed to compile");
            }
            println!("✅ Generated code compiles successfully!");
        }
        Err(e) => {
            eprintln!("Failed to run cargo check: {}", e);
            // Don't fail the test if cargo isn't available in test environment
            println!("⚠️  Skipping compilation check (cargo not available in test environment)");
        }
    }
}
