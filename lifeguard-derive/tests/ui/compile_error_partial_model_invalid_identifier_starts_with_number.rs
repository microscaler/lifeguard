//! Test that entity path starting with number causes compile error
//!
//! This test verifies that when entity = "123abc" (starts with number) is used,
//! the macro correctly reports a compile error instead of panicking.

use lifeguard_derive::DerivePartialModel;

#[derive(DerivePartialModel)]
#[lifeguard(entity = "123abc")]  // ERROR: starts with number
pub struct UserPartial {
    pub id: i32,
    pub name: String,
}
