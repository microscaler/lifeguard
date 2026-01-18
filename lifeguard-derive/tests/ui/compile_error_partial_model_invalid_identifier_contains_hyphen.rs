//! Test that entity path with hyphen causes compile error
//!
//! This test verifies that when entity = "foo-bar" (contains hyphen) is used,
//! the macro correctly reports a compile error instead of panicking.

use lifeguard_derive::DerivePartialModel;

#[derive(DerivePartialModel)]
#[lifeguard(entity = "foo-bar")]  // ERROR: contains hyphen
pub struct UserPartial {
    pub id: i32,
    pub name: String,
}
