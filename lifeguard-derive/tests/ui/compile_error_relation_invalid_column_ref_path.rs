//! Test that invalid column reference path causes compile error in DeriveRelation
//!
//! This test verifies that when from = "foo-bar::Column::Id" (invalid path segment) is used,
//! the macro correctly reports a compile error instead of panicking.

use lifeguard_derive::DeriveRelation;

#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(
        belongs_to = "super::users::Entity",
        from = "foo-bar::Column::Id"  // ERROR: invalid path segment "foo-bar"
    )]
    Invalid,
}
