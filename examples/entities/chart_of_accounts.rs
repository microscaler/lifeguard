//! Chart of Accounts entity
//!
//! This entity represents the hierarchical chart of accounts structure.
//! It's a self-referencing table where accounts can have parent accounts.

use lifeguard_derive::LifeModel;
use serde_json::Value;

#[derive(LifeModel)]
#[table_name = "chart_of_accounts"]
pub struct ChartOfAccount {
    #[primary_key]
    pub id: uuid::Uuid,
    
    #[unique]
    #[indexed]
    #[column_type = "VARCHAR(50)"]
    pub code: String,
    
    #[column_type = "VARCHAR(255)"]
    pub name: String,
    
    #[indexed]
    #[column_type = "VARCHAR(50)"]
    pub account_type: String, // ASSET, LIABILITY, EQUITY, REVENUE, EXPENSE
    
    // Self-referencing foreign key
    // TODO: Need foreign_key attribute support
    // #[foreign_key = "chart_of_accounts(id) ON DELETE SET NULL"]
    pub parent_id: Option<uuid::Uuid>,
    
    #[default_value = "0"]
    pub level: i32, // Hierarchy level (0 = root)
    
    #[default_value = "true"]
    #[indexed]
    pub is_active: bool,
    
    pub description: Option<String>,
    
    #[default_expr = "CURRENT_TIMESTAMP"]
    pub created_at: chrono::NaiveDateTime,
    
    #[default_expr = "CURRENT_TIMESTAMP"]
    pub updated_at: chrono::NaiveDateTime,
}

// Missing features identified:
// 1. Foreign key constraints (parent_id references chart_of_accounts(id) ON DELETE SET NULL)
// 2. Index definitions (CREATE INDEX statements need to be generated)
// 3. Table comments (COMMENT ON TABLE)
// 4. CHECK constraints (if needed for account_type enum validation)
