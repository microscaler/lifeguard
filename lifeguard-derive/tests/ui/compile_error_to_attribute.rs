//! Test that malformed `to` attribute causes compile error
//!
//! This test verifies that when `to = 456` (integer instead of string) is used,
//! the macro correctly reports a compile error instead of silently ignoring it.

use lifeguard_derive::DeriveRelation;

#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = 456  // ERROR: should be a string like "super::users::Column::Id"
    )]
    User,
}
