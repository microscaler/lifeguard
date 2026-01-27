//! Raw SQL Helpers - Epic 01 Story 04
//!
//! Provides convenience functions for executing raw SQL queries.
//! These helpers replicate `SeaORM`'s `find_by_statement()` and `execute_unprepared()` functionality.

use crate::executor::{LifeExecutor, LifeError};
use may_postgres::Row;
use may_postgres::types::ToSql;

/// Execute an unprepared SQL statement
///
/// This is equivalent to `SeaORM`'s `execute_unprepared()`. It executes a raw SQL string
/// without parameter binding.
///
/// # Arguments
///
/// * `executor` - The executor to use for database operations
/// * `sql` - Raw SQL string to execute
///
/// # Returns
///
/// Returns the number of rows affected, or an error.
///
/// # Errors
///
/// Returns `LifeError` if the SQL execution fails.
///
/// # Examples
///
/// ```no_run
/// use lifeguard::{MayPostgresExecutor, LifeExecutor, execute_unprepared, connect, LifeError};
///
/// # fn main() -> Result<(), LifeError> {
/// let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")
///     .map_err(|e| LifeError::Other(format!("Connection error: {}", e)))?;
/// let executor = MayPostgresExecutor::new(client);
/// let rows = execute_unprepared(&executor, "DELETE FROM users WHERE id = 42")?;
/// # Ok(())
/// # }
/// ```
pub fn execute_unprepared<E: LifeExecutor>(executor: &E, sql: &str) -> Result<u64, LifeError> {
    executor.execute(sql, &[])
}

/// Execute a prepared statement with parameters
///
/// This is equivalent to `SeaORM`'s `execute()`. It executes a parameterized SQL statement.
///
/// # Arguments
///
/// * `executor` - The executor to use for database operations
/// * `sql` - SQL string with parameters (e.g., `$1`, `$2`)
/// * `params` - Parameters to bind to the statement
///
/// # Returns
///
/// Returns the number of rows affected, or an error.
///
/// # Errors
///
/// Returns `LifeError` if the SQL execution fails.
///
/// # Examples
///
/// ```no_run
/// use lifeguard::{MayPostgresExecutor, LifeExecutor, execute_statement, connect, LifeError};
///
/// # fn main() -> Result<(), LifeError> {
/// let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")
///     .map_err(|e| LifeError::Other(format!("Connection error: {e}")))?;
/// let executor = MayPostgresExecutor::new(client);
/// let rows = execute_statement(&executor, "DELETE FROM users WHERE id = $1", &[&42i64])?;
/// # Ok(())
/// # }
/// ```
pub fn execute_statement<E: LifeExecutor>(
    executor: &E,
    sql: &str,
    params: &[&dyn ToSql],
) -> Result<u64, LifeError> {
    executor.execute(sql, params)
}

/// Query a single row using a raw SQL statement
///
/// This is equivalent to `SeaORM`'s `find_by_statement()` for single row queries.
///
/// # Arguments
///
/// * `executor` - The executor to use for database operations
/// * `sql` - SQL query string
/// * `params` - Optional parameters to bind (empty slice for no parameters)
///
/// # Returns
///
/// Returns a single `Row`, or an error if no rows or multiple rows are returned.
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
/// use lifeguard::{MayPostgresExecutor, LifeExecutor, find_by_statement, connect, LifeError};
///
/// # fn main() -> Result<(), LifeError> {
/// let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")
///     .map_err(|e| LifeError::Other(format!("Connection error: {e}")))?;
/// let executor = MayPostgresExecutor::new(client);
/// let row = find_by_statement(&executor, "SELECT * FROM users WHERE id = $1", &[&42i64])?;
/// let name: String = row.get("name");
/// # Ok(())
/// # }
/// ```
pub fn find_by_statement<E: LifeExecutor>(
    executor: &E,
    sql: &str,
    params: &[&dyn ToSql],
) -> Result<Row, LifeError> {
    executor.query_one(sql, params)
}

/// Query multiple rows using a raw SQL statement
///
/// This is equivalent to `SeaORM`'s `find_by_statement()` for multiple row queries.
///
/// # Arguments
///
/// * `executor` - The executor to use for database operations
/// * `sql` - SQL query string
/// * `params` - Optional parameters to bind (empty slice for no parameters)
///
/// # Returns
///
/// Returns a vector of `Row` objects.
///
/// # Errors
///
/// Returns `LifeError` if the query execution fails.
///
/// # Examples
///
/// ```no_run
/// use lifeguard::{MayPostgresExecutor, LifeExecutor, find_all_by_statement, connect, LifeError};
///
/// # fn main() -> Result<(), LifeError> {
/// let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")
///     .map_err(|e| LifeError::Other(format!("Connection error: {e}")))?;
/// let executor = MayPostgresExecutor::new(client);
/// let rows = find_all_by_statement(&executor, "SELECT * FROM users WHERE active = $1", &[&true])?;
/// for row in rows {
///     let id: i64 = row.get("id");
///     let name: String = row.get("name");
/// }
/// # Ok(())
/// # }
/// ```
pub fn find_all_by_statement<E: LifeExecutor>(
    executor: &E,
    sql: &str,
    params: &[&dyn ToSql],
) -> Result<Vec<Row>, LifeError> {
    executor.query_all(sql, params)
}

/// Query a single value from a raw SQL statement
///
/// Convenience function to extract a single value from the first row's first column.
///
/// # Arguments
///
/// * `executor` - The executor to use for database operations
/// * `sql` - SQL query string
/// * `params` - Optional parameters to bind (empty slice for no parameters)
///
/// # Returns
///
/// Returns the value from the first column of the first row.
///
/// # Examples
///
/// ```no_run
/// use lifeguard::{MayPostgresExecutor, LifeExecutor, query_value, connect, LifeError};
///
/// # fn main() -> Result<(), LifeError> {
/// let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")
///     .map_err(|e| LifeError::Other(format!("Connection error: {}", e)))?;
/// let executor = MayPostgresExecutor::new(client);
/// let count: i64 = query_value(&executor, "SELECT COUNT(*) FROM users", &[])?;
/// # Ok(())
/// # }
/// ```
///
/// # Note
///
/// For better error handling, consider using `find_by_statement()` and extracting
/// values manually with `row.try_get()`.
///
/// # Errors
///
/// Returns `LifeError` if:
/// - The query execution fails
/// - No rows are returned
/// - Multiple rows are returned
/// - Value extraction/conversion fails
pub fn query_value<T, E: LifeExecutor>(
    executor: &E,
    sql: &str,
    params: &[&dyn ToSql],
) -> Result<T, LifeError>
where
    T: for<'a> may_postgres::types::FromSql<'a>,
{
    let row = executor.query_one(sql, params)?;
    // try_get signature: try_get<'a, I, T>(&'a self, idx: I) -> Result<T, Error>
    // where I is the index type (usize or &str) and T is the value type
    // Type parameters in turbofish: <I, T> (index type, then value type)
    // We specify both explicitly to help type inference
    row.try_get::<usize, T>(0)
        .map_err(|e| LifeError::ParseError(format!("Failed to extract value: {e}")))
}

#[cfg(test)]
mod tests {
    use crate::executor::LifeError;

    #[test]
    fn test_error_handling() {
        // Test that error types work correctly
        let err = LifeError::ParseError("test".to_string());
        assert!(err.to_string().contains("Parse error"));
    }

    // Note: Actual SQL execution edge cases require database connection
    // These will be tested in integration tests
}
