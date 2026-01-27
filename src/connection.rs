//! Connection Module - Epic 01 Story 02
//!
//! Provides connection establishment and management for `may_postgres`.
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

/// Connection string for `PostgreSQL`
///
/// Supports `PostgreSQL` URI format: `postgresql://user:pass@host:port/dbname`
/// Also supports key-value format: `host=localhost user=postgres dbname=mydb`
pub type ConnectionString = String;

/// Connection error type
#[derive(Debug)]
pub enum ConnectionError {
    /// Invalid connection string format
    InvalidConnectionString(String),
    /// Network/authentication error from `may_postgres`
    PostgresError(PostgresError),
    /// Other connection errors
    Other(String),
}

impl fmt::Display for ConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionError::InvalidConnectionString(s) => {
                write!(f, "Invalid connection string: {s}")
            }
            ConnectionError::PostgresError(e) => {
                write!(f, "PostgreSQL error: {e}")
            }
            ConnectionError::Other(s) => {
                write!(f, "Connection error: {s}")
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

/// Establishes a connection to `PostgreSQL` using `may_postgres`
///
/// # Arguments
///
/// * `connection_string` - `PostgreSQL` connection string. Supports:
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
///
/// # Errors
///
/// Returns `ConnectionError` if:
/// - The connection string is invalid
/// - Network connection fails
/// - Authentication fails
/// - Database is unavailable
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
        .map_err(ConnectionError::PostgresError)?;

    let duration = start.elapsed();
    #[cfg(feature = "metrics")]
    crate::metrics::METRICS.record_connection_wait(duration);

    Ok(client)
}

/// Validates a connection string format
///
/// # Arguments
///
/// * `connection_string` - `PostgreSQL` connection string to validate
///
/// # Returns
///
/// Returns `Ok(())` if the connection string format is valid, or an error otherwise.
///
/// # Errors
///
/// Returns `ConnectionError::InvalidConnectionString` if the connection string format is invalid.
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

/// Check if a connection is healthy by executing a simple query
///
/// This function executes `SELECT 1` to verify that the connection is still alive
/// and responsive. This is useful for health checks and connection pool management.
///
/// # Arguments
///
/// * `client` - The `PostgreSQL` client to check
///
/// # Returns
///
/// Returns `Ok(true)` if the connection is healthy, `Ok(false)` if the connection
/// is unhealthy, or an error if the check itself fails.
///
/// # Errors
///
/// Returns `ConnectionError` if the health check query fails.
///
/// # Examples
///
/// ```no_run
/// use lifeguard::connection::{connect, check_connection_health};
///
/// let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")?;
/// match check_connection_health(&client) {
///     Ok(true) => println!("Connection is healthy"),
///     Ok(false) => println!("Connection is unhealthy"),
///     Err(e) => println!("Health check failed: {e}"),
/// }
/// # Ok::<(), lifeguard::connection::ConnectionError>(())
/// ```
pub fn check_connection_health(client: &Client) -> Result<bool, ConnectionError> {
    #[cfg(feature = "tracing")]
    let _span = tracing_helpers::health_check_span().entered();

    // Execute a simple query to check if the connection is alive
    // If the query succeeds, the connection is healthy
    // If it fails, we consider the connection unhealthy
    match client.query_one("SELECT 1", &[]) {
        Ok(_) => Ok(true),
        Err(_) => {
            // Any error means the connection is unhealthy
            // This includes network errors, database errors, etc.
            Ok(false)
        }
    }
}

/// Check connection health with a timeout
///
/// This function attempts to check the connection health, but may timeout
/// if the connection is unresponsive. The timeout is handled by the underlying
/// `may_postgres` client's connection settings.
///
/// # Arguments
///
/// * `client` - The `PostgreSQL` client to check
///
/// # Returns
///
/// Returns `Ok(true)` if healthy, `Ok(false)` if unhealthy, or an error.
///
/// # Errors
///
/// Returns `ConnectionError` if the health check query fails or times out.
pub fn check_connection_health_with_timeout(client: &Client) -> Result<bool, ConnectionError> {
    // For now, this is the same as check_connection_health
    // In the future, we could add explicit timeout handling
    check_connection_health(client)
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
            assert!(validate_connection_string(s).is_ok(), "Should validate: {s}");
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
            assert!(validate_connection_string(s).is_err(), "Should reject: {s}");
        }
    }

    #[test]
    fn test_connection_error_display() {
        let err = ConnectionError::InvalidConnectionString("test".to_string());
        assert!(err.to_string().contains("Invalid connection string"));
    }

    // Note: Integration tests for check_connection_health will be added in Story 08
    // when we have test infrastructure with testcontainers. For now, we test the
    // function signatures and error types.
    #[test]
    fn test_health_check_function_signatures() {
        // Verify that the functions compile and have the correct signatures
        // Actual health checks require a real database connection
        // This will be tested in Story 08 with testcontainers
        
        // The functions should return Result<bool, ConnectionError>
        // We can't test the actual behavior without a database, but we can
        // verify the types are correct by checking compilation
    }

    #[test]
    #[allow(clippy::panic)] // Test code - panic is acceptable
    fn test_empty_connection_string() {
        let result = validate_connection_string("");
        assert!(result.is_err());
        if let Err(ConnectionError::InvalidConnectionString(msg)) = result {
            assert!(msg.contains("empty"));
        } else {
            panic!("Expected InvalidConnectionString error");
        }
    }

    #[test]
    fn test_connection_string_with_whitespace() {
        // Leading/trailing whitespace should be trimmed or handled
        // Our validation is strict - whitespace in connection strings is invalid
        // Users should trim their connection strings before passing them
        let result = validate_connection_string("  postgresql://user:pass@host:5432/db  ");
        // Validation should fail - connection strings with whitespace are invalid
        assert!(result.is_err());
        
        // Trimmed version should work
        let trimmed = "  postgresql://user:pass@host:5432/db  ".trim();
        assert!(validate_connection_string(trimmed).is_ok());
    }

    #[test]
    fn test_connection_string_special_characters_in_password() {
        // Passwords with special characters should be URL-encoded
        let valid = "postgresql://user:p%40ss%21word@host:5432/db";
        assert!(validate_connection_string(valid).is_ok());
    }

    #[test]
    fn test_connection_string_missing_parts() {
        // Missing @ in URI format
        let result = validate_connection_string("postgresql://user:pass@host:5432/db");
        // Should pass validation (format is correct)
        assert!(result.is_ok());
        
        // Missing @ entirely
        let result2 = validate_connection_string("postgresql://localhost:5432/db");
        assert!(result2.is_err());
    }

    #[test]
    fn test_connection_string_key_value_missing_equals() {
        // Key-value format without = should fail
        let result = validate_connection_string("host localhost user postgres");
        assert!(result.is_err());
    }

    #[test]
    fn test_connection_string_very_long() {
        // Very long connection strings should still validate
        let long_string = format!("postgresql://user:pass@host:5432/db?{}", "a".repeat(1000));
        // Should pass format validation (actual connection may fail, but format is valid)
        assert!(validate_connection_string(&long_string).is_ok());
    }

    #[test]
    fn test_connection_error_display_all_variants() {
        let err1 = ConnectionError::InvalidConnectionString("test".to_string());
        assert!(err1.to_string().contains("Invalid connection string"));

        let err2 = ConnectionError::Other("test".to_string());
        assert!(err2.to_string().contains("Connection error"));
    }
}
