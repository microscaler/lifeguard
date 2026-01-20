//! Test that empty string in save_as attribute causes compile error
//!
//! This test verifies that when `save_as = ""` (empty string) is used,
//! the macro correctly reports a compile error instead of silently accepting it.

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "test_empty_save_as"]
pub struct TestEmptySaveAs {
    #[primary_key]
    pub id: i32,
    #[save_as = ""]
    pub name: String,
}
