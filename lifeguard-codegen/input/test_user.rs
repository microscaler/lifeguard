//! TestUser entity for comprehensive tests

#[table_name = "test_users"]
pub struct TestUser {
    #[primary_key]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub age: Option<i32>,
    pub active: bool,
}
