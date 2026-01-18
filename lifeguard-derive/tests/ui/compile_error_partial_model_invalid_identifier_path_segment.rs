//! Test that entity path with invalid identifier in path segment causes compile error
//!
//! This test verifies that when entity = "valid::123invalid" (valid segment followed by invalid) is used,
//! the macro correctly reports a compile error instead of panicking.

use lifeguard_derive::DerivePartialModel;

#[derive(DerivePartialModel)]
#[lifeguard(entity = "valid::123invalid")]  // ERROR: second segment starts with number
pub struct UserPartial {
    pub id: i32,
    pub name: String,
}
