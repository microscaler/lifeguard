//! User entity definition
//!
//! This is an example of how to define entities using Rust structs.
//! The codegen tool will parse this and generate Entity, Model, Column, etc.

#[table_name = "users"]
pub struct User {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    
    pub email: String,
    
    pub name: Option<String>,
}
