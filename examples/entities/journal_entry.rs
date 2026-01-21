//! Journal Entry entity
//!
//! Double-entry bookkeeping records.

use lifeguard_derive::LifeModel;
use serde_json::Value;

#[derive(LifeModel)]
#[table_name = "journal_entries"]
pub struct JournalEntry {
    #[primary_key]
    pub id: uuid::Uuid,
    
    #[unique]
    #[indexed]
    #[column_type = "VARCHAR(50)"]
    pub entry_number: String,
    
    #[indexed]
    pub entry_date: chrono::NaiveDate,
    
    pub description: String,
    
    #[column_type = "VARCHAR(100)"]
    pub reference_number: Option<String>, // External reference (invoice number, etc.)
    
    #[column_type = "VARCHAR(50)"]
    pub source_type: Option<String>, // MANUAL, INVOICE, PAYMENT, ADJUSTMENT, etc.
    
    pub source_id: Option<uuid::Uuid>, // Reference to source document
    
    pub fiscal_period_id: Option<uuid::Uuid>, // Reference to fiscal period
    
    #[default_value = "'DRAFT'"]
    #[indexed]
    #[column_type = "VARCHAR(20)"]
    pub status: String, // DRAFT, POSTED, REVERSED
    
    pub posted_at: Option<chrono::NaiveDateTime>,
    
    pub posted_by: Option<uuid::Uuid>, // User who posted the entry
    
    #[default_value = "0"]
    #[column_type = "NUMERIC(19, 4)"]
    pub total_debit: rust_decimal::Decimal,
    
    #[default_value = "0"]
    #[column_type = "NUMERIC(19, 4)"]
    pub total_credit: rust_decimal::Decimal,
    
    #[default_value = "'USD'"]
    #[column_type = "VARCHAR(3)"]
    pub currency_code: String,
    
    #[indexed]
    pub company_id: Option<uuid::Uuid>, // Multi-company support
    
    pub metadata: Option<Value>, // JSONB
    
    #[default_expr = "CURRENT_TIMESTAMP"]
    pub created_at: chrono::NaiveDateTime,
    
    #[default_expr = "CURRENT_TIMESTAMP"]
    pub updated_at: chrono::NaiveDateTime,
    
    pub created_by: Option<uuid::Uuid>,
    
    pub updated_by: Option<uuid::Uuid>,
}

// Missing features identified:
// 1. CHECK constraint: total_debit = total_credit (CONSTRAINT check_balanced_entry)
// 2. Composite index: (source_type, source_id)
// 3. Index definitions (CREATE INDEX statements)
// 4. Table comments (COMMENT ON TABLE)
