//! Test that empty entity path causes compile error
//!
//! This test verifies that when entity = "" (empty string) is used,
//! the macro correctly reports a compile error instead of panicking.

use lifeguard_derive::DerivePartialModel;

#[derive(DerivePartialModel)]
#[lifeguard(entity = "")]  // ERROR: empty entity path
pub struct UserPartial {
    pub id: i32,
    pub name: String,
}
