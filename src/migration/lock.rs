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
    
    // Set a per-query timeout to prevent hanging queries
    // Use a shorter timeout per attempt (5 seconds) to detect hanging queries quickly
    // The overall timeout is still enforced by the loop
    // Note: statement_timeout is session-level, so this affects all subsequent queries
    // We'll reset it after acquiring the lock
    let query_timeout_seconds = 5u64;
    let set_timeout_sql = format!("SET statement_timeout = '{}s'", query_timeout_seconds);
    
    // Set query timeout for this session
    // This ensures individual queries don't hang indefinitely
    // PostgreSQL will cancel queries that exceed this timeout
    let _ = executor.execute(&set_timeout_sql, &[]);
    
    loop {
        // CRITICAL: Check overall timeout BEFORE attempting query
        // This prevents infinite loops if queries hang indefinitely
        // Even with statement_timeout, network issues or database locks can cause hangs
        if start.elapsed() >= timeout {
            // Reset timeout before returning error
            let _ = executor.execute("RESET statement_timeout", &[]);
            return Err(MigrationError::LockTimeout(format!(
                "Failed to acquire migration lock within {} seconds. \
                 Another process may be running migrations. \
                 If this persists, check for stuck migration processes or manually delete \
                 the lock record: DELETE FROM lifeguard_migrations WHERE version = {}",
                timeout_seconds, LOCK_VERSION
            )));
        }
        
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
        
        // Execute with timeout protection
        // If this query hangs, PostgreSQL will cancel it after query_timeout_seconds
        // We also check timeout before each attempt to catch cases where query never returns
        let rows_affected = match executor.execute(&sql, &[]) {
            Ok(rows) => rows,
            Err(e) => {
                // Check if error is due to timeout
                let error_msg = format!("{}", e);
                if error_msg.contains("timeout") || error_msg.contains("canceling statement") {
                    // Query timed out - check overall timeout and retry if needed
                    if start.elapsed() >= timeout {
                        let _ = executor.execute("RESET statement_timeout", &[]);
                        return Err(MigrationError::LockTimeout(format!(
                            "Failed to acquire migration lock within {} seconds due to query timeout. \
                             Database may be slow or unresponsive. \
                             If this persists, check for stuck migration processes or manually delete \
                             the lock record: DELETE FROM lifeguard_migrations WHERE version = {}",
                            timeout_seconds, LOCK_VERSION
                        )));
                    }
                    // Query timed out but overall timeout not exceeded - retry
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                // Other database error - reset timeout and propagate it
                let _ = executor.execute("RESET statement_timeout", &[]);
                return Err(MigrationError::Database(e.into()));
            }
        };
        
        if rows_affected > 0 {
            // Lock acquired! We successfully inserted the lock record
            // Reset statement_timeout to default (unlimited) so it doesn't affect other operations
            let _ = executor.execute("RESET statement_timeout", &[]);
            return Ok(());
        }
        
        // Lock already held by another process
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

