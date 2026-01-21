//! Accounting Domain Entities
//!
//! This library provides Lifeguard entity definitions for an accounting system,
//! organized by service domain (General Ledger, Invoice, Accounts Receivable, Accounts Payable).
//!
//! ## Usage
//!
//! ```rust
//! use accounting_entities::accounting::general_ledger::ChartOfAccount;
//! use lifeguard::LifeModelTrait;
//!
//! // Access entity metadata
//! let entity = ChartOfAccount::Entity::default();
//! println!("Table: {}", entity.table_name());
//! ```
//!
//! ## Entity Organization
//!
//! Entities are organized by service domain:
//! - `accounting::general_ledger` - Core accounting entities
//! - `accounting::invoice` - Invoice management
//! - `accounting::accounts_receivable` - AR management
//! - `accounting::accounts_payable` - AP management

pub mod accounting;

// Re-export for convenience
pub use accounting::*;
