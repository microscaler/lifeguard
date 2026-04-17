//! Lifeguard Migration Library
//!
//! This library provides migration functionality for Lifeguard ORM.
//! The CLI tool (main.rs) uses this library.

// Tooling crate: keep `cargo clippy --workspace -- -D warnings -W clippy::pedantic` green without
// duplicating the main library’s doc/style bar.
#![allow(warnings)]

pub mod build_script;
pub mod dependency_ordering;
pub mod entity_loader;
pub mod generated_migration_diff;
pub mod migration_writer;
pub mod registry_loader;
pub mod schema_infer;
pub mod schema_migration_compare;
pub mod sql_dependency_order;
pub mod sql_generator;

// Note: entities.rs has been removed - entities are now discovered via build script
// and accessed through the generated registry module in the user's project
