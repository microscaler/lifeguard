//! Test that entity path with single colon causes compile error in DeriveRelation
//!
//! This test verifies that when entity = ":foo" (single colon) is used,
//! the macro correctly reports a compile error instead of panicking.

use lifeguard_derive::DeriveRelation;

#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(belongs_to = ":foo::Entity")]  // ERROR: single colon at start
    Invalid,
}
