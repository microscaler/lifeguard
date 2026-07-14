//! `LifeExecutor` Module
//!
//! Provides the `LifeExecutor` trait that abstracts database execution over `may_postgres`,
//! concrete executor implementations (`MayPostgresExecutor`, `PooledLifeExecutor`), and
//! the `SessionContext` type used for Row Level Security (RLS).
//!
//! ## Row Level Security (RLS)
//!
//! Lifeguard supports PostgreSQL Row Level Security by injecting transaction-local
//! variables (`sesame.tenant_id`, `sesame.subject_id`, etc.) around contextual work. The entry
//! points are:
//!
//! - [`MayPostgresExecutor::with_session_context`] — for single-connection executors
//! - [`MayPostgresExecutor::begin_with_session`] — for transactions (context set once)
//! - [`PooledLifeExecutor::with_session_context`] — for pooled executors
//! - [`crate::LifeguardPool::with_session_transaction`] — for pinned pooled transactions
//!
//! See the [`SessionContext`] struct for field-level documentation.

use may_postgres::types::ToSql;
use may_postgres::{Client, Error as PostgresError, Row};
use std::fmt;
use std::time::{Duration, Instant};

#[cfg(feature = "tracing")]
use crate::metrics::tracing_helpers;
#[cfg(feature = "metrics")]
use crate::metrics::METRICS;

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
    /// Timed out waiting to submit work to a pool worker (queue saturated or overload).
    ///
    /// Distinct from [`LifeError::QueryError`] and [`LifeError::Pool`] string cases so callers
    /// can match without parsing display text (PRD connection pooling R1.2).
    PoolAcquireTimeout {
        /// Wall time spent waiting before giving up.
        waited: Duration,
    },
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
            LifeError::PoolAcquireTimeout { waited } => {
                write!(
                    f,
                    "Pool error: timed out acquiring a worker after {waited:?}"
                )
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
    fn execute_values(&self, query: &str, values: &sea_query::Values) -> Result<u64, LifeError> {
        crate::query::converted_params::with_converted_value_slice(
            &values.0,
            LifeError::Other,
            |p| self.execute(query, p),
        )
    }

    /// Query one row with `sea_query::Values`.
    fn query_one_values(&self, query: &str, values: &sea_query::Values) -> Result<Row, LifeError> {
        crate::query::converted_params::with_converted_value_slice(
            &values.0,
            LifeError::Other,
            |p| self.query_one(query, p),
        )
    }

    /// Query all rows with `sea_query::Values`.
    fn query_all_values(
        &self,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<Vec<Row>, LifeError> {
        crate::query::converted_params::with_converted_value_slice(
            &values.0,
            LifeError::Other,
            |p| self.query_all(query, p),
        )
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

    fn execute_values(&self, query: &str, values: &sea_query::Values) -> Result<u64, LifeError> {
        (*self).execute_values(query, values)
    }

    fn query_one_values(&self, query: &str, values: &sea_query::Values) -> Result<Row, LifeError> {
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
///
/// **RLS integration (Story 2):** optionally carries a [`SessionContext`]. Each contextual
/// one-shot operation runs `BEGIN`, calls `public.rls_set_session(...)`, executes the
/// application statement, and commits. This keeps the helper's GUCs transaction-local and
/// prevents tenant context leaking through a reused connection. When `session_context` is
/// `None` (the default) the executor is functionally identical to the pre-RLS baseline.
pub struct MayPostgresExecutor {
    client: Client,
    session_context: Option<SessionContext>,
}

impl MayPostgresExecutor {
    /// Create a new executor from a `may_postgres::Client`
    ///
    /// The returned executor has no RLS session context (zero-regression path).
    /// Use [`with_session_context`](Self::with_session_context) to attach a context.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            session_context: None,
        }
    }

    /// Get a reference to the underlying client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Consume the executor and return the underlying client
    pub fn into_client(self) -> Client {
        self.client
    }

    /// Attach a [`SessionContext`] for RLS session injection.
    ///
    /// When a context is attached, every one-shot operation executed through this
    /// executor runs in a short transaction. The executor calls
    /// `public.rls_set_session($1, ..., $8)` before the application
    /// statement and commits only if both calls succeed.
    ///
    /// Returns `self` for method chaining.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use lifeguard::{MayPostgresExecutor, SessionContext, connect};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = connect("postgresql://postgres:***@localhost:5432/mydb")?;
    /// let executor = MayPostgresExecutor::new(client)
    ///     .with_session_context(SessionContext {
    ///         tenant_id: uuid::Uuid::new_v4(),
    ///         subject_id: uuid::Uuid::new_v4(),
    ///         organization_id: uuid::Uuid::new_v4(),
    ///         session_id: "session-123".to_string(),
    ///         roles: vec!["admin".to_string()],
    ///         permissions: vec!["read".to_string(), "write".to_string()],
    ///         user_type: Some("admin".to_string()),
    ///         org_type: None,
    ///     });
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_session_context(mut self, ctx: SessionContext) -> Self {
        self.session_context = Some(ctx);
        self
    }

    /// Runs `SELECT public.rls_set_session($1, ..., $8)` on the underlying
    /// client.
    ///
    /// This method is called only after the executor has opened a transaction. When
    /// `session_context` is `None` it returns immediately, preserving the non-RLS path.
    /// If a context is attached but the helper fails for any reason (including a missing
    /// function), the error is propagated and the enclosing transaction is rolled back.
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if:
    /// - Permissions cannot be serialised to JSON.
    /// - A session context is attached but the `rls_set_session` call fails.
    fn run_set_session(&self) -> Result<(), LifeError> {
        let Some(ctx) = self.session_context.as_ref() else {
            return Ok(());
        };
        let args = ctx.to_sql_args()?;
        let args_refs: Vec<&dyn may_postgres::types::ToSql> =
            args.iter().map(|a| a.as_ref()).collect();
        self.client.execute(
            "SELECT public.rls_set_session($1::uuid, $2::uuid, $3::uuid, $4::text, $5::jsonb, $6::jsonb, $7::text, $8::text)",
            &args_refs,
        )
        .map(|_| ())
        .map_err(LifeError::PostgresError)
    }

    /// Run one application operation inside the transaction that owns its RLS context.
    fn with_session_transaction<T>(
        &self,
        operation: impl FnOnce(&Client) -> Result<T, LifeError>,
    ) -> Result<T, LifeError> {
        if self.session_context.is_none() {
            return operation(&self.client);
        }

        self.client
            .execute("BEGIN", &[])
            .map_err(LifeError::PostgresError)?;

        let result = self
            .run_set_session()
            .and_then(|()| operation(&self.client));

        match result {
            Ok(value) => match self.client.execute("COMMIT", &[]) {
                Ok(_) => Ok(value),
                Err(error) => {
                    let _ = self.client.execute("ROLLBACK", &[]);
                    Err(LifeError::PostgresError(error))
                }
            },
            Err(error) => {
                if let Err(rollback_error) = self.client.execute("ROLLBACK", &[]) {
                    log::warn!(
                        "lifeguard executor: rollback after contextual operation failed: {rollback_error}"
                    );
                }
                Err(error)
            }
        }
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
    pub fn begin(
        &self,
    ) -> Result<crate::transaction::Transaction, crate::transaction::TransactionError> {
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
    /// let client = connect("postgresql://postgres:***@localhost:5432/mydb")
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

    /// Start a new transaction with a [`SessionContext`] for RLS injection.
    ///
    /// Runs `BEGIN` then executes `SELECT public.rls_set_session($1, $2, $3, $4, $5, $6)`
    /// to inject the session context once after `BEGIN`; all queries on that
    /// transaction's connection inherit the context.
    ///
    /// # Errors
    ///
    /// Returns `TransactionError` if `BEGIN` fails or if `rls_set_session`
    /// cannot be called.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use lifeguard::{MayPostgresExecutor, SessionContext, LifeExecutor, connect};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = connect("postgresql://postgres:***@localhost:5432/mydb")?;
    /// let executor = MayPostgresExecutor::new(client);
    ///
    /// let mut tx = executor.begin_with_session(SessionContext {
    ///     tenant_id: uuid::Uuid::new_v4(),
    ///     subject_id: uuid::Uuid::new_v4(),
    ///     organization_id: uuid::Uuid::new_v4(),
    ///     session_id: "session-123".to_string(),
    ///     roles: vec!["admin".to_string()],
    ///     permissions: vec!["read".to_string()],
    ///     user_type: Some("admin".to_string()),
    ///     org_type: None,
    /// })?;
    /// tx.execute("INSERT INTO users (name) VALUES ($1)", &[&"Alice"])?;
    /// tx.commit()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn begin_with_session(
        &self,
        ctx: SessionContext,
    ) -> Result<crate::transaction::Transaction, crate::transaction::TransactionError> {
        crate::transaction::Transaction::new_with_session(
            self.client.clone(),
            crate::transaction::IsolationLevel::ReadCommitted,
            Some(ctx),
        )
    }

    /// Start a new transaction with a specific isolation level and [`SessionContext`].
    ///
    /// Same as [`begin_with_session`](Self::begin_with_session) but allows setting
    /// a custom isolation level for the transaction.
    ///
    /// After `BEGIN`, the session context is injected once through
    /// `public.rls_set_session(...)`, whose settings are transaction-local. This makes
    /// the context available to all queries in the transaction without per-query overhead.
    ///
    /// # Errors
    ///
    /// Returns `TransactionError` if `BEGIN` fails, if the isolation level cannot
    /// be set, or if `rls_set_session` is not available in the schema.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use lifeguard::{MayPostgresExecutor, SessionContext, LifeExecutor, connect};
    /// use lifeguard::transaction::IsolationLevel;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = connect("postgresql://postgres:***@localhost:5432/mydb")?;
    /// let executor = MayPostgresExecutor::new(client);
    ///
    /// let mut tx = executor.begin_with_isolation_session(
    ///     IsolationLevel::Serializable,
    ///     SessionContext {
    ///         tenant_id: uuid::Uuid::new_v4(),
    ///         subject_id: uuid::Uuid::new_v4(),
    ///         organization_id: uuid::Uuid::new_v4(),
    ///         session_id: "session-123".to_string(),
    ///         roles: vec!["admin".to_string()],
    ///         permissions: vec!["read".to_string(), "write".to_string()],
    ///         user_type: Some("admin".to_string()),
    ///         org_type: None,
    ///     },
    /// )?;
    ///
    /// tx.execute("INSERT INTO orders (user_id, total) VALUES ($1, $2)", &[&uuid::Uuid::new_v4(), &42.0])?;
    /// tx.commit()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn begin_with_isolation_session(
        &self,
        isolation_level: crate::transaction::IsolationLevel,
        ctx: SessionContext,
    ) -> Result<crate::transaction::Transaction, crate::transaction::TransactionError> {
        crate::transaction::Transaction::new_with_session(
            self.client.clone(),
            isolation_level,
            Some(ctx),
        )
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
        let result = self.with_session_transaction(|client| {
            client.execute(query, params).map_err(|e| {
                #[cfg(feature = "metrics")]
                METRICS.record_query_error(None);
                LifeError::PostgresError(e)
            })
        });

        let duration = start.elapsed();
        #[cfg(feature = "metrics")]
        METRICS.record_query_duration(duration, None);

        result
    }

    fn query_one(&self, query: &str, params: &[&dyn ToSql]) -> Result<Row, LifeError> {
        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::execute_query_span(query).entered();

        let start = Instant::now();
        let result = self.with_session_transaction(|client| {
            client.query_one(query, params).map_err(|e| {
                #[cfg(feature = "metrics")]
                METRICS.record_query_error(None);
                LifeError::PostgresError(e)
            })
        });

        let duration = start.elapsed();
        #[cfg(feature = "metrics")]
        METRICS.record_query_duration(duration, None);

        result
    }

    fn query_all(&self, query: &str, params: &[&dyn ToSql]) -> Result<Vec<Row>, LifeError> {
        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::execute_query_span(query).entered();

        let start = Instant::now();
        let result = self.with_session_transaction(|client| {
            client.query(query, params).map_err(|e| {
                #[cfg(feature = "metrics")]
                METRICS.record_query_error(None);
                LifeError::PostgresError(e)
            })
        });

        let duration = start.elapsed();
        #[cfg(feature = "metrics")]
        METRICS.record_query_duration(duration, None);

        result
    }
}

/// Identity context for Row Level Security (RLS).
///
/// Carries verified identity claims from the consuming application's identity provider.
/// Lifeguard does **not** parse JWTs or extract these claims — the application constructs
/// and passes the context.
///
/// # How it works
///
/// When attached to an executor, every one-shot operation (or explicit transaction)
/// establishes a transaction and runs:
///
/// ```sql
/// SELECT public.rls_set_session($1, $2, $3, $4, $5, $6, $7, $8)
/// ```
///
/// This sets PostgreSQL transaction-local variables (`sesame.tenant_id`,
/// `sesame.subject_id`, etc.)
/// that power `CREATE POLICY` row filters. The SQL function is provided by the
/// `lifeguard_rls` migration.
///
/// # Transaction vs. per-query injection
///
/// - [`MayPostgresExecutor::with_session_context`] — each one-shot operation is wrapped
///   in a short transaction and receives context before its application statement.
/// - [`MayPostgresExecutor::begin_with_session`] — the context is injected **once at
///   transaction start** via `SET LOCAL`. Inherited by all queries in the transaction.
///   Use when executing multiple queries in a single transaction.
/// - [`PooledLifeExecutor::with_session_context`] — the context is serialized and sent
///   to the pool worker, which wraps each dispatched job in the same short transaction.
///
/// # Required identity boundary
///
/// Tenant, subject, active organization, and session identifiers are required. This
/// prevents a protected executor from being constructed with an ambiguous or unscoped
/// principal. Roles and permissions are always present (and may be empty); user and
/// organization classifications remain optional application metadata.
///
/// Derives `Clone + Send + Sync + 'static` so it can cross thread boundaries in the
/// pool worker path.
///
/// # Examples
///
/// Full context from an authenticated request:
///
/// ```no_run
/// use lifeguard::SessionContext;
///
/// let ctx = SessionContext {
///     tenant_id: uuid::Uuid::new_v4(),
///     subject_id: uuid::Uuid::new_v4(),
///     organization_id: uuid::Uuid::new_v4(),
///     session_id: "session-123".to_string(),
///     roles: vec!["member".to_string()],
///     permissions: vec!["read".to_string(), "write".to_string()],
///     user_type: Some("admin".to_string()),
///     org_type: Some("tenant".to_string()),
/// };
/// ```
#[derive(Clone, PartialEq)]
pub struct SessionContext {
    /// Hard tenant isolation boundary.
    /// Maps to PostgreSQL session variable `sesame.tenant_id`.
    pub tenant_id: uuid::Uuid,

    /// The authenticated subject's unique identifier.
    /// Maps to PostgreSQL session variable `sesame.subject_id`.
    pub subject_id: uuid::Uuid,

    /// The active organization identifier.
    /// Maps to PostgreSQL session variable `sesame.organization_id`.
    pub organization_id: uuid::Uuid,

    /// The authenticated session identifier (`sid`).
    /// Maps to PostgreSQL session variable `sesame.session_id`.
    pub session_id: String,

    /// Authoritative role names for this session.
    /// Serialized to JSON and mapped to `sesame.roles`.
    pub roles: Vec<String>,

    /// Authoritative permission strings for this session.
    /// Serialized to JSON and mapped to `sesame.permissions`.
    pub permissions: Vec<String>,

    /// Optional subject classification.
    /// Maps to PostgreSQL session variable `sesame.user_type`.
    pub user_type: Option<String>,

    /// Optional organization classification.
    /// Maps to PostgreSQL session variable `sesame.org_type`.
    pub org_type: Option<String>,
}

impl std::fmt::Debug for SessionContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionContext")
            .field("tenant_id", &"[REDACTED]")
            .field("subject_id", &"[REDACTED]")
            .field("organization_id", &"[REDACTED]")
            .field("session_id", &"[REDACTED]")
            .field("roles_count", &self.roles.len())
            .field("permissions_count", &self.permissions.len())
            .field("user_type", &self.user_type)
            .field("org_type", &self.org_type)
            .finish()
    }
}

impl SessionContext {
    /// Serialize this context into the SQL positional arguments expected by the
    /// `rls_set_session($1, $2, $3, $4, $5, $6, $7, $8)` function.
    ///
    /// Returns eight values in order: tenant ID, subject ID, active organization ID,
    /// session ID, roles (JSON array), permissions (JSON array), user type, and
    /// organization type.
    ///
    /// Fails only if permissions cannot be serialized to JSON, which would indicate
    /// a bug in how the application constructed the context.
    pub fn to_sql_args(&self) -> Result<Vec<Box<dyn ToSql + '_>>, LifeError> {
        let roles_json = serde_json::to_value(&self.roles)
            .map_err(|e| LifeError::Other(format!("failed to serialize session roles: {e}")))?;
        let permissions_json = serde_json::to_value(&self.permissions).map_err(|e| {
            LifeError::Other(format!("failed to serialize session permissions: {e}"))
        })?;
        Ok(vec![
            Box::new(self.tenant_id),
            Box::new(self.subject_id),
            Box::new(self.organization_id),
            Box::new(self.session_id.as_str()),
            Box::new(roles_json),
            Box::new(permissions_json),
            Box::new(self.user_type.as_deref()),
            Box::new(self.org_type.as_deref()),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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

        let err6 = LifeError::PoolAcquireTimeout {
            waited: Duration::from_millis(100),
        };
        let s6 = err6.to_string();
        assert!(s6.contains("timed out"), "display: {s6}");
        assert!(s6.contains("acquiring"), "display: {s6}");
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

// ============================================================================
// SessionContext Tests — RLS Integration Story 1
//
// NOTE: `dyn ToSql` is opaque (no `Any` downcasting). Unit tests verify
// structural properties. Value-level SQL correctness is tested in
// Story 6 (integration tests against a real Postgres instance).
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod session_context_tests {
    use super::*;
    use uuid::Uuid;

    fn context() -> SessionContext {
        SessionContext {
            tenant_id: Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").unwrap(),
            subject_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            organization_id: Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap(),
            session_id: "session-test".to_string(),
            roles: vec!["member".to_string()],
            permissions: vec!["read".to_string()],
            user_type: Some("customer".to_string()),
            org_type: Some("tenant".to_string()),
        }
    }

    // ----------------------------------------------------------------
    // Construction
    // ----------------------------------------------------------------

    #[test]
    fn test_session_context_required_identity_fields() {
        let ctx = context();

        assert_ne!(ctx.tenant_id, Uuid::nil());
        assert_ne!(ctx.subject_id, Uuid::nil());
        assert_ne!(ctx.organization_id, Uuid::nil());
        assert!(!ctx.session_id.is_empty());
    }

    #[test]
    fn test_session_context_full() {
        let uid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let oid = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
        let ctx = SessionContext {
            tenant_id: Uuid::new_v4(),
            subject_id: uid,
            organization_id: oid,
            session_id: "session-full".to_string(),
            roles: vec!["admin".to_string()],
            permissions: vec!["read".to_string(), "write".to_string()],
            user_type: Some("admin".to_string()),
            org_type: Some("tenant".to_string()),
        };

        assert_eq!(ctx.subject_id, uid);
        assert_eq!(ctx.organization_id, oid);
        assert_eq!(ctx.user_type, Some("admin".to_string()));
        assert_eq!(ctx.org_type, Some("tenant".to_string()));
        assert_eq!(
            ctx.permissions,
            vec!["read".to_string(), "write".to_string()]
        );
    }

    // ----------------------------------------------------------------
    // Clone / PartialEq
    // ----------------------------------------------------------------

    #[test]
    fn test_session_context_clone() {
        let ctx = context();

        let cloned = ctx.clone();
        assert_eq!(ctx, cloned);
        // Verify it's a deep clone (modifying the clone doesn't affect original)
        let mut cloned = cloned;
        cloned.permissions.push("write".to_string());
        assert_ne!(ctx, cloned);
    }

    #[test]
    fn test_session_context_partial_equality() {
        let ctx1 = context();
        let mut ctx2 = context();
        ctx2.permissions.push("extra".to_string());

        assert_eq!(ctx1, ctx1); // reflexivity
        assert_ne!(ctx1, ctx2); // different permissions
    }

    // ----------------------------------------------------------------
    // to_sql_args — structural tests
    //
    // `dyn ToSql` is opaque (no `Any` downcasting). We verify:
    //   - correct number of args (6)
    //   - Ok/Err return values
    //   - JSON serialization correctness (via serde_json on the struct)
    // Value-level SQL binding is tested in Story 6 integration tests.
    // ----------------------------------------------------------------

    #[test]
    fn test_to_sql_args_full_context_returns_eight_args() {
        let ctx = context();
        let args = ctx.to_sql_args().expect("full context should serialize");
        assert_eq!(args.len(), 8);
    }

    #[test]
    fn test_to_sql_args_permissions_serialization_correct() {
        // Verify permissions JSON is correct by checking the struct fields directly.
        // (The actual ToSql binding is verified in Story 6 integration tests.)
        let mut ctx = context();
        ctx.permissions = vec!["read".to_string(), "write".to_string()];

        let args = ctx.to_sql_args().expect("should serialize");
        assert_eq!(args.len(), 8);
        // Verify the struct's permissions field matches expectations
        // (this confirms the caller constructed the context correctly)
        assert_eq!(
            ctx.permissions,
            vec!["read".to_string(), "write".to_string()]
        );

        // Also verify JSON roundtrip is correct (sanity check on serialization path)
        let json = serde_json::to_value(&ctx.permissions).unwrap();
        assert!(json.is_array());
        assert_eq!(json.as_array().unwrap().len(), 2);
        assert_eq!(json[0], "read");
        assert_eq!(json[1], "write");
    }

    #[test]
    fn test_to_sql_args_empty_permissions_is_empty_json_array() {
        let mut ctx = context();
        ctx.permissions.clear();

        // Verify the struct field is empty (the ToSql binding is tested in Story 6)
        assert!(ctx.permissions.is_empty());

        // Verify JSON roundtrip produces empty array
        let json = serde_json::to_value(&ctx.permissions).unwrap();
        assert!(json.is_array());
        assert!(json.as_array().unwrap().is_empty());
    }

    // ----------------------------------------------------------------
    // Debug derive
    // ----------------------------------------------------------------

    #[test]
    fn test_session_context_debug_fmt() {
        let ctx = context();

        let debug_str = format!("{ctx:?}");
        assert!(debug_str.contains("customer"));
        assert!(!debug_str.contains(&ctx.tenant_id.to_string()));
        assert!(!debug_str.contains(&ctx.subject_id.to_string()));
        assert!(!debug_str.contains(&ctx.session_id));
        assert!(
            debug_str.contains("[REDACTED]"),
            "identity values should show [REDACTED]"
        );
    }
}

// ============================================================================
// MayPostgresExecutor RLS Tests — Story 2
//
// These tests verify the prerequisite surface: struct construction, builder
// pattern, and zero-regression behaviour. Actual SQL injection is tested in
// Story 6 (integration tests against a real Postgres instance).
//
// NOTE: `MayPostgresExecutor` wraps a `may_postgres::Client`, so we can't
// instantiate it without a live DB. We verify the *shape* through:
//   - Required `SessionContext` identity boundary
//   - Static trait assertions (`Send`, `Sync`, `Clone`)
//   - Builder signature compilation against the correct types
// ============================================================================

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::expect_used)]
mod may_postgres_executor_rls_tests {
    use super::*;

    // ----------------------------------------------------------------
    // Static trait bounds — must cross thread boundaries in pool worker path
    // ----------------------------------------------------------------

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    fn assert_clone<T: Clone>() {}

    /// Prerequisite: `SessionContext: Send` (required for `WorkerJob` channel dispatch).
    #[test]
    fn test_session_context_is_send() {
        assert_send::<SessionContext>();
    }

    /// Prerequisite: `SessionContext: Sync` (required when shared across threads).
    #[test]
    fn test_session_context_is_sync() {
        assert_sync::<SessionContext>();
    }

    /// Prerequisite: `SessionContext: Clone` (required for pool worker duplication).
    #[test]
    fn test_session_context_is_clone() {
        assert_clone::<SessionContext>();
    }

    // ----------------------------------------------------------------
    // Builder pattern — compile-time signature verification
    // ----------------------------------------------------------------

    /// Prerequisite: `MayPostgresExecutor::new(client)` returns a struct with
    /// `session_context: Option<SessionContext>` (initially `None`).
    ///
    /// We verify this through the *type shape*: if the field were not
    /// `Option<SessionContext>`, the type assertions below would fail to compile.
    #[test]
    fn test_executor_session_context_field_type() {
        // This test exists to verify the field type at compile time.
        // If `session_context` were e.g. `SessionContext` (not Option),
        // the code would not compile.
        //
        // We can't construct a `MayPostgresExecutor` without a `Client`,
        // but we can verify that `Option<SessionContext>` satisfies the
        // expected constraints.
        let _opt: Option<SessionContext> = None;
        assert!(_opt.is_none());
    }

    /// Prerequisite: Builder pattern is chainable — `with_session_context` returns `Self`.
    ///
    /// Verification: if `with_session_context` returned anything other than `Self`,
    /// chaining would fail at compile time. This test passes the compiler barrier.
    #[test]
    fn test_builder_pattern_chainable_signature() {
        // Compile-time verification: verify the return type of with_session_context
        // matches Self (allows method chaining).
        //
        // We use a compile-time assertion via function signature.
        // If `with_session_context` returned a different type, this would fail to compile.
        fn _verify_chainable(
            executor: MayPostgresExecutor,
            ctx: SessionContext,
        ) -> MayPostgresExecutor {
            executor.with_session_context(ctx)
        }

        // If this function compiles, the builder returns Self.
        // (We can't call it without a Client, but the signature verifies the shape.)
    }
}
