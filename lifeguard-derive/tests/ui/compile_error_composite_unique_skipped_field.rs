//! Test that composite unique constraints on skipped/ignored fields fail to compile

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "test_table"]
#[composite_unique = "tenant_id, cached_value, user_id"]
//~^ ERROR Column 'cached_value' in composite_unique does not exist on this struct
pub struct TestTable {
    #[primary_key]
    pub id: i32,
    pub tenant_id: i32,
    pub user_id: i32,
    #[skip]
    pub cached_value: String,
}

fn main() {}
