//! Test that indexes on skipped/ignored fields fail to compile with helpful error messages

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "test_table"]
#[index = "idx_test_skipped(cached_value)"]
//~^ ERROR Column 'cached_value' in index 'idx_test_skipped' does not exist on this struct
pub struct TestTable {
    #[primary_key]
    pub id: i32,
    pub name: String,
    pub email: String,
    #[skip]
    pub cached_value: String,
}

fn main() {}
