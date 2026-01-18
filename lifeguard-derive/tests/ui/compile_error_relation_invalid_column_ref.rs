//! Test that invalid column reference causes compile error in DeriveRelation
//!
//! This test verifies that when from = "Column::123invalid" (invalid identifier) is used,
//! the macro correctly reports a compile error instead of panicking.

use lifeguard_derive::DeriveRelation;

#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(
        belongs_to = "super::users::Entity",
        from = "Column::123invalid"  // ERROR: invalid identifier
    )]
    Invalid,
}
