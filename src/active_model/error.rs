//! Error types for `ActiveModel` operations.
//!
//! This module provides the `ActiveModelError` enum for handling errors
//! that occur during `ActiveModel` operations.

use super::validate_op::ValidationError;

/// Error type for `ActiveModel` operations
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
    /// Required model field was unset (`None`) on the active record when calling `to_model()`
    FieldRequired(String),
    /// Record not found (e.g., UPDATE/DELETE affected zero rows)
    RecordNotFound,
    /// Database operation failed
    DatabaseError(String),
    /// Other error
    Other(String),
    /// Validation failed (field-level and/or model-level rules)
    Validation(Vec<ValidationError>),
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
                "Invalid value type for column {column}: expected {expected}, got {actual}"
            ),
            ActiveModelError::ColumnNotFound(column) => {
                write!(f, "Column not found: {column}")
            }
            ActiveModelError::PrimaryKeyRequired => {
                write!(f, "Primary key is required for this operation")
            }
            ActiveModelError::FieldRequired(field) => {
                write!(f, "Required field not set: {field}")
            }
            ActiveModelError::RecordNotFound => {
                write!(f, "Record not found (no rows affected)")
            }
            ActiveModelError::DatabaseError(msg) => {
                write!(f, "Database error: {msg}")
            }
            ActiveModelError::Other(msg) => write!(f, "ActiveModel error: {msg}"),
            ActiveModelError::Validation(errors) => {
                if errors.is_empty() {
                    return write!(f, "Validation failed");
                }
                write!(f, "Validation failed: ")?;
                for (i, err) in errors.iter().enumerate() {
                    if i > 0 {
                        write!(f, "; ")?;
                    }
                    match &err.field {
                        Some(field) => write!(f, "{field}: {}", err.message)?,
                        None => write!(f, "{}", err.message)?,
                    }
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ActiveModelError {}
