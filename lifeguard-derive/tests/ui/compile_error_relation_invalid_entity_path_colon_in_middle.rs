//! Test that entity path with colon in middle causes compile error in DeriveRelation
//!
//! This test verifies that when entity = "foo:bar" (colon in middle) is used,
//! the macro correctly reports a compile error instead of panicking.

use lifeguard_derive::DeriveRelation;

#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(belongs_to = "foo:bar::Entity")]  // ERROR: colon in middle
    Invalid,
}
