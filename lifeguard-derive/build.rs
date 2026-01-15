//! Build script to generate entity code before compilation
//!
//! This script runs lifeguard-codegen to generate Entity, Model, Column, etc.
//! before the tests compile, avoiding E0223 errors from procedural macros.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // Get the crate root directory (where build.rs runs)
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    
    // Calculate paths relative to crate root
    // Input: lifeguard-codegen/input (relative to workspace root, which is parent of crate root)
    let workspace_root = crate_root.parent().expect("Crate root should have a parent (workspace root)");
    let input_dir = workspace_root.join("lifeguard-codegen").join("input");
    
    // Output: tests/generated (relative to crate root)
    let output_dir = crate_root.join("tests").join("generated");
    
    // Create output directory
    std::fs::create_dir_all(&output_dir).expect("Failed to create generated directory");
    
    // Set rerun-if-changed paths (relative to workspace root for input, crate root for output)
    println!("cargo:rerun-if-changed={}", input_dir.display());
    println!("cargo:rerun-if-changed={}", workspace_root.join("lifeguard-codegen").join("src").display());
    
    // Run codegen tool from workspace root
    // Pass absolute paths to avoid confusion
    let status = Command::new("cargo")
        .args(&["run", "--bin", "lifeguard-codegen", "--", "generate"])
        .args(&["--input", input_dir.to_str().unwrap()])
        .args(&["--output", output_dir.to_str().unwrap()])
        .current_dir(workspace_root)
        .status();
    
    match status {
        Ok(s) if s.success() => {
            println!("cargo:warning=✅ Codegen completed successfully");
        }
        Ok(s) => {
            eprintln!("cargo:warning=⚠️ Codegen failed with status: {:?}", s);
            // Don't fail the build - allow tests to be skipped if codegen isn't available
        }
        Err(e) => {
            eprintln!(
                "cargo:warning=⚠️ Failed to run codegen: {}. Tests may be skipped.",
                e
            );
            // Don't fail the build
        }
    }
}
