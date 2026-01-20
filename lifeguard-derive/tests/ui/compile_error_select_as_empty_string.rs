//! Test that empty string in select_as attribute causes compile error
//!
//! This test verifies that when `select_as = ""` (empty string) is used,
//! the macro correctly reports a compile error instead of silently accepting it.

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "test_empty_select_as"]
pub struct TestEmptySelectAs {
    #[primary_key]
    pub id: i32,
    #[select_as = ""]
    pub name: String,
}
