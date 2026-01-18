//! Error types for ActiveModel operations.
//!
//! This module provides the `ActiveModelError` enum for handling errors
//! that occur during ActiveModel operations.

/// Error type for ActiveModel operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveModelError {
    /// Invalid value type for the column
    InvalidValueType {
        column: String,
        expected: String,
        actual: String,
    },
    /// Column not found
    ColumnNotFound(String),
    /// Primary key required but not set
    PrimaryKeyRequired,
    /// Record not found (e.g., UPDATE/DELETE affected zero rows)
    RecordNotFound,
    /// Database operation failed
    DatabaseError(String),
    /// Other error
    Other(String),
}

impl std::fmt::Display for ActiveModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActiveModelError::InvalidValueType {
                column,
                expected,
                actual,
            } => write!(
                f,
                "Invalid value type for column {}: expected {}, got {}",
                column, expected, actual
            ),
            ActiveModelError::ColumnNotFound(column) => {
                write!(f, "Column not found: {}", column)
            }
            ActiveModelError::PrimaryKeyRequired => {
                write!(f, "Primary key is required for this operation")
            }
            ActiveModelError::RecordNotFound => {
                write!(f, "Record not found (no rows affected)")
            }
            ActiveModelError::DatabaseError(msg) => {
                write!(f, "Database error: {}", msg)
            }
            ActiveModelError::Other(msg) => write!(f, "ActiveModel error: {}", msg),
        }
    }
}

impl std::error::Error for ActiveModelError {}
