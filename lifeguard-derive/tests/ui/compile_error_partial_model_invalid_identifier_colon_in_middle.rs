//! Test that entity path with colon in middle causes compile error
//!
//! This test verifies that when entity = "foo:bar" (colon in middle) is used,
//! the macro correctly reports a compile error instead of panicking.

use lifeguard_derive::DerivePartialModel;

#[derive(DerivePartialModel)]
#[lifeguard(entity = "foo:bar")]  // ERROR: colon in middle
pub struct UserPartial {
    pub id: i32,
    pub name: String,
}
