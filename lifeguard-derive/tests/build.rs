//! Build script to generate entity code before compilation
//!
//! This script runs lifeguard-codegen to generate Entity, Model, Column, etc.
//! before the tests compile, avoiding E0223 errors from procedural macros.

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=lifeguard-codegen/input");
    println!("cargo:rerun-if-changed=lifeguard-codegen/src");

    let input_dir = Path::new("lifeguard-codegen/input");
    let output_dir = Path::new("tests/generated");

    // Create output directory
    std::fs::create_dir_all(output_dir).expect("Failed to create generated directory");

    // Run codegen tool
    let status = Command::new("cargo")
        .args(&["run", "--bin", "lifeguard-codegen", "--", "generate"])
        .args(&["--input", input_dir.to_str().unwrap()])
        .args(&["--output", output_dir.to_str().unwrap()])
        .current_dir("..")
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
