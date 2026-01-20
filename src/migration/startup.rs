//! In-process migration execution helpers

use crate::{LifeExecutor, LifeError};
use crate::migration::{Migrator, MigrationError, MigrationLock, LockGuard};

/// Run migrations on application startup
///
/// This function is designed to be called during application initialization
/// to automatically apply pending migrations. It handles:
/// - Lock acquisition (prevents concurrent execution in multi-instance deployments)
/// - Checksum validation (ensures migration files haven't been modified)
/// - Migration execution (applies pending migrations)
/// - Error handling (fails fast if migrations cannot be applied)
///
/// # Arguments
///
/// * `executor` - The database executor (will be moved into LockGuard)
/// * `migrations_dir` - Path to the migrations directory
/// * `timeout_seconds` - Maximum time to wait for lock acquisition (default: 60)
///
/// # Returns
///
/// Returns `Ok(())` if migrations were applied successfully, or an error if:
/// - Lock cannot be acquired within timeout
/// - Checksum validation fails
/// - Migration execution fails
///
/// # Behavior
///
/// - **First process wins:** The first process to start acquires the lock and runs migrations
/// - **Other processes wait:** Other processes wait for the lock to be released (with timeout)
/// - **Fail-fast:** If migrations fail, the application should not start
///
/// # Example
///
/// ```rust,no_run
/// use lifeguard::{connect, MayPostgresExecutor, migration::startup_migrations};
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")?;
///     let executor = MayPostgresExecutor::new(client);
///     
///     // Run migrations on startup
///     startup_migrations(Box::new(executor), "./migrations", None)?;
///     
///     // Continue with application startup...
///     Ok(())
/// }
/// ```
pub fn startup_migrations(
    executor: Box<dyn LifeExecutor>,
    migrations_dir: impl AsRef<std::path::Path>,
    timeout_seconds: Option<u64>,
) -> Result<(), MigrationError> {
    // Acquire migration lock
    // This prevents concurrent execution in multi-instance deployments (e.g., Kubernetes)
    let _lock = MigrationLock::acquire(executor, timeout_seconds)
        .map_err(|e| MigrationError::LockTimeout(format!("{}", e)))?;
    
    // Create migrator
    let migrator = Migrator::new(migrations_dir);
    
    // Validate checksums of already-applied migrations
    // This ensures migration files haven't been modified after deployment
    migrator.validate_checksums(_lock.executor())?;
    
    // Apply pending migrations (lock is already held)
    let applied = migrator.up_with_lock(_lock.executor(), None)?;
    
    if applied > 0 {
        log::info!("Applied {} migration(s) on startup", applied);
    } else {
        log::debug!("No pending migrations to apply");
    }
    
    // Lock is automatically released when _lock is dropped
    Ok(())
}

/// Run migrations with custom timeout and error handling
///
/// Similar to `startup_migrations()`, but allows custom timeout and
/// returns more detailed error information.
pub fn startup_migrations_with_timeout(
    executor: Box<dyn LifeExecutor>,
    migrations_dir: impl AsRef<std::path::Path>,
    timeout_seconds: u64,
) -> Result<usize, MigrationError> {
    let _lock = MigrationLock::acquire(executor, Some(timeout_seconds))
        .map_err(|e| MigrationError::LockTimeout(format!("{}", e)))?;
    
    let migrator = Migrator::new(migrations_dir);
    migrator.validate_checksums(_lock.executor())?;
    
    // Apply pending migrations (lock is already held)
    let applied = migrator.up_with_lock(_lock.executor(), None)?;
    
    Ok(applied)
}
