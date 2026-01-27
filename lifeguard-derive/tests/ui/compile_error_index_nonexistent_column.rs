//! Test that indexes on non-existent columns fail to compile with helpful error messages

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "test_table"]
#[index = "idx_test_nonexistent(nonexistent_column)"]
//~^ ERROR Column 'nonexistent_column' in index 'idx_test_nonexistent' does not exist on this struct
pub struct TestTable {
    #[primary_key]
    pub id: i32,
    pub name: String,
    pub email: String,
}

fn main() {}
