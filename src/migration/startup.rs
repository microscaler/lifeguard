//! In-process migration execution helpers

use crate::LifeExecutor;
use crate::migration::{Migrator, MigrationError, lock::MigrationLockGuard};

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
/// * `executor` - The database executor (reference, no ownership needed!)
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
/// # Errors
///
/// Returns `MigrationError` if lock acquisition fails, checksum validation fails,
/// or migration execution fails.
///
/// # Behavior
///
/// - **First process wins:** The first process to insert lock record acquires the lock
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
///     // Run migrations on startup (executor is just a reference!)
///     startup_migrations(&executor, "./migrations", None)?;
///     
///     // Continue with application startup...
///     Ok(())
/// }
/// ```
pub fn startup_migrations(
    executor: &dyn LifeExecutor,
    migrations_dir: impl AsRef<std::path::Path>,
    timeout_seconds: Option<u64>,
) -> Result<(), MigrationError> {
    use crate::migration::SchemaManager;
    
    // Acquire migration lock (Flyway-style: uses migration table itself)
    // This prevents concurrent execution in multi-instance deployments (e.g., Kubernetes)
    let _lock = MigrationLockGuard::new(executor, timeout_seconds)?;
    
    // Create migrator
    let migrator = Migrator::new(migrations_dir);
    
    // Validate checksums of already-applied migrations
    // This ensures migration files haven't been modified after deployment
    migrator.validate_checksums(executor)?;
    
    // Create SchemaManager for migration execution
    // Note: We use up_with_lock() instead of up() since we already hold the lock
    // Calling up() would attempt to acquire the lock again, causing a deadlock
    let manager = SchemaManager::new(executor);
    
    // Apply pending migrations (executor is just a reference - no ownership needed!)
    // Use up_with_lock() since lock is already held by MigrationLockGuard
    let applied = migrator.up_with_lock(executor, &manager, None)?;
    
    if applied > 0 {
        log::info!("Applied {applied} migration(s) on startup");
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
///
/// # Errors
///
/// Returns `MigrationError` if lock acquisition fails, checksum validation fails,
/// or migration execution fails.
pub fn startup_migrations_with_timeout(
    executor: &dyn LifeExecutor,
    migrations_dir: impl AsRef<std::path::Path>,
    timeout_seconds: u64,
) -> Result<usize, MigrationError> {
    use crate::migration::SchemaManager;
    
    // Acquire migration lock (Flyway-style: uses migration table itself)
    let _lock = MigrationLockGuard::new(executor, Some(timeout_seconds))?;
    
    let migrator = Migrator::new(migrations_dir);
    migrator.validate_checksums(executor)?;
    
    // Create SchemaManager for migration execution
    // Note: We use up_with_lock() instead of up() since we already hold the lock
    // Calling up() would attempt to acquire the lock again, causing a deadlock
    let manager = SchemaManager::new(executor);
    
    // Apply pending migrations (executor is just a reference - no ownership needed!)
    // Use up_with_lock() since lock is already held by MigrationLockGuard
    let applied = migrator.up_with_lock(executor, &manager, None)?;
    
    if applied > 0 {
        log::info!("Applied {applied} migration(s) on startup");
    } else {
        log::debug!("No pending migrations to apply");
    }
    
    // Lock is automatically released when _lock is dropped
    Ok(applied)
}
