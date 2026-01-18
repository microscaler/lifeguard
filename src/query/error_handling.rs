//! Error detection and classification utilities.
//!
//! This module provides utilities for detecting and classifying database errors,
//! particularly for distinguishing "no rows found" errors from other database errors.

use crate::executor::LifeError;

/// Check if an error represents a "no rows found" condition.
///
///
/// This function uses specific patterns to detect "no rows found" errors while
/// avoiding false positives from legitimate database errors like "table not found",
/// "column not found", "function not found", or "constraint not found".
///
/// # Arguments
///
/// * `error` - The error to check
///
/// # Returns
///
/// Returns `true` if the error indicates no rows were found, `false` otherwise.
pub(crate) fn is_no_rows_error(error: &LifeError) -> bool {
    match error {
        LifeError::PostgresError(pg_error) => {
            // Check the underlying PostgreSQL error message
            // may_postgres typically returns errors with specific messages for "no rows"
            let error_msg = pg_error.to_string().to_lowercase();
            // Only match specific "no rows" patterns, not the broad "not found"
            error_msg.contains("no rows")
                || error_msg.contains("no row")
                || error_msg.contains("row not found")
                || error_msg.contains("no rows returned")
                || error_msg.contains("expected one row")
        }
        LifeError::QueryError(msg) => {
            // Check QueryError messages - be specific about "no rows" patterns
            let error_msg = msg.to_lowercase();
            error_msg.contains("no rows")
                || error_msg.contains("no row")
                || error_msg.contains("row not found")
                || error_msg.contains("no rows returned")
                || error_msg.contains("expected one row")
        }
        LifeError::ParseError(_) => {
            // Parse errors are never "no rows found" errors
            false
        }
        LifeError::Other(msg) => {
            // Check Other error messages - be specific about "no rows" patterns
            let error_msg = msg.to_lowercase();
            error_msg.contains("no rows")
                || error_msg.contains("no row")
                || error_msg.contains("row not found")
                || error_msg.contains("no rows returned")
                || error_msg.contains("expected one row")
        }
    }
}
