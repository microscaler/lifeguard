//! UI tests for DeriveRelation macro compile errors
//!
//! These tests verify that malformed attributes cause compile errors
//! instead of being silently ignored.

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
