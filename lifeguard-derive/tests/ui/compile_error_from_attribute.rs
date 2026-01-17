//! Test that malformed `from` attribute causes compile error
//!
//! This test verifies that when `from = 123` (integer instead of string) is used,
//! the macro correctly reports a compile error instead of silently ignoring it.

use lifeguard_derive::DeriveRelation;

#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(
        belongs_to = "super::users::Entity",
        from = 123,  // ERROR: should be a string like "Column::UserId"
        to = "super::users::Column::Id"
    )]
    User,
}
