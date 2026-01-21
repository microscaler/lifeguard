//! Entity definitions for SQL generation
//!
//! This module includes entity definitions from examples/entities/
//! so they can be compiled and used for SQL generation.
//!
//! Note: Some entities use types like serde_json::Value and rust_decimal::Decimal
//! that don't implement FromSql yet, which causes compilation errors in FromRow generation.
//! For SQL generation purposes, we can still use these entities to extract metadata
//! even if FromRow compilation fails.

// Include all entity definitions organized by service
// General Ledger entities
#[path = "../../examples/entities/accounting/general-ledger/chart_of_accounts.rs"]
pub mod chart_of_accounts;

#[path = "../../examples/entities/accounting/general-ledger/account.rs"]
pub mod account;

#[path = "../../examples/entities/accounting/general-ledger/journal_entry.rs"]
pub mod journal_entry;

#[path = "../../examples/entities/accounting/general-ledger/journal_entry_line.rs"]
pub mod journal_entry_line;

#[path = "../../examples/entities/accounting/general-ledger/account_balance.rs"]
pub mod account_balance;

// Re-export for easier access
pub use chart_of_accounts::ChartOfAccount;
pub use account::Account;
pub use journal_entry::JournalEntry;
pub use journal_entry_line::JournalEntryLine;
pub use account_balance::AccountBalance;