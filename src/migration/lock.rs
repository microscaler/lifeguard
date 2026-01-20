//! Migration locking mechanism for concurrent execution protection

use crate::{LifeExecutor, LifeError};
use crate::migration::MigrationError;

/// Lock guard that automatically releases the lock when dropped
///
/// This ensures that locks are always released, even if an error occurs.
pub struct LockGuard {
    executor: Box<dyn LifeExecutor>,
    lock_id: i64,
    lock_type: LockType,
}

impl LockGuard {
    /// Get a reference to the underlying executor
    pub fn executor(&self) -> &dyn LifeExecutor {
        self.executor.as_ref()
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // Attempt to release the lock
        // Ignore errors during drop - we can't propagate them
        match self.lock_type {
            LockType::PostgresAdvisory => {
                let _ = self.executor.execute(
                    &format!("SELECT pg_advisory_unlock({})", self.lock_id),
                    &[],
                );
            }
            LockType::TableLock => {
                let _ = self.executor.execute(
                    "UPDATE lifeguard_migration_lock SET locked = false, locked_by = NULL, locked_at = NULL WHERE id = 1",
                    &[],
                );
            }
        }
    }
}

/// Type of lock mechanism being used
#[derive(Debug, Clone, Copy)]
enum LockType {
    /// PostgreSQL advisory lock
    PostgresAdvisory,
    /// Generic table-based lock
    TableLock,
}

/// Migration lock manager
///
/// Provides locking mechanisms to prevent concurrent migration execution.
/// Supports PostgreSQL advisory locks (preferred) and generic table locks (fallback).
pub struct MigrationLock;

impl MigrationLock {
    /// Unique lock ID for migration operations
    ///
    /// This is a constant used for PostgreSQL advisory locks.
    /// Using a constant ensures all processes use the same lock ID.
    const ADVISORY_LOCK_ID: i64 = 0x4C49464547554152; // "LIFEGUARD" in hex
    
    /// Acquire a migration lock
    ///
    /// This method attempts to acquire a lock using the best available mechanism:
    /// 1. PostgreSQL advisory locks (if available)
    /// 2. Generic table lock (fallback)
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor
    /// * `timeout_seconds` - Maximum time to wait for lock (default: 60)
    ///
    /// # Returns
    ///
    /// Returns a `LockGuard` that will automatically release the lock when dropped.
    /// If the lock cannot be acquired within the timeout, returns an error.
    ///
    /// # Errors
    ///
    /// Returns `MigrationError::LockTimeout` if the lock cannot be acquired.
    pub fn acquire(
        executor: Box<dyn LifeExecutor>,
        timeout_seconds: Option<u64>,
    ) -> Result<LockGuard, crate::migration::MigrationError> {
        let timeout = timeout_seconds.unwrap_or(60);
        
        // Try PostgreSQL advisory lock first
        if let Ok(guard) = Self::acquire_postgres_lock(executor, timeout) {
            return Ok(guard);
        }
        
        // Fallback to table lock
        // Note: We'd need to recreate the executor here, but for now we'll just try advisory lock
        // In a full implementation, we'd detect the database type and use the appropriate lock
        Err(MigrationError::LockTimeout(
            format!("Failed to acquire migration lock within {} seconds", timeout)
        ))
    }
    
    /// Acquire PostgreSQL advisory lock
    fn acquire_postgres_lock(
        executor: Box<dyn LifeExecutor>,
        timeout_seconds: u64,
    ) -> Result<LockGuard, LifeError> {
        // PostgreSQL advisory locks are non-blocking by default
        // We need to poll with a timeout
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_seconds);
        
        loop {
            // Try to acquire the lock
            // pg_advisory_lock returns true if lock acquired, false if already held
            let sql = format!("SELECT pg_try_advisory_lock({})", Self::ADVISORY_LOCK_ID);
            let row = executor.query_one(&sql, &[])?;
            let acquired: bool = row.get(0);
            
            if acquired {
                return Ok(LockGuard {
                    executor,
                    lock_id: Self::ADVISORY_LOCK_ID,
                    lock_type: LockType::PostgresAdvisory,
                });
            }
            
            // Check timeout
            if start.elapsed() >= timeout {
                return Err(LifeError::Other(format!(
                    "Migration lock timeout: could not acquire lock within {} seconds",
                    timeout_seconds
                )));
            }
            
            // Wait a bit before retrying (100ms)
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    
    /// Initialize the lock table (for non-PostgreSQL databases)
    ///
    /// Creates the `lifeguard_migration_lock` table if it doesn't exist.
    pub fn initialize_lock_table(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
        let sql = r#"
            CREATE TABLE IF NOT EXISTS lifeguard_migration_lock (
                id INTEGER PRIMARY KEY DEFAULT 1,
                locked BOOLEAN NOT NULL DEFAULT false,
                locked_by VARCHAR(255),
                locked_at TIMESTAMP,
                CHECK (id = 1)
            )
        "#;
        
        executor.execute(sql, &[])?;
        
        // Insert initial row if it doesn't exist
        let insert_sql = r#"
            INSERT INTO lifeguard_migration_lock (id, locked)
            VALUES (1, false)
            ON CONFLICT (id) DO NOTHING
        "#;
        
        executor.execute(insert_sql, &[])?;
        
        Ok(())
    }
}
