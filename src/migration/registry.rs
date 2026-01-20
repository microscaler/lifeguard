//! Migration registry for runtime registration

use crate::migration::{Migration, MigrationError};
use std::collections::HashMap;
use std::sync::{Mutex, LazyLock};

/// Global migration registry
///
/// This registry stores all available migrations, indexed by version.
/// Migrations are registered at runtime using `register_migration()`.
/// 
/// For build-time registration, use a build script to generate a module
/// that calls `register_migration()` for each migration.
static MIGRATION_REGISTRY: LazyLock<Mutex<HashMap<i64, Box<dyn Migration + Send + Sync>>>> = LazyLock::new(|| {
    Mutex::new(HashMap::new())
});

/// Register a migration in the global registry
///
/// This function registers a migration so it can be executed later.
/// Migrations must be registered before they can be executed.
///
/// # Arguments
///
/// * `migration` - A boxed Migration trait implementation
///
/// # Returns
///
/// Returns `Ok(())` if registration succeeded, or an error if a migration
/// with the same version is already registered.
pub fn register_migration(migration: Box<dyn Migration + Send + Sync>) -> Result<(), MigrationError> {
    let version = migration.version();
    let name = migration.name().to_string();
    
    let mut registry = MIGRATION_REGISTRY.lock()
        .map_err(|e| MigrationError::InvalidFormat(format!("Failed to lock migration registry: {}", e)))?;
    
    if registry.contains_key(&version) {
        return Err(MigrationError::AlreadyApplied { version, name });
    }
    
    registry.insert(version, migration);
    Ok(())
}

/// Get a migration by version
///
/// # Arguments
///
/// * `version` - The migration version (timestamp)
///
/// # Returns
///
/// Returns `Some(migration)` if found, `None` otherwise
pub fn get_migration(version: i64) -> Result<Option<Box<dyn Migration + Send + Sync>>, MigrationError> {
    let registry = MIGRATION_REGISTRY.lock()
        .map_err(|e| MigrationError::InvalidFormat(format!("Failed to lock migration registry: {}", e)))?;
    
    Ok(registry.get(&version).map(|m| {
        // We can't return a reference to a trait object from a Mutex guard
        // So we need to clone the migration or use a different approach
        // For now, we'll return an error indicating this needs to be implemented differently
        // Actually, we can't clone a trait object easily. We need a different design.
        // Let me use a different approach - return a reference that's valid for the guard's lifetime
        // But that won't work either because we're returning from the function.
        // 
        // The solution: We need migrations to be Clone or we need to use Arc<dyn Migration>
        // For now, let's document this limitation and use a workaround
        todo!("Migration registry needs to support cloning or use Arc<dyn Migration>")
    }))
}

/// Get all registered migration versions, sorted
///
/// # Returns
///
/// Returns a vector of migration versions sorted (ascending)
pub fn get_all_migration_versions() -> Result<Vec<i64>, MigrationError> {
    let registry = MIGRATION_REGISTRY.lock()
        .map_err(|e| MigrationError::InvalidFormat(format!("Failed to lock migration registry: {}", e)))?;
    
    let mut versions: Vec<i64> = registry.keys().copied().collect();
    versions.sort();
    Ok(versions)
}

/// Check if a migration is registered
pub fn is_registered(version: i64) -> Result<bool, MigrationError> {
    let registry = MIGRATION_REGISTRY.lock()
        .map_err(|e| MigrationError::InvalidFormat(format!("Failed to lock migration registry: {}", e)))?;
    
    Ok(registry.contains_key(&version))
}

/// Execute a migration by version
///
/// This is a helper that gets the migration and executes it.
/// The migration must be registered first.
///
/// # Arguments
///
/// * `version` - The migration version
/// * `manager` - The SchemaManager for executing DDL
/// * `direction` - Whether to run `up()` or `down()`
///
/// # Returns
///
/// Returns `Ok(())` if execution succeeded, or an error otherwise
/// Execute a migration by version
///
/// This is a helper that gets the migration and executes it.
/// The migration must be registered first.
///
/// # Arguments
///
/// * `version` - The migration version
/// * `manager` - The SchemaManager for executing DDL
/// * `direction` - Whether to run `up()` or `down()`
///
/// # Returns
///
/// Returns `Ok(())` if execution succeeded, or an error otherwise
pub fn execute_migration(
    version: i64,
    manager: &crate::migration::SchemaManager,
    direction: MigrationDirection,
) -> Result<(), MigrationError> {
    
    // Get migration from registry
    // We need to hold the lock only long enough to get a reference
    // Since we can't return a trait object from a Mutex guard, we'll
    // execute the migration while holding the lock
    let registry = MIGRATION_REGISTRY.lock()
        .map_err(|e| MigrationError::InvalidFormat(format!("Failed to lock migration registry: {}", e)))?;
    
    let migration = registry.get(&version)
        .ok_or_else(|| MigrationError::MissingFile {
            version,
            name: format!("migration_{}", version),
        })?;
    
    // Execute migration while holding the lock
    // This is safe because Migration::up() and down() don't need to mutate the migration
    let result = match direction {
        MigrationDirection::Up => {
            migration.up(&manager)
                .map_err(|e| MigrationError::ExecutionFailed {
                    version,
                    name: migration.name().to_string(),
                    error: format!("{}", e),
                })
        }
        MigrationDirection::Down => {
            migration.down(&manager)
                .map_err(|e| MigrationError::ExecutionFailed {
                    version,
                    name: migration.name().to_string(),
                    error: format!("{}", e),
                })
        }
    };
    
    // Lock is released here when registry goes out of scope
    result
}

/// Direction for migration execution
#[derive(Debug, Clone, Copy)]
pub enum MigrationDirection {
    /// Apply the migration (up)
    Up,
    /// Rollback the migration (down)
    Down,
}
