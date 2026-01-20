//! Migration state table management

use crate::{LifeExecutor, LifeError};
use sea_query::{Table, ColumnDef, TableCreateStatement, Index, IndexCreateStatement};

/// Create the `lifeguard_migrations` state tracking table
///
/// This table stores metadata about applied migrations, including:
/// - Version (timestamp)
/// - Name (human-readable)
/// - Checksum (SHA-256 hash of migration file)
/// - Applied timestamp
/// - Execution time
/// - Success status
///
/// # Returns
///
/// Returns a `TableCreateStatement` that can be executed via `SchemaManager::create_table()`
pub fn create_state_table() -> TableCreateStatement {
    Table::create()
        .table("lifeguard_migrations")
        .col(
            ColumnDef::new("version")
                .big_integer()
                .not_null()
                .primary_key()
        )
        .col(
            ColumnDef::new("name")
                .string()
                .string_len(255)
                .not_null()
        )
        .col(
            ColumnDef::new("checksum")
                .string()
                .string_len(64)
                .not_null()
        )
        .col(
            ColumnDef::new("applied_at")
                .timestamp()
                .not_null()
        )
        .col(
            ColumnDef::new("execution_time_ms")
                .integer()
                .null()
        )
        .col(
            ColumnDef::new("success")
                .boolean()
                .not_null()
                .default(false)
        )
        .to_owned()
}

/// Create index on `applied_at` for faster queries
pub fn create_state_table_index() -> IndexCreateStatement {
    Index::create()
        .name("idx_lifeguard_migrations_applied_at")
        .table("lifeguard_migrations")
        .col(sea_query::Expr::col("applied_at"))
        .to_owned()
}

/// Initialize the migration state table
///
/// Creates the `lifeguard_migrations` table and its index if they don't exist.
///
/// # Arguments
///
/// * `executor` - The database executor
///
/// # Returns
///
/// Returns `Ok(())` if the table was created successfully, or an error if it fails.
/// If the table already exists, this is a no-op (PostgreSQL's `IF NOT EXISTS` handles this).
pub fn initialize_state_table(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
    // Use raw SQL with IF NOT EXISTS for safety
    // This avoids the need to wrap executor in a Box (which has lifetime issues)
    let sql = r#"
        CREATE TABLE IF NOT EXISTS lifeguard_migrations (
            version BIGINT PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            checksum VARCHAR(64) NOT NULL,
            applied_at TIMESTAMP NOT NULL,
            execution_time_ms INTEGER,
            success BOOLEAN NOT NULL DEFAULT true
        )
    "#;
    
    executor.execute(sql, &[])?;
    
    // Create index (IF NOT EXISTS)
    let index_sql = r#"
        CREATE INDEX IF NOT EXISTS idx_lifeguard_migrations_applied_at 
        ON lifeguard_migrations(applied_at)
    "#;
    
    executor.execute(index_sql, &[])?;
    
    Ok(())
}
