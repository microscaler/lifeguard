//! Transaction Example - Epic 01 Story 06
//!
//! Demonstrates transaction usage with Lifeguard:
//! - Starting transactions
//! - Committing transactions
//! - Rolling back transactions
//! - Using different isolation levels
//! - Nested transactions (savepoints)

use lifeguard::transaction::IsolationLevel;
use lifeguard::{connect, LifeError, LifeExecutor, MayPostgresExecutor};

/// App tables for this example live in schema `lifeguard` (same as shared Kind / `get_test_connection_string.sh`).
/// URI `?options=-c search_path=lifeguard` is not always honored by the driver; ensure schema exists and set
/// `search_path` for this session so unqualified `CREATE TABLE` succeeds.
fn ensure_lifeguard_schema(executor: &MayPostgresExecutor) -> Result<(), LifeError> {
    executor.execute("CREATE SCHEMA IF NOT EXISTS lifeguard", &[])?;
    executor.execute("SET search_path TO lifeguard, public", &[])?;
    Ok(())
}

fn main() -> Result<(), LifeError> {
    // Connect to database
    let connection_string = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/postgres?options=-c%20search_path%3Dlifeguard".to_string());

    let client = connect(&connection_string)
        .map_err(|e| LifeError::Other(format!("Connection error: {e}")))?;
    let executor = MayPostgresExecutor::new(client);
    ensure_lifeguard_schema(&executor)?;

    // Example 1: Basic transaction with commit
    println!("Example 1: Basic transaction with commit");
    let transaction = executor
        .begin()
        .map_err(|e| LifeError::Other(format!("Transaction error: {e}")))?;

    // Perform operations within transaction
    transaction.execute(
        "CREATE TABLE IF NOT EXISTS example_users (id SERIAL PRIMARY KEY, name VARCHAR(100))",
        &[],
    )?;

    transaction.execute("INSERT INTO example_users (name) VALUES ($1)", &[&"Alice"])?;

    // Commit the transaction
    transaction
        .commit()
        .map_err(|e| LifeError::Other(format!("Commit error: {e}")))?;
    println!("✓ Transaction committed successfully\n");

    // Example 2: Transaction with rollback
    println!("Example 2: Transaction with rollback");
    let transaction = executor
        .begin()
        .map_err(|e| LifeError::Other(format!("Transaction error: {e}")))?;

    transaction.execute("INSERT INTO example_users (name) VALUES ($1)", &[&"Bob"])?;

    // Rollback the transaction
    transaction
        .rollback()
        .map_err(|e| LifeError::Other(format!("Rollback error: {e}")))?;
    println!("✓ Transaction rolled back successfully\n");

    // Example 3: Transaction with custom isolation level
    println!("Example 3: Transaction with Serializable isolation level");
    let transaction = executor
        .begin_with_isolation(IsolationLevel::Serializable)
        .map_err(|e| LifeError::Other(format!("Transaction error: {e}")))?;

    transaction.execute(
        "INSERT INTO example_users (name) VALUES ($1)",
        &[&"Charlie"],
    )?;

    transaction
        .commit()
        .map_err(|e| LifeError::Other(format!("Commit error: {e}")))?;
    println!("✓ Serializable transaction committed successfully\n");

    // Example 4: Query within transaction
    println!("Example 4: Query within transaction");
    let transaction = executor
        .begin()
        .map_err(|e| LifeError::Other(format!("Transaction error: {e}")))?;

    let row = transaction.query_one("SELECT COUNT(*) FROM example_users", &[])?;
    let count: i64 = row.get(0);
    println!("✓ Found {count} users in transaction");

    transaction
        .commit()
        .map_err(|e| LifeError::Other(format!("Commit error: {e}")))?;
    println!("✓ Transaction committed successfully\n");

    // Example 5: Nested transaction (savepoint)
    println!("Example 5: Nested transaction with savepoint");
    let mut outer = executor
        .begin()
        .map_err(|e| LifeError::Other(format!("Transaction error: {e}")))?;

    outer.execute("INSERT INTO example_users (name) VALUES ($1)", &[&"David"])?;

    // Start nested transaction
    let nested = outer
        .begin_nested()
        .map_err(|e| LifeError::Other(format!("Nested transaction error: {e}")))?;

    nested.execute("INSERT INTO example_users (name) VALUES ($1)", &[&"Eve"])?;

    // Rollback only the nested transaction
    nested
        .rollback()
        .map_err(|e| LifeError::Other(format!("Rollback error: {e}")))?;
    println!("✓ Nested transaction rolled back, outer transaction still active");

    // Commit the outer transaction
    outer
        .commit()
        .map_err(|e| LifeError::Other(format!("Commit error: {e}")))?;
    println!("✓ Outer transaction committed successfully\n");

    // Cleanup
    let transaction = executor
        .begin()
        .map_err(|e| LifeError::Other(format!("Transaction error: {e}")))?;
    transaction.execute("DROP TABLE IF EXISTS example_users", &[])?;
    transaction
        .commit()
        .map_err(|e| LifeError::Other(format!("Commit error: {e}")))?;

    println!("All examples completed successfully!");
    Ok(())
}
