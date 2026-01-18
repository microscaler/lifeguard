//! UI tests for DeriveRelation and DerivePartialModel macro compile errors
//!
//! These tests verify that malformed attributes cause compile errors
//! instead of being silently ignored or panicking.

#[test]
fn compile_error_from_attribute() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_from_attribute.rs");
}

#[test]
fn compile_error_to_attribute() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_to_attribute.rs");
}

#[test]
fn compile_error_has_many_attribute() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_has_many_attribute.rs");
}

#[test]
fn compile_error_partial_model_empty_entity() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_partial_model_empty_entity.rs");
}

#[test]
fn compile_error_partial_model_leading_colons() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_partial_model_leading_colons.rs");
}

#[test]
fn compile_error_partial_model_trailing_colons() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_partial_model_trailing_colons.rs");
}

#[test]
fn compile_error_partial_model_consecutive_colons() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_partial_model_consecutive_colons.rs");
}

#[test]
fn compile_error_partial_model_invalid_identifier_single_colon() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_partial_model_invalid_identifier_single_colon.rs");
}

#[test]
fn compile_error_partial_model_invalid_identifier_colon_in_middle() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_partial_model_invalid_identifier_colon_in_middle.rs");
}

#[test]
fn compile_error_partial_model_invalid_identifier_starts_with_number() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_partial_model_invalid_identifier_starts_with_number.rs");
}

#[test]
fn compile_error_partial_model_invalid_identifier_contains_hyphen() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_partial_model_invalid_identifier_contains_hyphen.rs");
}

#[test]
fn compile_error_partial_model_invalid_identifier_path_segment() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_partial_model_invalid_identifier_path_segment.rs");
}

#[test]
fn compile_error_relation_invalid_entity_path_single_colon() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_relation_invalid_entity_path_single_colon.rs");
}

#[test]
fn compile_error_relation_invalid_entity_path_colon_in_middle() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_relation_invalid_entity_path_colon_in_middle.rs");
}

#[test]
fn compile_error_relation_invalid_column_ref() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_relation_invalid_column_ref.rs");
}

#[test]
fn compile_error_relation_invalid_column_ref_path() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/compile_error_relation_invalid_column_ref_path.rs");
}
