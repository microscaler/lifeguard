//! Test that entity path with leading colons causes compile error
//!
//! This test verifies that when entity = "::foo::Entity" (leading colons) is used,
//! the macro correctly reports a compile error instead of panicking.

use lifeguard_derive::DerivePartialModel;

#[derive(DerivePartialModel)]
#[lifeguard(entity = "::UserEntity")]  // ERROR: leading colons
pub struct UserPartial {
    pub id: i32,
    pub name: String,
}
