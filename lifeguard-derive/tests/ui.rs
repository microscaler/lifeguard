//! UI tests for DeriveRelation, DerivePartialModel, and DeriveLinked macro compile errors
//!
//! These tests verify that malformed attributes cause compile errors
//! instead of being silently ignored or panicking.
//!
//! Note: We use a single shared TestCases instance to avoid race conditions
//! when tests run in parallel. Each TestCases instance creates temporary directories
//! and Cargo.toml files, and parallel execution can cause file corruption.

#[macro_use]
extern crate lazy_static;

use std::sync::Mutex;

// Shared TestCases instance to avoid race conditions in parallel test execution
lazy_static! {
    static ref TEST_CASES: Mutex<trybuild::TestCases> = Mutex::new(trybuild::TestCases::new());
}

#[test]
fn compile_error_from_attribute() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_from_attribute.rs");
}

#[test]
fn compile_error_to_attribute() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_to_attribute.rs");
}

#[test]
fn compile_error_has_many_attribute() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_has_many_attribute.rs");
}

#[test]
fn compile_error_partial_model_empty_entity() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_partial_model_empty_entity.rs");
}

#[test]
fn compile_error_partial_model_leading_colons() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_partial_model_leading_colons.rs");
}

#[test]
fn compile_error_partial_model_trailing_colons() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_partial_model_trailing_colons.rs");
}

#[test]
fn compile_error_partial_model_consecutive_colons() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_partial_model_consecutive_colons.rs");
}

#[test]
fn compile_error_partial_model_invalid_identifier_single_colon() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_partial_model_invalid_identifier_single_colon.rs");
}

#[test]
fn compile_error_partial_model_invalid_identifier_colon_in_middle() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_partial_model_invalid_identifier_colon_in_middle.rs");
}

#[test]
fn compile_error_partial_model_invalid_identifier_starts_with_number() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_partial_model_invalid_identifier_starts_with_number.rs");
}

#[test]
fn compile_error_partial_model_invalid_identifier_contains_hyphen() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_partial_model_invalid_identifier_contains_hyphen.rs");
}

#[test]
fn compile_error_partial_model_invalid_identifier_path_segment() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_partial_model_invalid_identifier_path_segment.rs");
}

#[test]
fn compile_error_relation_invalid_entity_path_single_colon() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_relation_invalid_entity_path_single_colon.rs");
}

#[test]
fn compile_error_relation_invalid_entity_path_colon_in_middle() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_relation_invalid_entity_path_colon_in_middle.rs");
}

#[test]
fn compile_error_relation_invalid_column_ref() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_relation_invalid_column_ref.rs");
}

#[test]
fn compile_error_relation_invalid_column_ref_path() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_relation_invalid_column_ref_path.rs");
}

#[test]
fn compile_error_duplicate_related_impl_different_columns() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_duplicate_related_impl_different_columns.rs");
}

#[test]
fn compile_error_try_into_model_missing_model() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_try_into_model_missing_model.rs");
}

#[test]
fn compile_error_try_into_model_custom_error_convert() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_try_into_model_custom_error_convert.rs");
}

#[test]
fn compile_pass_try_into_model_split_attributes() {
    let t = TEST_CASES.lock().unwrap();
    t.pass("tests/ui/compile_error_try_into_model_split_attributes.rs");
}

#[test]
fn compile_error_try_into_model_malformed_convert() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_try_into_model_malformed_convert.rs");
}

#[test]
fn compile_error_try_into_model_malformed_map_from() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_try_into_model_malformed_map_from.rs");
}

#[test]
fn compile_error_try_into_model_custom_lifeerror_from_other_module() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_try_into_model_custom_lifeerror_from_other_module.rs");
}

#[test]
fn compile_error_linked_invalid_path() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_linked_invalid_path.rs");
}

#[test]
fn compile_error_linked_empty_path() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_linked_empty_path.rs");
}

#[test]
fn compile_error_linked_invalid_entity_path() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_linked_invalid_entity_path.rs");
}

#[test]
fn compile_error_skip_primary_key() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_skip_primary_key.rs");
}

#[test]
fn compile_error_ignore_primary_key() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_ignore_primary_key.rs");
}

#[test]
fn compile_error_select_as_empty_string() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_select_as_empty_string.rs");
}

#[test]
fn compile_error_save_as_empty_string() {
    let t = TEST_CASES.lock().unwrap();
    t.compile_fail("tests/ui/compile_error_save_as_empty_string.rs");
}
