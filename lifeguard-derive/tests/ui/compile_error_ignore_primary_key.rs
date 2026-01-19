//! Test that conflicting `#[ignore]` and `#[primary_key]` attributes cause compile error
//!
//! This test verifies that when a field has both `#[ignore]` and `#[primary_key]`
//! attributes, the macro correctly reports a compile error.

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "test_ignore_primary_key"]
pub struct TestIgnorePrimaryKey {
    #[primary_key]
    #[ignore]  // ERROR: primary key fields cannot be ignored
    pub id: i32,
    pub name: String,
}
