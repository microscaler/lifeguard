//! Inventory Item entity
//!
//! Tracks individual inventory items with location and status.

use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[table_name = "inventory_items"]
#[skip_from_row]  // Skip FromRow generation - NaiveDate doesn't implement FromSql yet
#[table_comment = "Individual inventory items with location tracking"]
#[composite_unique = "product_id, location_code, batch_number"]
#[index = "idx_inventory_items_product_id(product_id)"]
#[index = "idx_inventory_items_location_code(location_code)"]
#[index = "idx_inventory_items_status(status)"]
#[index = "idx_inventory_items_expiry_date(expiry_date)"]
pub struct InventoryItem {
    #[primary_key]
    pub id: uuid::Uuid,
    
    // Foreign key to products
    #[foreign_key = "products(id) ON DELETE CASCADE"]
    #[indexed]
    pub product_id: uuid::Uuid,
    
    #[indexed]
    #[column_type = "VARCHAR(50)"]
    pub location_code: String,
    
    #[column_type = "VARCHAR(100)"]
    pub batch_number: String,
    
    #[default_value = "1"]
    pub quantity: i32,
    
    #[indexed]
    #[column_type = "VARCHAR(50)"]
    pub status: String, // AVAILABLE, RESERVED, SOLD, DAMAGED
    
    #[indexed]
    pub expiry_date: Option<chrono::NaiveDate>,
    
    #[default_expr = "CURRENT_TIMESTAMP"]
    pub created_at: chrono::NaiveDateTime,
    
    #[default_expr = "CURRENT_TIMESTAMP"]
    pub updated_at: chrono::NaiveDateTime,
}
