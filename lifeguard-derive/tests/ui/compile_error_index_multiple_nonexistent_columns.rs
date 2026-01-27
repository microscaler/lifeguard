//! Test that multi-column indexes with non-existent columns fail to compile

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "test_table"]
#[index = "idx_test_multi(name, nonexistent_col, email)"]
//~^ ERROR Column 'nonexistent_col' in index 'idx_test_multi' does not exist on this struct
pub struct TestTable {
    #[primary_key]
    pub id: i32,
    pub name: String,
    pub email: String,
}

fn main() {}
