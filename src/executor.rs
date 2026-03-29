//! `LifeExecutor` Module - Epic 01 Story 03
//!
//! Provides the `LifeExecutor` trait that abstracts database execution over `may_postgres`.
//!
//! This trait will be the foundation for all database operations, allowing the ORM layer
//! and migrations to work with any executor implementation.

use may_postgres::{Client, Error as PostgresError, Row};
use may_postgres::types::ToSql;
use std::fmt;
use std::time::Instant;

#[cfg(feature = "metrics")]
use crate::metrics::METRICS;
#[cfg(feature = "tracing")]
use crate::metrics::tracing_helpers;


/// `LifeExecutor` error type
#[derive(Debug)]
pub enum LifeError {
    /// `PostgreSQL` error from `may_postgres`
    PostgresError(PostgresError),
    /// Query execution error
    QueryError(String),
    /// Row parsing/conversion error
    ParseError(String),
    /// Other execution errors
    Other(String),
    /// Pool-specific failures (dispatch, configuration, unsupported executor usage)
    Pool(String),
}

impl fmt::Display for LifeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LifeError::PostgresError(e) => {
                write!(f, "PostgreSQL error: {e}")
            }
            LifeError::QueryError(s) => {
                write!(f, "Query error: {s}")
            }
            LifeError::ParseError(s) => {
                write!(f, "Parse error: {s}")
            }
            LifeError::Other(s) => {
                write!(f, "Execution error: {s}")
            }
            LifeError::Pool(s) => {
                write!(f, "Pool error: {s}")
            }
        }
    }
}

impl std::error::Error for LifeError {}

impl From<PostgresError> for LifeError {
    fn from(err: PostgresError) -> Self {
        LifeError::PostgresError(err)
    }
}

/// Trait for executing database operations
///
/// This trait abstracts database execution, allowing different implementations
/// (direct client, pooled connection, transaction, etc.) to be used interchangeably.
///
/// # Examples
///
/// ```no_run
/// use lifeguard::{MayPostgresExecutor, LifeExecutor, LifeError, connect};
/// use may_postgres::Row;
///
/// # fn main() -> Result<(), LifeError> {
/// let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")
///     .map_err(|e| LifeError::Other(format!("Connection error: {}", e)))?;
/// let executor = MayPostgresExecutor::new(client);
///
/// // Execute a statement
/// let rows_affected = executor.execute("DELETE FROM users WHERE id = $1", &[&42i64])?;
///
/// // Query a single row (returns Row, extract values with .get())
/// let row = executor.query_one("SELECT COUNT(*) FROM users", &[])?;
/// let count: i64 = row.get(0);
///
/// // Query multiple rows
/// let rows = executor.query_all("SELECT id FROM users", &[])?;
/// let user_ids: Vec<i64> = rows.iter().map(|r| r.get(0)).collect();
/// # Ok(())
/// # }
/// ```
pub trait LifeExecutor {
    /// Execute a SQL statement and return the number of rows affected
    ///
    /// # Arguments
    ///
    /// * `query` - SQL query string (can contain parameters like `$1`, `$2`, etc.)
    /// * `params` - Parameters to bind to the query
    ///
    /// # Returns
    ///
    /// Returns the number of rows affected (for INSERT, UPDATE, DELETE) or `Ok(0)` for other statements.
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if the query execution fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use lifeguard::executor::LifeExecutor;
    /// # fn example(executor: &dyn LifeExecutor) -> Result<(), lifeguard::executor::LifeError> {
    /// let rows = executor.execute("UPDATE users SET active = $1 WHERE id = $2", &[&true, &42i64])?;
    /// # Ok(())
    /// # }
    /// ```
    fn execute(&self, query: &str, params: &[&dyn ToSql]) -> Result<u64, LifeError>;

    /// Execute a query and return a single row
    ///
    /// # Arguments
    ///
    /// * `query` - SQL query string
    /// * `params` - Parameters to bind to the query
    ///
    /// # Returns
    ///
    /// Returns a single `Row`, or an error if no rows or multiple rows are returned.
    /// Extract values from the row using `.get(index)` or `.get(name)`.
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if:
    /// - The query execution fails
    /// - No rows are returned
    /// - Multiple rows are returned
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use lifeguard::executor::{LifeExecutor, LifeError};
    /// # use may_postgres::Row;
    /// # let executor: &dyn LifeExecutor = todo!();
    /// let row = executor.query_one("SELECT COUNT(*) FROM users", &[])?;
    /// let count: i64 = row.get(0);
    /// # Ok::<(), LifeError>(())
    /// ```
    fn query_one(&self, query: &str, params: &[&dyn ToSql]) -> Result<Row, LifeError>;

    /// Execute a query and return all rows
    ///
    /// # Arguments
    ///
    /// * `query` - SQL query string
    /// * `params` - Parameters to bind to the query
    ///
    /// # Returns
    ///
    /// Returns a vector of all `Row` objects. Extract values from each row using `.get(index)` or `.get(name)`.
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if the query execution fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use lifeguard::executor::{LifeExecutor, LifeError};
    /// # use may_postgres::Row;
    /// # let executor: &dyn LifeExecutor = todo!();
    /// let rows = executor.query_all("SELECT id FROM users", &[])?;
    /// let user_ids: Vec<i64> = rows.iter().map(|r| r.get(0)).collect();
    /// # Ok::<(), LifeError>(())
    /// ```
    fn query_all(&self, query: &str, params: &[&dyn ToSql]) -> Result<Vec<Row>, LifeError>;

    /// Execute a statement with `sea_query::Values` (ORM / pool-safe parameter path).
    ///
    /// Default implementation converts values to `ToSql` on the stack and calls [`Self::execute`].
    fn execute_values(
        &self,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<u64, LifeError> {
        crate::query::converted_params::with_converted_value_slice(&values.0, LifeError::Other, |p| {
            self.execute(query, p)
        })
    }

    /// Query one row with `sea_query::Values`.
    fn query_one_values(
        &self,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<Row, LifeError> {
        crate::query::converted_params::with_converted_value_slice(&values.0, LifeError::Other, |p| {
            self.query_one(query, p)
        })
    }

    /// Query all rows with `sea_query::Values`.
    fn query_all_values(
        &self,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<Vec<Row>, LifeError> {
        crate::query::converted_params::with_converted_value_slice(&values.0, LifeError::Other, |p| {
            self.query_all(query, p)
        })
    }

    /// Retrieve the transparent cache provider if configured for this executor
    fn cache_provider(&self) -> Option<std::sync::Arc<dyn crate::cache::CacheProvider>> {
        None
    }
}

/// Blanket implementation to allow trait objects (`&dyn LifeExecutor`) to be passed
/// to generic functions expecting `<E: LifeExecutor>` without hitting `Sized` compiler errors.
impl LifeExecutor for &dyn LifeExecutor {
    fn execute(&self, query: &str, params: &[&dyn ToSql]) -> Result<u64, LifeError> {
        (*self).execute(query, params)
    }

    fn query_one(&self, query: &str, params: &[&dyn ToSql]) -> Result<Row, LifeError> {
        (*self).query_one(query, params)
    }

    fn query_all(&self, query: &str, params: &[&dyn ToSql]) -> Result<Vec<Row>, LifeError> {
        (*self).query_all(query, params)
    }

    fn execute_values(
        &self,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<u64, LifeError> {
        (*self).execute_values(query, values)
    }

    fn query_one_values(
        &self,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<Row, LifeError> {
        (*self).query_one_values(query, values)
    }

    fn query_all_values(
        &self,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<Vec<Row>, LifeError> {
        (*self).query_all_values(query, values)
    }

    fn cache_provider(&self) -> Option<std::sync::Arc<dyn crate::cache::CacheProvider>> {
        (*self).cache_provider()
    }
}

/// Implementation of `LifeExecutor` for `may_postgres::Client`
///
/// This is the primary executor implementation that directly uses a `may_postgres::Client`.
pub struct MayPostgresExecutor {
    client: Client,
}

impl MayPostgresExecutor {
    /// Create a new executor from a `may_postgres::Client`
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Get a reference to the underlying client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Consume the executor and return the underlying client
    pub fn into_client(self) -> Client {
        self.client
    }

    /// Start a new transaction
    ///
    /// This begins a new transaction with the default isolation level (`ReadCommitted`).
    /// The transaction must be committed or rolled back before the executor can be used again.
    ///
    /// # Errors
    ///
    /// Returns `TransactionError` if the transaction cannot be started.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use lifeguard::{MayPostgresExecutor, LifeExecutor, LifeError, connect};
    /// use lifeguard::transaction::IsolationLevel;
    ///
    /// # fn main() -> Result<(), LifeError> {
    /// let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")
    ///     .map_err(|e| LifeError::Other(format!("Connection error: {}", e)))?;
    /// let executor = MayPostgresExecutor::new(client);
    ///
    /// // Start a transaction
    /// let mut transaction = executor.begin()?;
    ///
    /// // Perform operations
    /// transaction.execute("INSERT INTO users (name) VALUES ($1)", &[&"Alice"])?;
    ///
    /// // Commit
    /// transaction.commit()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn begin(&self) -> Result<crate::transaction::Transaction, crate::transaction::TransactionError> {
        crate::transaction::Transaction::new(self.client.clone())
    }

    /// Start a new transaction with a specific isolation level
    ///
    /// # Errors
    ///
    /// Returns `TransactionError` if the transaction cannot be started.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use lifeguard::{MayPostgresExecutor, LifeExecutor, LifeError, connect};
    /// use lifeguard::transaction::{IsolationLevel, Transaction};
    ///
    /// # fn main() -> Result<(), LifeError> {
    /// let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")
    ///     .map_err(|e| LifeError::Other(format!("Connection error: {e}")))?;
    /// let executor = MayPostgresExecutor::new(client);
    ///
    /// // Start a serializable transaction
    /// let mut transaction = executor.begin_with_isolation(IsolationLevel::Serializable)?;
    /// transaction.execute("INSERT INTO users (name) VALUES ($1)", &[&"Bob"])?;
    /// transaction.commit()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn begin_with_isolation(
        &self,
        isolation_level: crate::transaction::IsolationLevel,
    ) -> Result<crate::transaction::Transaction, crate::transaction::TransactionError> {
        crate::transaction::Transaction::new_with_isolation(self.client.clone(), isolation_level)
    }

    /// Check if the underlying connection is healthy
    ///
    /// This method executes a simple query (`SELECT 1`) to verify that the
    /// connection is still alive and responsive. This is useful for connection
    /// pool health monitoring and automatic reconnection.
    ///
    /// # Returns
    ///
    /// Returns `Ok(true)` if the connection is healthy, `Ok(false)` if unhealthy,
    /// or an error if the health check itself fails.
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if the health check query fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use lifeguard::{MayPostgresExecutor, LifeError, connect};
    ///
    /// # fn main() -> Result<(), LifeError> {
    /// let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")
    ///     .map_err(|e| LifeError::Other(format!("Connection error: {e}")))?;
    /// let executor = MayPostgresExecutor::new(client);
    ///
    /// match executor.check_health() {
    ///     Ok(true) => println!("Connection is healthy"),
    ///     Ok(false) => println!("Connection is unhealthy - may need reconnection"),
    ///     Err(e) => println!("Health check failed: {e}"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn check_health(&self) -> Result<bool, LifeError> {
        crate::connection::check_connection_health(&self.client)
            .map_err(|e| LifeError::Other(format!("Health check error: {e}")))
    }

    /// Check connection health with timeout
    ///
    /// Similar to `check_health()`, but may timeout if the connection is unresponsive.
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if the health check query fails or times out.
    pub fn check_health_with_timeout(&self) -> Result<bool, LifeError> {
        crate::connection::check_connection_health_with_timeout(&self.client)
            .map_err(|e| LifeError::Other(format!("Health check error: {e}")))
    }
}

impl LifeExecutor for MayPostgresExecutor {
    fn execute(&self, query: &str, params: &[&dyn ToSql]) -> Result<u64, LifeError> {
        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::execute_query_span(query).entered();
        
        let start = Instant::now();
        let result = self.client.execute(query, params)
            .map_err(|e| {
                #[cfg(feature = "metrics")]
                METRICS.record_query_error();
                LifeError::PostgresError(e)
            });
        
        let duration = start.elapsed();
        #[cfg(feature = "metrics")]
        METRICS.record_query_duration(duration);
        
        result
    }

    fn query_one(&self, query: &str, params: &[&dyn ToSql]) -> Result<Row, LifeError> {
        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::execute_query_span(query).entered();
        
        let start = Instant::now();
        let result = self.client.query_one(query, params)
            .map_err(|e| {
                #[cfg(feature = "metrics")]
                METRICS.record_query_error();
                LifeError::PostgresError(e)
            });
        
        let duration = start.elapsed();
        #[cfg(feature = "metrics")]
        METRICS.record_query_duration(duration);
        
        result
    }

    fn query_all(&self, query: &str, params: &[&dyn ToSql]) -> Result<Vec<Row>, LifeError> {
        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::execute_query_span(query).entered();
        
        let start = Instant::now();
        let result = self.client.query(query, params)
            .map_err(|e| {
                #[cfg(feature = "metrics")]
                METRICS.record_query_error();
                LifeError::PostgresError(e)
            });
        
        let duration = start.elapsed();
        #[cfg(feature = "metrics")]
        METRICS.record_query_duration(duration);
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_life_error_display() {
        let err = LifeError::QueryError("test error".to_string());
        assert!(err.to_string().contains("Query error"));
    }

    #[test]
    fn test_empty_query_string() {
        // Empty query should be handled by may_postgres
        // We just verify error types work
        let err = LifeError::QueryError("Empty query".to_string());
        assert!(err.to_string().contains("Query error"));
    }

    #[test]
    fn test_life_error_all_variants() {
        // Test all error variants display correctly
        // Note: We can't easily create PostgresError without a connection,
        // but we can test the other variants
        let err2 = LifeError::QueryError("test".to_string());
        assert!(err2.to_string().contains("Query error"));

        let err3 = LifeError::ParseError("test".to_string());
        assert!(err3.to_string().contains("Parse error"));

        let err4 = LifeError::Other("test".to_string());
        assert!(err4.to_string().contains("Execution error"));

        let err5 = LifeError::Pool("test".to_string());
        assert!(err5.to_string().contains("Pool error"));
    }

    #[test]
    fn test_life_error_display_format() {
        // Test error display formatting
        let err = LifeError::QueryError("test query error".to_string());
        let display = err.to_string();
        assert!(display.contains("Query error"));
        assert!(display.contains("test query error"));
    }

    // Note: Integration tests for actual database operations will be added
    // when we have a test database setup (Story 08)
}
