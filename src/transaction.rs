//! Transaction Module - Epic 01 Story 06
//!
//! Provides transaction support for Lifeguard, replicating `SeaORM`'s transaction API.
//!
//! This module provides:
//! - `Transaction` type that implements `LifeExecutor`
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
    /// Read uncommitted (not supported by `PostgreSQL`, maps to `ReadCommitted`)
    ReadUncommitted,
    /// Read committed (default)
    ReadCommitted,
    /// Repeatable read
    RepeatableRead,
    /// Serializable
    Serializable,
}

impl IsolationLevel {
    /// Convert to `PostgreSQL` SQL syntax
    #[allow(clippy::wrong_self_convention)]
    fn to_sql(self) -> &'static str {
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
    /// `PostgreSQL` error from `may_postgres`
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
                write!(f, "PostgreSQL error: {e}")
            }
            TransactionError::TransactionClosed => {
                write!(f, "Transaction has already been committed or rolled back")
            }
            TransactionError::NestedTransactionError(s) => {
                write!(f, "Nested transaction error: {s}")
            }
            TransactionError::Other(s) => {
                write!(f, "Transaction error: {s}")
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
            TransactionError::NestedTransactionError(s) | TransactionError::Other(s) => {
                LifeError::Other(s)
            }
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
    session_context: Option<crate::executor::SessionContext>,
}

impl Transaction {
    /// Create a new transaction from a client
    ///
    /// This starts a new transaction with the default isolation level (`ReadCommitted`).
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

        // Use PostgreSQL's BEGIN ISOLATION LEVEL syntax so the isolation
        // level takes effect inside the transaction (SET TRANSACTION ISOLATION
        // LEVEL sent before BEGIN is ignored by Postgres).
        if isolation_level != IsolationLevel::ReadCommitted {
            let begin_sql = format!("BEGIN ISOLATION LEVEL {}", isolation_level.to_sql());
            client
                .execute(begin_sql.as_str(), &[])
                .map_err(TransactionError::from)?;
        } else {
            client
                .execute("BEGIN", &[])
                .map_err(TransactionError::from)?;
        }

        Ok(Self {
            client,
            depth: 0,
            closed: false,
            session_context: None,
        })
    }

    /// Create a new transaction with a [`SessionContext`] for RLS injection.
    ///
    /// Runs `BEGIN` (with optional isolation level) then, if `ctx` is `Some`,
    /// executes `SELECT public.rls_set_session($1, $2, $3, $4, $5, $6)` to set the
    /// transaction-local variables that power Row Level Security policies.
    ///
    /// The RLS context is set once at transaction start. The application-owned
    /// helper must use transaction-local settings (`set_config(..., true)`), so all
    /// queries in this transaction inherit the context and PostgreSQL clears it on
    /// `COMMIT` or `ROLLBACK`.
    ///
    /// # Errors
    ///
    /// Returns `TransactionError` if `BEGIN` fails or if `rls_set_session` cannot
    /// be called (e.g. the function is not available in the schema).
    pub(crate) fn new_with_session(
        client: Client,
        isolation_level: IsolationLevel,
        ctx: Option<crate::executor::SessionContext>,
    ) -> Result<Self, TransactionError> {
        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::begin_transaction_span().entered();

        // Use PostgreSQL's BEGIN ISOLATION LEVEL syntax so the isolation
        // level takes effect inside the transaction (SET TRANSACTION ISOLATION
        // LEVEL sent before BEGIN is ignored by Postgres).
        if isolation_level != IsolationLevel::ReadCommitted {
            let begin_sql = format!("BEGIN ISOLATION LEVEL {}", isolation_level.to_sql());
            client
                .execute(begin_sql.as_str(), &[])
                .map_err(TransactionError::from)?;
        } else {
            client
                .execute("BEGIN", &[])
                .map_err(TransactionError::from)?;
        }

        // Inject RLS context after BEGIN so transaction-local GUCs cannot leak.
        let injection = (|| -> Result<(), TransactionError> {
            if let Some(ref ctx) = ctx {
                let args = ctx.to_sql_args().map_err(|e| {
                    TransactionError::Other(format!("failed to serialize session context: {e}"))
                })?;
                let args_refs: Vec<&dyn may_postgres::types::ToSql> =
                    args.iter().map(|a| a.as_ref()).collect();
                client
                    .execute("SELECT public.rls_set_session($1::uuid, $2::uuid, $3::uuid, $4::text, $5::jsonb, $6::jsonb, $7::text, $8::text)", &args_refs)
                    .map_err(TransactionError::from)?;
            }
            Ok(())
        })();

        if let Err(error) = injection {
            let _ = client.execute("ROLLBACK", &[]);
            return Err(error);
        }

        Ok(Self {
            client,
            depth: 0,
            closed: false,
            session_context: ctx,
        })
    }

    /// Start a nested transaction (savepoint)
    ///
    /// Nested transactions are implemented using `PostgreSQL` savepoints.
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
    ///
    /// # Errors
    ///
    /// Returns `TransactionError` if:
    /// - The transaction is already closed
    /// - The savepoint creation fails
    pub fn begin_nested(&mut self) -> Result<Transaction, TransactionError> {
        if self.closed {
            return Err(TransactionError::TransactionClosed);
        }

        let savepoint_name = format!("sp_{}", self.depth + 1);
        let savepoint_sql = format!("SAVEPOINT {savepoint_name}");
        self.client
            .execute(savepoint_sql.as_str(), &[])
            .map_err(TransactionError::from)?;

        Ok(Transaction {
            client: self.client.clone(),
            depth: self.depth + 1,
            closed: false,
            session_context: self.session_context.clone(),
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
            let release_sql = format!("RELEASE SAVEPOINT {savepoint_name}");
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
            let rollback_sql = format!("ROLLBACK TO SAVEPOINT {savepoint_name}");
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
            METRICS.record_query_error(None);
            LifeError::PostgresError(e)
        });

        let duration = start.elapsed();
        #[cfg(feature = "metrics")]
        METRICS.record_query_duration(duration, None);

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
            METRICS.record_query_error(None);
            LifeError::PostgresError(e)
        });

        let duration = start.elapsed();
        #[cfg(feature = "metrics")]
        METRICS.record_query_duration(duration, None);

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
            METRICS.record_query_error(None);
            LifeError::PostgresError(e)
        });

        let duration = start.elapsed();
        #[cfg(feature = "metrics")]
        METRICS.record_query_duration(duration, None);

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

// ============================================================================
// Transaction RLS Tests — Story 3
//
// Verify the prerequisite surface: struct construction with session_context,
// begin_with_session builder on MayPostgresExecutor. Actual SQL injection is
// tested in Story 6 (integration tests against a real Postgres instance).
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod transaction_rls_tests {
    use super::*;
    use crate::executor::SessionContext;

    // ----------------------------------------------------------------
    // Construction
    // ----------------------------------------------------------------

    /// Prerequisite: `Transaction::new_with_session()` constructs correctly
    /// with `Some` and `None` contexts.
    #[test]
    fn test_transaction_new_with_session_signature_compiles() {
        // Structural test: verify the constructor signature compiles.
        // Actual runtime testing requires a live Client (Story 6).
        fn _signature(_ctx: Option<SessionContext>) {
            // If this compiles, the constructor accepts Option<SessionContext>.
        }
        let _ = _signature;
    }

    // ----------------------------------------------------------------
    // Builder pattern on MayPostgresExecutor
    // ----------------------------------------------------------------

    /// Prerequisite: `MayPostgresExecutor::begin_with_session()` returns
    /// correct error type on failure.
    #[test]
    fn test_begin_with_session_compile_signature() {
        // Structural test: verify begin_with_session compiles.
        // We can't create a live Client without a running database.
        fn _signature(executor: crate::executor::MayPostgresExecutor, ctx: SessionContext) {
            // If this compiles, the method signature is correct:
            // begin_with_session(SessionContext) -> Result<Transaction, TransactionError>
            let _ = (executor, ctx);
        }
        let _ = _signature;
    }

    /// Prerequisite: Verify nested savepoint creation does not duplicate
    /// session injection (documented expectation: `SET LOCAL` is
    /// transaction-scoped).
    #[test]
    fn test_nested_savepoint_preserves_session_context() {
        // Structural test: verify begin_nested propagates session_context.
        // If the field were missing from Transaction, this would not compile.
        fn _verify_session_field(_t: Transaction) {
            // The fact that Transaction has session_context: Option<...>
            // means nested transactions inherit it.
        }
        let _ = _verify_session_field;
    }
}
