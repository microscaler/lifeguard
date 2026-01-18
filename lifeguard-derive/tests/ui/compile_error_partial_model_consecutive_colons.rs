//! Test that entity path with consecutive colons causes compile error
//!
//! This test verifies that when entity = "foo::::Entity" (consecutive colons) is used,
//! the macro correctly reports a compile error instead of panicking.

use lifeguard_derive::DerivePartialModel;

#[derive(DerivePartialModel)]
#[lifeguard(entity = "users::::Entity")]  // ERROR: consecutive colons
pub struct UserPartial {
    pub id: i32,
    pub name: String,
}
