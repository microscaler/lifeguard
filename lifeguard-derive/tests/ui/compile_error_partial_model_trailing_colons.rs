//! Test that entity path with trailing colons causes compile error
//!
//! This test verifies that when entity = "foo::Entity::" (trailing colons) is used,
//! the macro correctly reports a compile error instead of panicking.

use lifeguard_derive::DerivePartialModel;

#[derive(DerivePartialModel)]
#[lifeguard(entity = "UserEntity::")]  // ERROR: trailing colons
pub struct UserPartial {
    pub id: i32,
    pub name: String,
}
