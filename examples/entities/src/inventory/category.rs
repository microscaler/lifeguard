//! Product Category entity
//!
//! Categories for organizing products in the inventory system.

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "categories"]
#[skip_from_row]  // Skip FromRow generation - NaiveDateTime doesn't implement FromSql yet
#[table_comment = "Product categories for inventory organization"]
#[index = "idx_categories_code(code)"]
#[index = "idx_categories_name(name)"]
#[index = "idx_categories_is_active(is_active)"]
pub struct Category {
    #[primary_key]
    pub id: uuid::Uuid,
    
    #[unique]
    #[indexed]
    #[column_type = "VARCHAR(50)"]
    pub code: String,
    
    #[indexed]
    #[column_type = "VARCHAR(255)"]
    pub name: String,
    
    pub description: Option<String>,
    
    // Self-referencing foreign key for hierarchical categories
    #[foreign_key = "categories(id) ON DELETE SET NULL"]
    pub parent_id: Option<uuid::Uuid>,
    
    #[default_value = "true"]
    #[indexed]
    pub is_active: bool,
    
    #[default_expr = "CURRENT_TIMESTAMP"]
    pub created_at: chrono::NaiveDateTime,
    
    #[default_expr = "CURRENT_TIMESTAMP"]
    pub updated_at: chrono::NaiveDateTime,
}
