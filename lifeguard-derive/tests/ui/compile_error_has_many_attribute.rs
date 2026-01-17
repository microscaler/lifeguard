//! Test that malformed `has_many` attribute causes compile error
//!
//! This test verifies that when `has_many = 789` (integer instead of string) is used,
//! the macro correctly reports a compile error instead of silently ignoring it.

use lifeguard_derive::DeriveRelation;

#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(has_many = 789)]  // ERROR: should be a string like "super::posts::Entity"
    Posts,
}
