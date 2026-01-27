//! Product entity
//!
//! Products in the inventory system.

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "products"]
#[skip_from_row]  // Skip FromRow generation - NaiveDateTime doesn't implement FromSql yet
#[table_comment = "Products in the inventory system"]
#[index = "idx_products_sku(sku)"]
#[index = "idx_products_category_id(category_id)"]
#[index = "idx_products_name(name)"]
#[index = "idx_products_is_active(is_active)"]
pub struct Product {
    #[primary_key]
    pub id: uuid::Uuid,
    
    #[unique]
    #[indexed]
    #[column_type = "VARCHAR(100)"]
    pub sku: String,
    
    #[indexed]
    #[column_type = "VARCHAR(255)"]
    pub name: String,
    
    pub description: Option<String>,
    
    // Foreign key to categories
    #[foreign_key = "categories(id) ON DELETE RESTRICT"]
    #[indexed]
    pub category_id: Option<uuid::Uuid>,
    
    #[default_value = "0"]
    #[column_type = "NUMERIC(19, 4)"]
    pub price: rust_decimal::Decimal,
    
    #[default_value = "'USD'"]
    #[column_type = "VARCHAR(3)"]
    pub currency_code: String,
    
    #[default_value = "0"]
    pub stock_quantity: i32,
    
    #[default_value = "true"]
    #[indexed]
    pub is_active: bool,
    
    #[default_expr = "CURRENT_TIMESTAMP"]
    pub created_at: chrono::NaiveDateTime,
    
    #[default_expr = "CURRENT_TIMESTAMP"]
    pub updated_at: chrono::NaiveDateTime,
}
