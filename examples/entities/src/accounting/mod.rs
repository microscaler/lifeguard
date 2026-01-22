//! Accounting service entities
//!
//! This module contains all accounting-related entities organized by service domain.

pub mod general_ledger;
pub mod invoice;
pub mod accounts_receivable;
pub mod accounts_payable;

// Re-export entities for convenience
pub use general_ledger::*;
pub use invoice::*;
pub use accounts_receivable::*;
pub use accounts_payable::*;
