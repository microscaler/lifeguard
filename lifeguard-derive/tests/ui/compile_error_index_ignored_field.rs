//! Test that indexes on ignored fields fail to compile with helpful error messages

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "test_table"]
#[index = "idx_test_ignored(computed_field)"]
//~^ ERROR Column 'computed_field' in index 'idx_test_ignored' does not exist on this struct
pub struct TestTable {
    #[primary_key]
    pub id: i32,
    pub name: String,
    pub email: String,
    #[ignore]
    pub computed_field: String,
}

fn main() {}
