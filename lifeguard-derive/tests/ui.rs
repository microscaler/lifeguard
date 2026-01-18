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
fn compile_pass_partial_model_valid_paths() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/compile_pass_partial_model_valid_paths.rs");
}
