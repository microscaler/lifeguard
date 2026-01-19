//! Test that conflicting `#[skip]` and `#[primary_key]` attributes cause compile error
//!
//! This test verifies that when a field has both `#[skip]` (or `#[ignore]`) and `#[primary_key]`
//! attributes, the macro correctly reports a compile error instead of silently skipping
//! the primary key tracking, which would lead to incorrect INSERT/UPDATE operations.

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "test_skip_primary_key"]
pub struct TestSkipPrimaryKey {
    #[primary_key]
    #[skip]  // ERROR: primary key fields cannot be skipped
    pub id: i32,
    pub name: String,
}
