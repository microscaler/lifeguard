//! Entity definitions for SQL generation
//!
//! This module includes entity definitions from examples/entities/
//! so they can be compiled and used for SQL generation.
//!
//! Note: Currently only ChartOfAccount is included because it uses only
//! types that implement FromSql. Other entities (Account, JournalEntry)
//! use types like serde_json::Value and rust_decimal::Decimal that don't
//! implement FromSql yet, which causes compilation errors in FromRow generation.
//!
//! TODO: Add support for these types or create simplified versions for SQL generation.

// Include only ChartOfAccount for now (uses only basic types)
#[path = "../../examples/entities/chart_of_accounts.rs"]
pub mod chart_of_accounts;

// Re-export for easier access
pub use chart_of_accounts::ChartOfAccount;
