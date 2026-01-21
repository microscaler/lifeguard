//! Lifeguard Migration Library
//!
//! This library provides migration functionality for Lifeguard ORM.
//! The CLI tool (main.rs) uses this library.

pub mod sql_generator;
pub mod entity_loader;
pub mod build_script;

// Note: entities.rs has been removed - entities are now discovered via build script
// and accessed through the generated registry module in the user's project