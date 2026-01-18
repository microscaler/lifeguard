//! Test that valid entity paths compile successfully
//!
//! This test verifies that valid entity paths work correctly:
//! - Simple identifier: "UserEntity"
//! - Qualified path: "users::Entity"
//! - Fully qualified path: "crate::users::Entity"
//! - Super path: "super::UserEntity"

use lifeguard_derive::DerivePartialModel;

// Test simple identifier
#[derive(DerivePartialModel)]
#[lifeguard(entity = "UserEntity")]
pub struct UserPartialSimple {
    pub id: i32,
}

// Test qualified path
#[derive(DerivePartialModel)]
#[lifeguard(entity = "users::Entity")]
pub struct UserPartialQualified {
    pub id: i32,
}

// Test fully qualified path
#[derive(DerivePartialModel)]
#[lifeguard(entity = "crate::users::Entity")]
pub struct UserPartialFullyQualified {
    pub id: i32,
}

// Test super path
#[derive(DerivePartialModel)]
#[lifeguard(entity = "super::UserEntity")]
pub struct UserPartialSuper {
    pub id: i32,
}

// Test multi-segment path
#[derive(DerivePartialModel)]
#[lifeguard(entity = "crate::models::users::Entity")]
pub struct UserPartialMultiSegment {
    pub id: i32,
}
