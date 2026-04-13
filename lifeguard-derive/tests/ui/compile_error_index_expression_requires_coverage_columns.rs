//! Expression index keys must list coverage columns after ` | `.

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "test_expr_index"]
#[index = "idx_lower_email(lower(email))"]
//~^ ERROR Invalid index definition: expression or parenthesized keys must use
pub struct TestExprIndex {
    #[primary_key]
    pub id: i32,
    pub email: String,
}

fn main() {}
