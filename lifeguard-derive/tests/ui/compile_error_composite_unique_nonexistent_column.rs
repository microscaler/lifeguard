//! Test that composite unique constraints on non-existent columns fail to compile

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "test_table"]
#[composite_unique = "tenant_id, nonexistent_col, user_id"]
//~^ ERROR Column 'nonexistent_col' in composite_unique does not exist on this struct
pub struct TestTable {
    #[primary_key]
    pub id: i32,
    pub tenant_id: i32,
    pub user_id: i32,
}

fn main() {}
