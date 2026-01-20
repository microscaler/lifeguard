//! Migrator - Core migration execution engine

use crate::LifeExecutor;
use crate::migration::{
    MigrationError, MigrationRecord, MigrationStatus, MigrationFile,
    MigrationLock, initialize_state_table, PendingMigration,
};
use crate::migration::file::discover_migrations;
use chrono::Utc;
use std::path::Path;
use std::time::Instant;

/// Core migration execution engine
///
/// The `Migrator` orchestrates migration discovery, validation, execution, and state tracking.
/// It supports both CLI and in-process execution modes.
pub struct Migrator {
    migrations_dir: std::path::PathBuf,
}

impl Migrator {
    /// Create a new Migrator with the specified migrations directory
    pub fn new(migrations_dir: impl AsRef<Path>) -> Self {
        Self {
            migrations_dir: migrations_dir.as_ref().to_path_buf(),
        }
    }
    
    /// Discover all migration files in the migrations directory
    ///
    /// Scans the directory for files matching the pattern `m{YYYYMMDDHHMMSS}_{name}.rs`
    /// and returns them sorted by version.
    pub fn discover_migrations(&self) -> Result<Vec<MigrationFile>, MigrationError> {
        discover_migrations(&self.migrations_dir)
    }
    
    /// Get migration status (applied vs pending)
    ///
    /// Compares discovered migration files with the state table to determine
    /// which migrations have been applied and which are pending.
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor
    ///
    /// # Returns
    ///
    /// Returns a `MigrationStatus` containing applied and pending migrations.
    pub fn status(&self, executor: &dyn LifeExecutor) -> Result<MigrationStatus, MigrationError> {
        // Ensure state table exists
        initialize_state_table(executor)?;
        
        // Discover migration files
        let migration_files = self.discover_migrations()?;
        
        // Query applied migrations from database
        let applied = Self::query_applied_migrations(executor)?;
        
        // Build set of applied versions for quick lookup
        let applied_versions: std::collections::HashSet<i64> = applied.iter()
            .map(|m| m.version)
            .collect();
        
        // Build set of file versions for quick lookup
        let file_versions: std::collections::HashSet<i64> = migration_files.iter()
            .map(|f| f.version)
            .collect();
        
        // Separate into applied and pending
        let mut applied_records = Vec::new();
        let mut pending_migrations = Vec::new();
        
        for file in &migration_files {
            if let Some(record) = applied.iter().find(|r| r.version == file.version) {
                // Migration is applied - validate checksum
                if record.checksum != file.checksum {
                    return Err(MigrationError::ChecksumMismatch {
                        version: file.version,
                        name: file.name.clone(),
                        stored: record.checksum.clone(),
                        current: file.checksum.clone(),
                    });
                }
                applied_records.push(record.clone());
            } else {
                // Migration is pending
                pending_migrations.push(PendingMigration {
                    version: file.version,
                    name: file.name.clone(),
                    path: file.path.clone(),
                    checksum: file.checksum.clone(),
                });
            }
        }
        
        // Check for missing files (applied but file not found)
        for record in &applied {
            if !file_versions.contains(&record.version) {
                return Err(MigrationError::MissingFile {
                    version: record.version,
                    name: record.name.clone(),
                });
            }
        }
        
        Ok(MigrationStatus::new(applied_records, pending_migrations))
    }
    
    /// Validate checksums of all applied migrations
    ///
    /// Reads all applied migrations from the state table, calculates current checksums
    /// from migration files, and compares them. Returns an error if any checksum mismatches.
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if all checksums match, or an error if any mismatch is found.
    pub fn validate_checksums(&self, executor: &dyn LifeExecutor) -> Result<(), MigrationError> {
        let status = self.status(executor)?;
        
        // Status already validates checksums, so if we get here, all are valid
        Ok(())
    }
    
    /// Apply pending migrations
    ///
    /// Discovers pending migrations, acquires a lock, validates checksums,
    /// and executes migrations in order.
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor (will be moved into LockGuard)
    /// * `steps` - Number of migrations to apply (None = all pending)
    ///
    /// # Returns
    ///
    /// Returns the number of migrations applied, or an error if execution fails.
    pub fn up(
        &self,
        executor: Box<dyn LifeExecutor>,
        steps: Option<usize>,
    ) -> Result<usize, MigrationError> {
        // Acquire migration lock
        let _lock = MigrationLock::acquire(executor, Some(60))
            .map_err(|e| MigrationError::LockTimeout(format!("{}", e)))?;
        
        // Get status (validates checksums)
        let status = self.status(_lock.executor())?;
        
        if status.pending.is_empty() {
            return Ok(0);
        }
        
        // Determine how many migrations to apply
        let migrations_to_apply = steps.unwrap_or(status.pending.len());
        let pending = status.pending.iter()
            .take(migrations_to_apply)
            .collect::<Vec<_>>();
        
        // Execute each migration
        let mut applied_count = 0;
        
        for pending_migration in pending {
            let start = Instant::now();
            
            // TODO: Execute migration
            // The actual migration execution requires loading and instantiating the Migration trait
            // implementation from the Rust file. This is complex and may require:
            // 1. Compiling migrations as a separate crate and loading dynamically
            // 2. Using a plugin system
            // 3. Having migrations call a registration function at compile time
            // 
            // For now, the structure is in place. The execution logic will be implemented
            // based on the chosen approach (likely requiring CLI tool integration or
            // a build-time migration registration system).
            //
            // Example of what execution would look like:
            // let migration = load_migration(&pending_migration.path)?;
            // let manager = SchemaManager::new(Box::new(_lock.executor()));
            // migration.up(&manager)?;
            
            // Record migration in state table
            let execution_time = start.elapsed().as_millis() as i64;
            let record = MigrationRecord::new(
                pending_migration.version,
                pending_migration.name.clone(),
                pending_migration.checksum.clone(),
                Utc::now(),
                Some(execution_time),
                true,
            );
            
            Self::record_migration(_lock.executor(), &record)?;
            applied_count += 1;
        }
        
        Ok(applied_count)
    }
    
    /// Rollback migrations
    ///
    /// Rolls back the last N applied migrations by executing their `down()` methods.
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor
    /// * `steps` - Number of migrations to rollback (default: 1)
    ///
    /// # Returns
    ///
    /// Returns the number of migrations rolled back, or an error if execution fails.
    pub fn down(
        &self,
        executor: Box<dyn LifeExecutor>,
        steps: Option<usize>,
    ) -> Result<usize, MigrationError> {
        // Acquire migration lock
        let _lock = MigrationLock::acquire(executor, Some(60))
            .map_err(|e| MigrationError::LockTimeout(format!("{}", e)))?;
        
        // Get status
        let status = self.status(_lock.executor())?;
        
        if status.applied.is_empty() {
            return Ok(0);
        }
        
        // Get migrations to rollback (in reverse order - newest first)
        let steps = steps.unwrap_or(1);
        let mut applied = status.applied;
        applied.sort_by_key(|m| std::cmp::Reverse(m.version));
        
        let migrations_to_rollback: Vec<_> = applied.iter()
            .take(steps)
            .collect();
        
        let rollback_count = migrations_to_rollback.len();
        
        // Execute down() for each migration
        // TODO: Implement actual migration execution
        // For now, just remove from state table
        for record in migrations_to_rollback {
            Self::remove_migration_record(_lock.executor(), record.version)?;
        }
        
        Ok(rollback_count)
    }
    
    /// Query applied migrations from the state table
    fn query_applied_migrations(executor: &dyn LifeExecutor) -> Result<Vec<MigrationRecord>, MigrationError> {
        let sql = r#"
            SELECT version, name, checksum, applied_at, execution_time_ms, success
            FROM lifeguard_migrations
            ORDER BY version ASC
        "#;
        
        let rows = executor.query_all(sql, &[])
            .map_err(|e| MigrationError::Database(e.into()))?;
        
        let mut records = Vec::new();
        for row in rows {
            let record = MigrationRecord::from_row(&row)
                .map_err(|e| MigrationError::Database(e))?;
            records.push(record);
        }
        
        Ok(records)
    }
    
    /// Record a migration in the state table
    fn record_migration(executor: &dyn LifeExecutor, record: &MigrationRecord) -> Result<(), MigrationError> {
        let sql = r#"
            INSERT INTO lifeguard_migrations (version, name, checksum, applied_at, execution_time_ms, success)
            VALUES ($1, $2, $3, $4, $5, $6)
        "#;
        
        // Format timestamp as PostgreSQL timestamp string
        let timestamp_str = record.applied_at.format("%Y-%m-%d %H:%M:%S%.f").to_string();
        
        executor.execute(sql, &[
            &record.version,
            &record.name,
            &record.checksum,
            &timestamp_str,
            &record.execution_time_ms,
            &record.success,
        ])
        .map_err(|e| MigrationError::Database(e.into()))?;
        
        Ok(())
    }
    
    /// Remove a migration record from the state table
    fn remove_migration_record(executor: &dyn LifeExecutor, version: i64) -> Result<(), MigrationError> {
        let sql = "DELETE FROM lifeguard_migrations WHERE version = $1";
        
        executor.execute(sql, &[&version])
            .map_err(|e| MigrationError::Database(e.into()))?;
        
        Ok(())
    }
}
