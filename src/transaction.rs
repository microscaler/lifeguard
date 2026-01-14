//! Transaction Module - Epic 01 Story 06
//!
//! Provides transaction support for Lifeguard, replicating SeaORM's transaction API.
//!
//! This module provides:
//! - Transaction type that implements LifeExecutor
//! - Transaction isolation levels
//! - Nested transaction support (savepoints)
//! - Commit/rollback operations

use crate::executor::{LifeError, LifeExecutor};
use may_postgres::types::ToSql;
use may_postgres::{Client, Error as PostgresError, Row};
use std::fmt;
use std::time::Instant;

#[cfg(feature = "tracing")]
use crate::metrics::tracing_helpers;
#[cfg(feature = "metrics")]
use crate::metrics::METRICS;

/// Transaction isolation level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationLevel {
    /// Read uncommitted (not supported by PostgreSQL, maps to ReadCommitted)
    ReadUncommitted,
    /// Read committed (default)
    ReadCommitted,
    /// Repeatable read
    RepeatableRead,
    /// Serializable
    Serializable,
}

impl IsolationLevel {
    /// Convert to PostgreSQL SQL syntax
    fn to_sql(&self) -> &'static str {
        match self {
            IsolationLevel::ReadUncommitted => "READ UNCOMMITTED",
            IsolationLevel::ReadCommitted => "READ COMMITTED",
            IsolationLevel::RepeatableRead => "REPEATABLE READ",
            IsolationLevel::Serializable => "SERIALIZABLE",
        }
    }
}

/// Transaction error type
#[derive(Debug)]
pub enum TransactionError {
    /// PostgreSQL error from may_postgres
    PostgresError(PostgresError),
    /// Transaction already committed or rolled back
    TransactionClosed,
    /// Nested transaction error
    NestedTransactionError(String),
    /// Other transaction errors
    Other(String),
}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionError::PostgresError(e) => {
                write!(f, "PostgreSQL error: {}", e)
            }
            TransactionError::TransactionClosed => {
                write!(f, "Transaction has already been committed or rolled back")
            }
            TransactionError::NestedTransactionError(s) => {
                write!(f, "Nested transaction error: {}", s)
            }
            TransactionError::Other(s) => {
                write!(f, "Transaction error: {}", s)
            }
        }
    }
}

impl std::error::Error for TransactionError {}

impl From<PostgresError> for TransactionError {
    fn from(err: PostgresError) -> Self {
        TransactionError::PostgresError(err)
    }
}

impl From<TransactionError> for LifeError {
    fn from(err: TransactionError) -> Self {
        match err {
            TransactionError::PostgresError(e) => LifeError::PostgresError(e),
            TransactionError::TransactionClosed => {
                LifeError::Other("Transaction closed".to_string())
            }
            TransactionError::NestedTransactionError(s) => LifeError::Other(s),
            TransactionError::Other(s) => LifeError::Other(s),
        }
    }
}

/// A database transaction
///
/// Transactions provide atomicity, consistency, isolation, and durability (ACID)
/// for database operations. All operations within a transaction are either
/// committed together or rolled back together.
///
/// # Examples
///
/// ```no_run
/// use lifeguard::{connect, MayPostgresExecutor, LifeExecutor, LifeError};
/// use lifeguard::transaction::{Transaction, IsolationLevel};
///
/// # fn main() -> Result<(), LifeError> {
/// let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")
///     .map_err(|e| LifeError::Other(format!("Connection error: {}", e)))?;
/// let executor = MayPostgresExecutor::new(client);
///
/// // Start a transaction
/// let mut transaction = executor.begin()?;
///
/// // Perform operations within the transaction
/// transaction.execute("INSERT INTO users (name) VALUES ($1)", &[&"Alice"])?;
/// transaction.execute("UPDATE users SET active = $1 WHERE name = $2", &[&true, &"Alice"])?;
///
/// // Commit the transaction
/// transaction.commit()?;
/// # Ok(())
/// # }
/// ```
pub struct Transaction {
    client: Client,
    depth: u32,
    closed: bool,
}

impl Transaction {
    /// Create a new transaction from a client
    ///
    /// This starts a new transaction with the default isolation level (ReadCommitted).
    /// For custom isolation levels, use `begin_with_isolation()`.
    pub(crate) fn new(client: Client) -> Result<Self, TransactionError> {
        Self::new_with_isolation(client, IsolationLevel::ReadCommitted)
    }

    /// Create a new transaction with a specific isolation level
    pub(crate) fn new_with_isolation(
        client: Client,
        isolation_level: IsolationLevel,
    ) -> Result<Self, TransactionError> {
        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::begin_transaction_span().entered();

        // Set isolation level if not ReadCommitted (default)
        if isolation_level != IsolationLevel::ReadCommitted {
            let isolation_sql = format!(
                "SET TRANSACTION ISOLATION LEVEL {}",
                isolation_level.to_sql()
            );
            client
                .execute(isolation_sql.as_str(), &[])
                .map_err(TransactionError::from)?;
        }

        // Start the transaction
        client
            .execute("BEGIN", &[])
            .map_err(TransactionError::from)?;

        Ok(Self {
            client,
            depth: 0,
            closed: false,
        })
    }

    /// Start a nested transaction (savepoint)
    ///
    /// Nested transactions are implemented using PostgreSQL savepoints.
    /// Each nested transaction creates a new savepoint that can be rolled back
    /// independently while keeping the outer transaction intact.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use lifeguard::{connect, MayPostgresExecutor, LifeExecutor, LifeError};
    /// # use lifeguard::transaction::Transaction;
    /// # fn main() -> Result<(), LifeError> {
    /// # let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")
    /// #     .map_err(|e| LifeError::Other(format!("Connection error: {}", e)))?;
    /// # let executor = MayPostgresExecutor::new(client);
    /// # let mut transaction = executor.begin()?;
    /// // Start a nested transaction
    /// let mut nested = transaction.begin_nested()?;
    ///
    /// // Operations in nested transaction
    /// nested.execute("INSERT INTO users (name) VALUES ($1)", &[&"Bob"])?;
    ///
    /// // Rollback only the nested transaction
    /// nested.rollback()?;
    ///
    /// // Outer transaction is still active
    /// transaction.commit()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn begin_nested(&mut self) -> Result<Transaction, TransactionError> {
        if self.closed {
            return Err(TransactionError::TransactionClosed);
        }

        let savepoint_name = format!("sp_{}", self.depth + 1);
        let savepoint_sql = format!("SAVEPOINT {}", savepoint_name);
        self.client
            .execute(savepoint_sql.as_str(), &[])
            .map_err(TransactionError::from)?;

        Ok(Transaction {
            client: self.client.clone(), // Note: may_postgres Client may need to be shared
            depth: self.depth + 1,
            closed: false,
        })
    }

    /// Commit the transaction
    ///
    /// All changes made within the transaction are permanently saved to the database.
    /// After committing, the transaction is closed and cannot be used for further operations.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction has already been committed or rolled back.
    pub fn commit(mut self) -> Result<(), TransactionError> {
        if self.closed {
            return Err(TransactionError::TransactionClosed);
        }

        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::commit_transaction_span().entered();

        if self.depth == 0 {
            // Top-level transaction: commit
            self.client
                .execute("COMMIT", &[])
                .map_err(TransactionError::from)?;
        } else {
            // Nested transaction: release savepoint
            let savepoint_name = format!("sp_{}", self.depth);
            let release_sql = format!("RELEASE SAVEPOINT {}", savepoint_name);
            self.client
                .execute(release_sql.as_str(), &[])
                .map_err(TransactionError::from)?;
        }

        self.closed = true;
        Ok(())
    }

    /// Rollback the transaction
    ///
    /// All changes made within the transaction are discarded.
    /// After rolling back, the transaction is closed and cannot be used for further operations.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction has already been committed or rolled back.
    pub fn rollback(mut self) -> Result<(), TransactionError> {
        if self.closed {
            return Err(TransactionError::TransactionClosed);
        }

        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::rollback_transaction_span().entered();

        if self.depth == 0 {
            // Top-level transaction: rollback
            self.client
                .execute("ROLLBACK", &[])
                .map_err(TransactionError::from)?;
        } else {
            // Nested transaction: rollback to savepoint
            let savepoint_name = format!("sp_{}", self.depth);
            let rollback_sql = format!("ROLLBACK TO SAVEPOINT {}", savepoint_name);
            self.client
                .execute(rollback_sql.as_str(), &[])
                .map_err(TransactionError::from)?;
        }

        self.closed = true;
        Ok(())
    }

    /// Get a reference to the underlying client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Check if the transaction is closed
    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

impl LifeExecutor for Transaction {
    fn execute(&self, query: &str, params: &[&dyn ToSql]) -> Result<u64, LifeError> {
        if self.closed {
            return Err(LifeError::Other("Transaction is closed".to_string()));
        }

        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::execute_query_span(query).entered();

        let start = Instant::now();
        let result = self.client.execute(query, params).map_err(|e| {
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
        if self.closed {
            return Err(LifeError::Other("Transaction is closed".to_string()));
        }

        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::execute_query_span(query).entered();

        let start = Instant::now();
        let result = self.client.query_one(query, params).map_err(|e| {
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
        if self.closed {
            return Err(LifeError::Other("Transaction is closed".to_string()));
        }

        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::execute_query_span(query).entered();

        let start = Instant::now();
        let result = self.client.query(query, params).map_err(|e| {
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
    fn test_isolation_level_to_sql() {
        assert_eq!(IsolationLevel::ReadUncommitted.to_sql(), "READ UNCOMMITTED");
        assert_eq!(IsolationLevel::ReadCommitted.to_sql(), "READ COMMITTED");
        assert_eq!(IsolationLevel::RepeatableRead.to_sql(), "REPEATABLE READ");
        assert_eq!(IsolationLevel::Serializable.to_sql(), "SERIALIZABLE");
    }

    #[test]
    fn test_transaction_error_display() {
        let err = TransactionError::TransactionClosed;
        assert!(err
            .to_string()
            .contains("Transaction has already been committed"));

        let err2 = TransactionError::NestedTransactionError("test".to_string());
        assert!(err2.to_string().contains("Nested transaction error"));

        let err3 = TransactionError::Other("test error".to_string());
        assert!(err3.to_string().contains("Transaction error"));
    }

    #[test]
    fn test_transaction_error_conversion() {
        let err = TransactionError::TransactionClosed;
        let life_err: LifeError = err.into();
        assert!(life_err.to_string().contains("Transaction closed"));
    }

    #[test]
    fn test_transaction_error_all_variants() {
        let err1 = TransactionError::TransactionClosed;
        assert!(err1.to_string().contains("already been committed"));

        let err2 = TransactionError::NestedTransactionError("test".to_string());
        assert!(err2.to_string().contains("Nested transaction error"));

        let err3 = TransactionError::Other("test".to_string());
        assert!(err3.to_string().contains("Transaction error"));
    }

    #[test]
    fn test_transaction_error_conversions() {
        // Test conversion from TransactionError to LifeError
        let tx_err = TransactionError::TransactionClosed;
        let life_err: LifeError = tx_err.into();
        assert!(life_err.to_string().contains("Transaction closed"));

        // Test conversion from NestedTransactionError
        let tx_err2 = TransactionError::NestedTransactionError("nested error".to_string());
        let life_err2: LifeError = tx_err2.into();
        assert!(life_err2.to_string().contains("nested error"));
    }

    #[test]
    fn test_isolation_level_equality() {
        // Test PartialEq implementation
        assert_eq!(IsolationLevel::ReadCommitted, IsolationLevel::ReadCommitted);
        assert_ne!(IsolationLevel::ReadCommitted, IsolationLevel::Serializable);
    }

    // Note: Integration tests for actual transaction operations (begin, commit, rollback)
    // will be added in Story 08 when we have test infrastructure with testcontainers.
}
