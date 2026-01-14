//! EdgeCaseUser entity for edge case tests

#[table_name = "edge_case_users"]
pub struct EdgeCaseUser {
    #[primary_key]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub age: Option<i32>,
    pub active: bool,
}
