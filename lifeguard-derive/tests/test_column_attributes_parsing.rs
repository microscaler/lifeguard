//! Unit tests for column attribute parsing (doesn't require LifeModel macro expansion)
//!
//! NOTE: These tests are currently disabled because proc-macro crates cannot export modules.
//! The attribute parsing functionality is tested indirectly via the integration tests
//! in test_column_attributes.rs, which verify that the LifeModel macro correctly uses
//! parse_column_attributes() to extract and apply all column attributes.
//!
//! If we need direct unit tests for attribute parsing, we would need to either:
//! 1. Move the attributes module to a separate non-proc-macro crate
//! 2. Create a test helper that exposes the parsing function
//! 3. Use integration tests that test the macro output

// All tests are disabled - attribute parsing is tested via integration tests in test_column_attributes.rs
