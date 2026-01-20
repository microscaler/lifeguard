//! Flyway-style migration table-based locking mechanism

use crate::LifeExecutor;
use crate::migration::MigrationError;
use std::time::{Instant, Duration};

/// Reserved version number for lock record
/// 
/// This value is never used for real migrations (which use positive timestamps).
/// The lock record uses version = -1 to identify it as a lock, not a migration.
const LOCK_VERSION: i64 = -1;

/// Lock guard that automatically releases the lock when dropped
///
/// This ensures that locks are always released, even if an error occurs.
/// Uses Flyway-style table-based locking (migration table itself as lock).
pub struct MigrationLockGuard<'a> {
    executor: &'a dyn LifeExecutor,
}

impl<'a> MigrationLockGuard<'a> {
    /// Acquire migration lock and create guard
    ///
    /// Uses Flyway-style locking: inserts a lock record (version = -1) into
    /// the migration table. The process that successfully inserts the record
    /// holds the lock.
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor (reference, no ownership needed!)
    /// * `timeout_seconds` - Maximum time to wait for lock (default: 60)
    ///
    /// # Returns
    ///
    /// Returns a guard that will automatically release the lock when dropped.
    /// If the lock cannot be acquired within the timeout, returns an error.
    ///
    /// # Errors
    ///
    /// Returns `MigrationError::LockTimeout` if the lock cannot be acquired.
    pub fn new(
        executor: &'a dyn LifeExecutor,
        timeout_seconds: Option<u64>,
    ) -> Result<Self, MigrationError> {
        let timeout = timeout_seconds.unwrap_or(60);
        acquire_migration_lock(executor, timeout)?;
        
        Ok(Self { executor })
    }
    
    /// Get a reference to the underlying executor
    pub fn executor(&self) -> &'a dyn LifeExecutor {
        self.executor
    }
}

impl<'a> Drop for MigrationLockGuard<'a> {
    fn drop(&mut self) {
        // Attempt to release the lock by deleting the lock record
        // Ignore errors during drop - we can't propagate them
        let _ = release_migration_lock(self.executor);
    }
}

/// Acquire migration lock by inserting lock record into migration table
///
/// Uses Flyway-style locking: the process that successfully inserts a lock record
/// (version = -1) into the migration table holds the lock.
///
/// # Arguments
///
/// * `executor` - The database executor (reference, no ownership needed!)
/// * `timeout_seconds` - Maximum time to wait for lock
///
/// # Returns
///
/// Returns `Ok(())` if lock acquired, or `MigrationError::LockTimeout` if timeout exceeded.
///
/// # How It Works
///
/// 1. Try to INSERT lock record (version = -1) into migration table
/// 2. If INSERT succeeds (rows_affected > 0): we hold the lock
/// 3. If INSERT fails (rows_affected = 0): another process has the lock
/// 4. Poll with timeout until lock acquired or timeout exceeded
pub fn acquire_migration_lock(
    executor: &dyn LifeExecutor,
    timeout_seconds: u64,
) -> Result<(), MigrationError> {
    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_seconds);
    
    loop {
        // Try to insert lock record
        // ON CONFLICT DO NOTHING ensures atomicity via PRIMARY KEY constraint
        let sql = format!(
            r#"
            INSERT INTO lifeguard_migrations (version, name, checksum, applied_at, success)
            VALUES ({}, 'LOCK', 'lock', NOW(), true)
            ON CONFLICT (version) DO NOTHING
            "#,
            LOCK_VERSION
        );
        
        let rows_affected = executor.execute(&sql, &[])
            .map_err(|e| MigrationError::Database(e.into()))?;
        
        if rows_affected > 0 {
            // Lock acquired! We successfully inserted the lock record
            return Ok(());
        }
        
        // Lock already held by another process
        // Check timeout
        if start.elapsed() >= timeout {
            return Err(MigrationError::LockTimeout(format!(
                "Failed to acquire migration lock within {} seconds. \
                 Another process may be running migrations. \
                 If this persists, check for stuck migration processes or manually delete \
                 the lock record: DELETE FROM lifeguard_migrations WHERE version = {}",
                timeout_seconds, LOCK_VERSION
            )));
        }
        
        // Wait before retrying (100ms)
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Release migration lock by deleting lock record
///
/// # Arguments
///
/// * `executor` - The database executor (reference, no ownership needed!)
///
/// # Returns
///
/// Returns `Ok(())` if lock released successfully, or an error if deletion fails.
pub fn release_migration_lock(
    executor: &dyn LifeExecutor,
) -> Result<(), MigrationError> {
    let sql = format!(
        "DELETE FROM lifeguard_migrations WHERE version = {}",
        LOCK_VERSION
    );
    
    executor.execute(&sql, &[])
        .map_err(|e| MigrationError::Database(e.into()))?;
    
    Ok(())
}

/// Check if migration lock is currently held
///
/// # Arguments
///
/// * `executor` - The database executor
///
/// # Returns
///
/// Returns `Ok(true)` if lock is held, `Ok(false)` if not.
pub fn is_migration_lock_held(
    executor: &dyn LifeExecutor,
) -> Result<bool, MigrationError> {
    let sql = format!(
        "SELECT COUNT(*) FROM lifeguard_migrations WHERE version = {}",
        LOCK_VERSION
    );
    
    let row = executor.query_one(&sql, &[])
        .map_err(|e| MigrationError::Database(e.into()))?;
    
    let count: i64 = row.get(0);
    Ok(count > 0)
}

