//! Integration tests for Migration system
//!
//! These tests validate that the migration system works correctly
//! with a real PostgreSQL database on the Kind cluster.
//!
//! Test flow:
//! 1. Create a test schema
//! 2. Create a sample migration file
//! 3. Register and run the migration
//! 4. Verify migration state table
//! 5. Verify actual database schema

use lifeguard::{
    LifeExecutor,
    test_helpers::TestDatabase,
    migration::{
        Migrator, initialize_state_table,
        MigrationRecord, Migration, SchemaManager,
        register_migration,
    },
    LifeError,
};
use sea_query::{Table, ColumnDef, Index, Expr};
use std::path::PathBuf;
use std::fs;
use std::env;

/// Test migration implementation
struct TestMigration {
    version: i64,
    name: String,
}

impl Migration for TestMigration {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn version(&self) -> i64 {
        self.version
    }
    
    fn up(&self, manager: &SchemaManager<'_>) -> Result<(), LifeError> {
        // Create a test table
        let table = Table::create()
            .table("migration_test_table")
            .col(ColumnDef::new("id").integer().not_null().primary_key())
            .col(ColumnDef::new("name").string().string_len(255).not_null())
            .col(ColumnDef::new("created_at").timestamp().not_null())
            .to_owned();
        
        manager.create_table(table)?;
        
        // Create an index
        let index = Index::create()
            .name("idx_migration_test_name")
            .table("migration_test_table")
            .col(Expr::col("name"))
            .to_owned();
        
        manager.create_index(index)?;
        
        Ok(())
    }
    
    fn down(&self, manager: &SchemaManager<'_>) -> Result<(), LifeError> {
        // Drop the index first
        let index = Index::drop()
            .name("idx_migration_test_name")
            .table("migration_test_table")
            .to_owned();
        
        manager.drop_index(index)?;
        
        // Drop the table
        let table = Table::drop()
            .table("migration_test_table")
            .to_owned();
        
        manager.drop_table(table)?;
        
        Ok(())
    }
}

/// Helper to create a sample migration file (for discovery testing)
fn create_test_migration_file(migrations_dir: &PathBuf, version: i64, name: &str) -> Result<PathBuf, std::io::Error> {
    let filename = format!("m{:014}_{}.rs", version, name);
    let filepath = migrations_dir.join(&filename);
    
    // Create a minimal migration file (just for discovery/checksum testing)
    let migration_code = format!(
        r#"//! Test migration: {}
//! Version: {}
//! This is a test migration file for integration testing.
"#,
        name, version
    );
    
    fs::write(&filepath, migration_code)?;
    Ok(filepath)
}

/// Helper to verify migration state table
fn verify_migration_state(
    executor: &dyn LifeExecutor,
    expected_version: i64,
    expected_name: &str,
) -> Result<(), lifeguard::LifeError> {
    let sql = r#"
        SELECT version, name, checksum, applied_at, execution_time_ms, success
        FROM lifeguard_migrations
        WHERE version = $1
    "#;
    
    let rows = executor.query_all(sql, &[&expected_version])?;
    
    assert_eq!(rows.len(), 1, "Expected exactly one migration record");
    
    let record = MigrationRecord::from_row(&rows[0])?;
    assert_eq!(record.version, expected_version);
    assert_eq!(record.name, expected_name);
    assert!(record.success, "Migration should be marked as successful");
    assert!(record.checksum.len() == 64, "Checksum should be SHA-256 (64 hex chars)");
    
    Ok(())
}

/// Helper to verify database schema
fn verify_database_schema(executor: &dyn LifeExecutor, version: i64) -> Result<(), lifeguard::LifeError> {
    let table_name = format!("migration_test_table_{}", version);
    let index_name = format!("idx_migration_test_name_{}", version);
    
    // Check that the table exists
    let table_check_sql = format!(
        "SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_schema = 'public' 
            AND table_name = '{}'
        )",
        table_name
    );
    
    let rows = executor.query_all(&table_check_sql, &[])?;
    assert_eq!(rows.len(), 1);
    let exists: bool = rows[0].get(0);
    assert!(exists, "{} should exist", table_name);
    
    // Check table columns
    let columns_sql = format!(
        "SELECT column_name, data_type, is_nullable
        FROM information_schema.columns
        WHERE table_schema = 'public' 
        AND table_name = '{}'
        ORDER BY ordinal_position",
        table_name
    );
    
    let rows = executor.query_all(&columns_sql, &[])?;
    assert_eq!(rows.len(), 3, "Table should have 3 columns");
    
    // Verify column: id
    let col_name: String = rows[0].get(0);
    let data_type: String = rows[0].get(1);
    assert_eq!(col_name, "id");
    assert!(data_type.contains("integer") || data_type.contains("int"));
    
    // Verify column: name
    let col_name: String = rows[1].get(0);
    let data_type: String = rows[1].get(1);
    assert_eq!(col_name, "name");
    assert!(data_type.contains("character") || data_type.contains("text") || data_type.contains("varchar"));
    
    // Verify column: created_at
    let col_name: String = rows[2].get(0);
    let data_type: String = rows[2].get(1);
    assert_eq!(col_name, "created_at");
    assert!(data_type.contains("timestamp") || data_type.contains("time"));
    
    // Check that the index exists
    let index_check_sql = format!(
        "SELECT EXISTS (
            SELECT FROM pg_indexes
            WHERE schemaname = 'public'
            AND tablename = '{}'
            AND indexname = '{}'
        )",
        table_name, index_name
    );
    
    let rows = executor.query_all(&index_check_sql, &[])?;
    assert_eq!(rows.len(), 1);
    let exists: bool = rows[0].get(0);
    assert!(exists, "Index {} should exist", index_name);
    
    Ok(())
}

#[test]
fn test_migration_lifecycle() {
    // Get test database connection
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let executor = test_db.executor().expect("Failed to get executor");
    
    // Use the default 'public' schema for simplicity
    // Table names will be unique based on migration version
    
    // Initialize migration state table
    initialize_state_table(&executor)
        .expect("Failed to initialize migration state table");
    
    // Create a test migration and register it
    let version = 20240120120000i64;
    let migration_name = "create_test_table";
    let migration = TestMigration {
        version,
        name: migration_name.to_string(),
    };
    
    register_migration(Box::new(migration))
        .expect("Failed to register migration");
    
    // Create temporary directory for migration files (for discovery testing)
    let temp_dir = env::temp_dir().join(format!("lifeguard_migration_test_{}", version));
    fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");
    
    // Create a test migration file (for discovery/checksum testing)
    let _migration_file = create_test_migration_file(&temp_dir, version, migration_name)
        .expect("Failed to create test migration file");
    
    // Create migrator
    let migrator = Migrator::new(&temp_dir);
    
    // Check initial status (should show pending migration)
    let status = migrator.status(&executor)
        .expect("Failed to get migration status");
    assert_eq!(status.pending_count, 1, "Should have 1 pending migration");
    assert_eq!(status.applied_count, 0, "Should have 0 applied migrations");
    
    // Run the migration
    let applied = migrator.up(&executor, None)
        .expect("Failed to run migration");
    assert_eq!(applied, 1, "Should have applied 1 migration");
    
    // Verify migration state
    verify_migration_state(&executor, version, migration_name)
        .expect("Failed to verify migration state");
    
    // Verify database schema
    verify_database_schema(&executor, version)
        .expect("Failed to verify database schema");
    
    // Check status again (should show applied migration)
    let status = migrator.status(&executor)
        .expect("Failed to get migration status");
    assert_eq!(status.pending_count, 0, "Should have 0 pending migrations");
    assert_eq!(status.applied_count, 1, "Should have 1 applied migration");
    
    // Cleanup: Drop test table
    let table_name = format!("migration_test_table_{}", version);
    let drop_table_sql = format!("DROP TABLE IF EXISTS {} CASCADE", table_name);
    let _ = executor.execute(&drop_table_sql, &[]);
    
    // Cleanup: Remove temp directory
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_migration_state_table_creation() {
    // Test that the migration state table can be created and queried
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let executor = test_db.executor().expect("Failed to get executor");
    
    // Initialize state table
    initialize_state_table(&executor)
        .expect("Failed to initialize migration state table");
    
    // Verify table exists
    let check_sql = r#"
        SELECT EXISTS (
            SELECT FROM information_schema.tables 
            WHERE table_schema = 'public' 
            AND table_name = 'lifeguard_migrations'
        )
    "#;
    
    let rows = executor.query_all(check_sql, &[]).expect("Failed to query");
    assert_eq!(rows.len(), 1);
    let exists: bool = rows[0].get(0);
    assert!(exists, "lifeguard_migrations table should exist");
    
    // Verify table structure
    let columns_sql = r#"
        SELECT column_name, data_type
        FROM information_schema.columns
        WHERE table_schema = 'public' 
        AND table_name = 'lifeguard_migrations'
        ORDER BY ordinal_position
    "#;
    
    let rows = executor.query_all(columns_sql, &[]).expect("Failed to query columns");
    assert!(rows.len() >= 6, "State table should have at least 6 columns");
    
    // Verify index exists
    let index_sql = r#"
        SELECT EXISTS (
            SELECT FROM pg_indexes
            WHERE schemaname = 'public'
            AND tablename = 'lifeguard_migrations'
            AND indexname = 'idx_lifeguard_migrations_applied_at'
        )
    "#;
    
    let rows = executor.query_all(index_sql, &[]).expect("Failed to query index");
    assert_eq!(rows.len(), 1);
    let exists: bool = rows[0].get(0);
    assert!(exists, "Index should exist");
}
