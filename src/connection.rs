//! Connection Module - Epic 01 Story 02
//!
//! Provides connection establishment and management for may_postgres.
//!
//! This module wraps `may_postgres::Client` and provides:
//! - Connection string parsing and validation
//! - Connection establishment
//! - Error handling
//!
//! See Story 02 acceptance criteria for details.

use may_postgres::{Client, Error as PostgresError};
use std::fmt;
use std::time::Instant;

#[cfg(feature = "tracing")]
use crate::metrics::tracing_helpers;

/// Connection string for PostgreSQL
///
/// Supports PostgreSQL URI format: `postgresql://user:pass@host:port/dbname`
/// Also supports key-value format: `host=localhost user=postgres dbname=mydb`
pub type ConnectionString = String;

/// Connection error type
#[derive(Debug)]
pub enum ConnectionError {
    /// Invalid connection string format
    InvalidConnectionString(String),
    /// Network/authentication error from may_postgres
    PostgresError(PostgresError),
    /// Other connection errors
    Other(String),
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionError::InvalidConnectionString(s) => {
                write!(f, "Invalid connection string: {}", s)
            }
            ConnectionError::PostgresError(e) => {
                write!(f, "PostgreSQL error: {}", e)
            }
            ConnectionError::Other(s) => {
                write!(f, "Connection error: {}", s)
            }
        }
    }
}

impl std::error::Error for ConnectionError {}

impl From<PostgresError> for ConnectionError {
    fn from(err: PostgresError) -> Self {
        ConnectionError::PostgresError(err)
    }
}

/// Establishes a connection to PostgreSQL using may_postgres
///
/// # Arguments
///
/// * `connection_string` - PostgreSQL connection string. Supports:
///   - URI format: `postgresql://user:pass@host:port/dbname`
///   - Key-value format: `host=localhost user=postgres dbname=mydb`
///
/// # Returns
///
/// Returns a `Client` on success, or a `ConnectionError` on failure.
///
/// # Examples
///
/// ```no_run
/// use lifeguard::connection::connect;
///
/// // URI format
/// let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")?;
///
/// // Key-value format
/// let client = connect("host=localhost user=postgres dbname=mydb")?;
/// # Ok::<(), lifeguard::connection::ConnectionError>(())
/// ```
///
/// # Notes
///
/// This is a blocking call that works within coroutines. The connection
/// is established synchronously and returns immediately with a `Client`
/// that can be used for queries.
pub fn connect(connection_string: &str) -> Result<Client, ConnectionError> {
    #[cfg(feature = "tracing")]
    let _span = tracing_helpers::acquire_connection_span().entered();
    
    let start = Instant::now();
    
    // Validate connection string format
    validate_connection_string(connection_string)?;

    // Connect using may_postgres
    // Note: may_postgres::connect is a blocking call that works within coroutines
    // It returns a Client directly (no separate connection handle to manage)
    let client = may_postgres::connect(connection_string)
        .map_err(|e| ConnectionError::PostgresError(e))?;

    let duration = start.elapsed();
    #[cfg(feature = "metrics")]
    crate::metrics::METRICS.record_connection_wait(duration);

    Ok(client)
}

/// Validates a connection string format
///
/// # Arguments
///
/// * `connection_string` - PostgreSQL connection string to validate
///
/// # Returns
///
/// Returns `Ok(())` if the connection string format is valid, or an error otherwise.
///
/// # Supported Formats
///
/// - URI format: `postgresql://user:pass@host:port/dbname`
/// - Key-value format: `host=localhost user=postgres dbname=mydb`
pub fn validate_connection_string(connection_string: &str) -> Result<(), ConnectionError> {
    if connection_string.is_empty() {
        return Err(ConnectionError::InvalidConnectionString(
            "Connection string cannot be empty".to_string(),
        ));
    }

    // Check for URI format
    let is_uri_format = connection_string.starts_with("postgresql://") 
        || connection_string.starts_with("postgres://");
    
    // Check for key-value format (contains =)
    let is_key_value_format = connection_string.contains('=');

    if !is_uri_format && !is_key_value_format {
        return Err(ConnectionError::InvalidConnectionString(
            "Connection string must be in URI format (postgresql://...) or key-value format (host=...)".to_string(),
        ));
    }

    // For URI format, basic check - should have @ to separate credentials from host
    if is_uri_format && !connection_string.contains('@') {
        return Err(ConnectionError::InvalidConnectionString(
            "URI format connection string must contain '@' to separate credentials from host".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_connection_string_valid() {
        let valid_strings = vec![
            // URI format
            "postgresql://user:pass@localhost:5432/dbname",
            "postgres://user:pass@localhost:5432/dbname",
            "postgresql://postgres:postgres@localhost:5432/mydb",
            // Key-value format
            "host=localhost user=postgres dbname=mydb",
            "host=localhost port=5432 user=postgres password=secret dbname=testdb",
        ];

        for s in valid_strings {
            assert!(validate_connection_string(s).is_ok(), "Should validate: {}", s);
        }
    }

    #[test]
    fn test_validate_connection_string_invalid() {
        let invalid_strings = vec![
            "",
            "invalid://user:pass@localhost:5432/dbname",
            "postgresql://localhost:5432/dbname", // missing @ for URI format
        ];

        for s in invalid_strings {
            assert!(validate_connection_string(s).is_err(), "Should reject: {}", s);
        }
    }

    #[test]
    fn test_connection_error_display() {
        let err = ConnectionError::InvalidConnectionString("test".to_string());
        assert!(err.to_string().contains("Invalid connection string"));
    }
}
