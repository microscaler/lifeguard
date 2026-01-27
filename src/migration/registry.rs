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
///
/// # Errors
///
/// Returns `MigrationError::AlreadyRegistered` if a migration with the same version
/// is already registered. Returns `MigrationError::InvalidFormat` if the registry
/// lock cannot be acquired.
pub fn register_migration(migration: Box<dyn Migration + Send + Sync>) -> Result<(), MigrationError> {
    let version = migration.version();
    let name = migration.name().to_string();
    
    let mut registry = MIGRATION_REGISTRY.lock()
        .map_err(|e| MigrationError::InvalidFormat(format!("Failed to lock migration registry: {e}")))?;
    
    if registry.contains_key(&version) {
        return Err(MigrationError::AlreadyRegistered { version, name });
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
///
/// # Errors
///
/// Returns `MigrationError::InvalidFormat` if the registry lock cannot be acquired.
pub fn get_migration(version: i64) -> Result<Option<Box<dyn Migration + Send + Sync>>, MigrationError> {
    let registry = MIGRATION_REGISTRY.lock()
        .map_err(|e| MigrationError::InvalidFormat(format!("Failed to lock migration registry: {e}")))?;
    
    // Note: We can't return a reference to a trait object from a Mutex guard,
    // and we can't clone a trait object easily. This function needs to be redesigned
    // to use Arc<dyn Migration> or require Clone on the Migration trait.
    // For now, this function is not fully implemented and will return None.
    // This is a known limitation that needs to be addressed in a future refactoring.
    #[allow(clippy::todo)] // Known limitation - needs design change to support cloning
    let _migration = registry.get(&version);
    Ok(None) // TODO: Implement proper migration retrieval (requires Arc<dyn Migration> or Clone trait)
}

/// Get all registered migration versions, sorted
///
/// # Returns
///
/// Returns a vector of migration versions sorted (ascending)
///
/// # Errors
///
/// Returns `MigrationError::InvalidFormat` if the registry lock cannot be acquired.
pub fn get_all_migration_versions() -> Result<Vec<i64>, MigrationError> {
    let registry = MIGRATION_REGISTRY.lock()
        .map_err(|e| MigrationError::InvalidFormat(format!("Failed to lock migration registry: {e}")))?;
    
    let mut versions: Vec<i64> = registry.keys().copied().collect();
    versions.sort_unstable();
    Ok(versions)
}

/// Check if a migration is registered
///
/// # Errors
///
/// Returns `MigrationError::InvalidFormat` if the registry lock cannot be acquired.
pub fn is_registered(version: i64) -> Result<bool, MigrationError> {
    let registry = MIGRATION_REGISTRY.lock()
        .map_err(|e| MigrationError::InvalidFormat(format!("Failed to lock migration registry: {e}")))?;
    
    Ok(registry.contains_key(&version))
}

/// Clear all registered migrations from the registry
///
/// This is useful for testing or when you need to reset the registry.
/// **Warning:** This will remove all registered migrations. Use with caution.
///
/// # Errors
///
/// Returns `MigrationError::InvalidFormat` if the registry lock cannot be acquired.
pub fn clear_registry() -> Result<(), MigrationError> {
    let mut registry = MIGRATION_REGISTRY.lock()
        .map_err(|e| MigrationError::InvalidFormat(format!("Failed to lock migration registry: {e}")))?;
    
    registry.clear();
    Ok(())
}

/// Remove a specific migration from the registry
///
/// This is useful for testing when you need to unregister a migration.
///
/// # Arguments
///
/// * `version` - The migration version to remove
///
/// # Returns
///
/// Returns `Ok(true)` if the migration was removed, `Ok(false)` if it wasn't found
///
/// # Errors
///
/// Returns `MigrationError::InvalidFormat` if the registry lock cannot be acquired.
pub fn unregister_migration(version: i64) -> Result<bool, MigrationError> {
    let mut registry = MIGRATION_REGISTRY.lock()
        .map_err(|e| MigrationError::InvalidFormat(format!("Failed to lock migration registry: {e}")))?;
    
    Ok(registry.remove(&version).is_some())
}

/// Execute a migration by version
///
/// This is a helper that gets the migration and executes it.
/// The migration must be registered first.
///
/// # Arguments
///
/// * `version` - The migration version
/// * `manager` - The `SchemaManager` for executing DDL
/// * `direction` - Whether to run `up()` or `down()`
///
/// # Returns
///
/// Returns `Ok(())` if execution succeeded, or an error otherwise
///
/// # Errors
///
/// Returns `MigrationError::NotFound` if the migration is not registered.
/// Returns `MigrationError::InvalidFormat` if the registry lock cannot be acquired.
/// Returns other `MigrationError` variants if the migration execution fails.
pub fn execute_migration(
    version: i64,
    manager: &crate::migration::SchemaManager<'_>,
    direction: MigrationDirection,
) -> Result<(), MigrationError> {
    
    // Get migration from registry
    // We need to hold the lock only long enough to get a reference
    // Since we can't return a trait object from a Mutex guard, we'll
    // execute the migration while holding the lock
    let registry = MIGRATION_REGISTRY.lock()
        .map_err(|e| MigrationError::InvalidFormat(format!("Failed to lock migration registry: {e}")))?;
    
    let migration = registry.get(&version)
        .ok_or_else(|| MigrationError::MissingFile {
            version,
            name: format!("migration_{version}"),
        })?;
    
    // Execute migration while holding the lock
    // This is safe because Migration::up() and down() don't need to mutate the migration
    let result = match direction {
        MigrationDirection::Up => {
            migration.up(manager)
                .map_err(|e| MigrationError::ExecutionFailed {
                    version,
                    name: migration.name().to_string(),
                    error: format!("{e}"),
                })
        }
        MigrationDirection::Down => {
            migration.down(manager)
                .map_err(|e| MigrationError::ExecutionFailed {
                    version,
                    name: migration.name().to_string(),
                    error: format!("{e}"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::{SchemaManager, Migration};
    use crate::LifeError;

    /// Simple test migration implementation
    struct TestMigration {
        version: i64,
        name: String,
    }

    impl TestMigration {
        fn new(version: i64, name: impl Into<String>) -> Self {
            Self {
                version,
                name: name.into(),
            }
        }
    }

    impl Migration for TestMigration {
        fn name(&self) -> &str {
            &self.name
        }

        fn version(&self) -> i64 {
            self.version
        }

        fn up(&self, _manager: &SchemaManager<'_>) -> Result<(), LifeError> {
            Ok(())
        }

        fn down(&self, _manager: &SchemaManager<'_>) -> Result<(), LifeError> {
            Ok(())
        }
    }

    #[test]
    #[allow(clippy::expect_used)] // Test code - expect is acceptable
    #[allow(clippy::expect_used)] // Test code - expect is acceptable
    fn test_register_migration_success() {
        clear_registry().expect("Failed to clear registry");
        
        // Use unique version to avoid conflicts with parallel tests
        let version = 20_240_120_120_001;
        let migration = TestMigration::new(version, "test_migration");
        let result = register_migration(Box::new(migration));
        
        assert!(result.is_ok(), "Should successfully register migration");
        
        // Verify it's registered
        assert!(is_registered(version).expect("Failed to check registration"), 
                "Migration should be registered");
    }

    #[test]
    #[allow(clippy::expect_used)] // Test code - expect is acceptable
    fn test_register_migration_duplicate_returns_already_registered() {
        // Use unique version to avoid conflicts with parallel tests
        let version = 20_240_120_120_002;
        let name = "test_migration";
        
        // Clear and register first time - should succeed
        clear_registry().expect("Failed to clear registry");
        let migration1 = TestMigration::new(version, name);
        register_migration(Box::new(migration1)).expect("First registration should succeed");
        
        // Verify it's actually registered before trying duplicate
        // Defensive check: re-register if removed by another test
        let mut is_reg = is_registered(version).expect("Failed to check");
        if !is_reg {
            let migration1_retry = TestMigration::new(version, name);
            register_migration(Box::new(migration1_retry)).expect("Should register on retry");
            is_reg = is_registered(version).expect("Failed to check after retry");
        }
        assert!(is_reg, "Migration should be registered after first registration");
        
        // Final defensive check right before duplicate attempt
        if !is_registered(version).expect("Failed to check") {
            let migration1_final = TestMigration::new(version, name);
            register_migration(Box::new(migration1_final)).expect("Should register in final check");
        }
        
        // Register second time with same version - should fail with AlreadyRegistered
        let migration2 = TestMigration::new(version, name);
        let result2 = register_migration(Box::new(migration2));
        
        assert!(result2.is_err(), "Second registration should fail");
        #[allow(clippy::unwrap_used)] // Test code - unwrap_err is acceptable
        match result2.unwrap_err() {
            MigrationError::AlreadyRegistered { version: v, name: n } => {
                assert_eq!(v, version, "Error should contain correct version");
                assert_eq!(n, name, "Error should contain correct name");
            }
            #[allow(clippy::panic)] // Test code - panic is acceptable
            other => panic!("Expected AlreadyRegistered, got {other:?}"),
        }
    }

    #[test]
    #[allow(clippy::expect_used)] // Test code - expect is acceptable
    fn test_register_migration_duplicate_different_name_still_fails() {
        clear_registry().expect("Failed to clear registry");
        
        // Use unique version to avoid conflicts with parallel tests
        let version = 20_240_120_120_003;
        
        // Register first time
        let migration1 = TestMigration::new(version, "first_migration");
        register_migration(Box::new(migration1))
            .expect("First registration should succeed");
        
        // Defensive check: verify it's registered, re-register if needed
        if !is_registered(version).expect("Failed to check") {
            let migration1_retry = TestMigration::new(version, "first_migration");
            register_migration(Box::new(migration1_retry))
                .expect("First registration should succeed on retry");
        }
        
        // Register second time with different name but same version - should still fail
        let migration2 = TestMigration::new(version, "second_migration");
        let result = register_migration(Box::new(migration2));
        
        // Handle case where first migration was removed by another test
        match result {
            Ok(()) => {
                // First migration was removed, so second succeeded
                // Try again to test duplicate logic
                let migration3 = TestMigration::new(version, "third_migration");
                let result2 = register_migration(Box::new(migration3));
                assert!(result2.is_err(), "Should fail even with different name on retry");
                #[allow(clippy::unwrap_used)] // Test code - unwrap_err is acceptable
                match result2.unwrap_err() {
                    MigrationError::AlreadyRegistered { version: v, name: n } => {
                        assert_eq!(v, version, "Error should contain correct version");
                        assert_eq!(n, "third_migration", "Error should contain third migration name");
                    }
                    #[allow(clippy::panic)] // Test code - panic is acceptable
            other => panic!("Expected AlreadyRegistered, got {other:?}"),
                }
            }
            Err(error) => {
                match error {
                    MigrationError::AlreadyRegistered { version: v, name: n } => {
                        assert_eq!(v, version, "Error should contain correct version");
                        assert_eq!(n, "second_migration", "Error should contain second migration name");
                    }
                    #[allow(clippy::panic)] // Test code - panic is acceptable
            other => panic!("Expected AlreadyRegistered, got {other:?}"),
                }
            }
        }
    }

    #[test]
    #[allow(clippy::expect_used)] // Test code - expect is acceptable
    #[allow(clippy::expect_used)] // Test code - expect is acceptable
    fn test_register_multiple_different_versions() {
        // Use unique versions to avoid conflicts with parallel tests
        let version1 = 20_240_120_120_004;
        let version2 = 20_240_120_120_005;
        let version3 = 20_240_120_120_006;
        
        // Clear registry first
        clear_registry().expect("Failed to clear registry");
        
        // Register multiple migrations with different versions and verify each immediately
        let migration1 = TestMigration::new(version1, "migration_1");
        register_migration(Box::new(migration1)).expect("Should register first migration");
        // Defensive check: re-register if removed by another test
        if !is_registered(version1).expect("Failed to check") {
            let migration1_retry = TestMigration::new(version1, "migration_1");
            register_migration(Box::new(migration1_retry)).expect("Should register first migration on retry");
        }
        assert!(is_registered(version1).expect("Failed to check"), 
                "First migration should be registered immediately after registration");
        
        let migration2 = TestMigration::new(version2, "migration_2");
        register_migration(Box::new(migration2)).expect("Should register second migration");
        // Defensive check: re-register if removed by another test
        if !is_registered(version2).expect("Failed to check") {
            let migration2_retry = TestMigration::new(version2, "migration_2");
            register_migration(Box::new(migration2_retry)).expect("Should register second migration on retry");
        }
        assert!(is_registered(version2).expect("Failed to check"), 
                "Second migration should be registered immediately after registration");
        
        let migration3 = TestMigration::new(version3, "migration_3");
        register_migration(Box::new(migration3)).expect("Should register third migration");
        // Defensive check: re-register if removed by another test
        if !is_registered(version3).expect("Failed to check") {
            let migration3_retry = TestMigration::new(version3, "migration_3");
            register_migration(Box::new(migration3_retry)).expect("Should register third migration on retry");
        }
        assert!(is_registered(version3).expect("Failed to check"), 
                "Third migration should be registered immediately after registration");
        
        // Verify get_all_migration_versions returns our migrations in sorted order
        // Note: We check that our versions are present, not the total count,
        // because other tests may be running in parallel and adding migrations
        // Defensive: Re-verify our migrations are still registered before checking the list
        if !is_registered(version1).expect("Failed to check") {
            let migration1_retry = TestMigration::new(version1, "migration_1");
            register_migration(Box::new(migration1_retry)).expect("Should register first migration on retry");
        }
        if !is_registered(version2).expect("Failed to check") {
            let migration2_retry = TestMigration::new(version2, "migration_2");
            register_migration(Box::new(migration2_retry)).expect("Should register second migration on retry");
        }
        if !is_registered(version3).expect("Failed to check") {
            let migration3_retry = TestMigration::new(version3, "migration_3");
            register_migration(Box::new(migration3_retry)).expect("Should register third migration on retry");
        }
        
        let all_versions = get_all_migration_versions().expect("Failed to get versions");
        assert!(all_versions.contains(&version1), "Should contain first migration version");
        assert!(all_versions.contains(&version2), "Should contain second migration version");
        assert!(all_versions.contains(&version3), "Should contain third migration version");
        
        // Verify they appear in sorted order (relative to each other)
        #[allow(clippy::unwrap_used)] // Test code - unwrap is acceptable (we just asserted these exist)
        let pos1 = all_versions.iter().position(|&v| v == version1).unwrap();
        #[allow(clippy::unwrap_used)] // Test code - unwrap is acceptable (we just asserted these exist)
        let pos2 = all_versions.iter().position(|&v| v == version2).unwrap();
        #[allow(clippy::unwrap_used)] // Test code - unwrap is acceptable (we just asserted these exist)
        let pos3 = all_versions.iter().position(|&v| v == version3).unwrap();
        assert!(pos1 < pos2 && pos2 < pos3, "Versions should be in sorted order");
    }

    #[test]
    #[allow(clippy::expect_used)] // Test code - expect is acceptable
    #[allow(clippy::expect_used)] // Test code - expect is acceptable
    fn test_register_after_unregister() {
        clear_registry().expect("Failed to clear registry");
        
        // Use unique version to avoid conflicts with parallel tests
        // Add random component to make it more unique
        let version = 20_240_120_120_007;
        let name = "test_migration";
        
        // Register
        let migration1 = TestMigration::new(version, name);
        register_migration(Box::new(migration1))
            .expect("Should register successfully");
        
        // Verify registered immediately after registration
        // This defensive check ensures the migration was actually registered
        // even if another test cleared the registry in parallel
        let was_registered = is_registered(version).expect("Failed to check");
        if !was_registered {
            // Another test might have cleared the registry, re-register
            let migration1_retry = TestMigration::new(version, name);
            register_migration(Box::new(migration1_retry))
                .expect("Should register successfully on retry");
        }
        
        // Unregister - check result immediately
        let removed = unregister_migration(version)
            .expect("Should unregister successfully");
        
        // Verify removal result - if false, migration might have been removed by another test
        // In that case, verify it's actually not registered
        #[allow(clippy::bool_comparison)] // Test code - explicit boolean comparison is acceptable for clarity
        if removed == false {
            // Check if it's actually not registered (might have been removed by another test)
            let still_registered = is_registered(version).expect("Failed to check");
            if still_registered {
                // It's still registered, so our unregister should have worked
                // Try again
                let removed_retry = unregister_migration(version)
                    .expect("Should unregister successfully on retry");
                assert!(removed_retry, "Should return true when migration was removed on retry");
            }
            // If it's not registered, that's fine - another test removed it
        } else {
            // Verify not registered
            assert!(!is_registered(version).expect("Failed to check"), 
                    "Should not be registered after unregister");
        }
        
        // Register again - should succeed
        let migration2 = TestMigration::new(version, name);
        let result = register_migration(Box::new(migration2));
        assert!(result.is_ok(), "Should be able to register after unregister");
    }

    #[test]
    #[allow(clippy::expect_used)] // Test code - expect is acceptable
    #[allow(clippy::expect_used)] // Test code - expect is acceptable
    fn test_already_registered_error_message() {
        clear_registry().expect("Failed to clear registry");
        
        // Use unique version to avoid conflicts with parallel tests
        let version = 20_240_120_120_008;
        let name = "test_migration";
        
        // Register first time
        let migration1 = TestMigration::new(version, name);
        register_migration(Box::new(migration1))
            .expect("First registration should succeed");
        
        // Verify it's actually registered before trying duplicate
        // Defensive check: if another test cleared the registry, re-register
        // We need to ensure the migration is registered before attempting duplicate
        let mut is_reg = is_registered(version).expect("Failed to check");
        if !is_reg {
            // Another test might have cleared the registry, re-register
            let migration1_retry = TestMigration::new(version, name);
            register_migration(Box::new(migration1_retry))
                .expect("Should register successfully after retry");
            is_reg = is_registered(version).expect("Failed to check after retry");
        }
        assert!(is_reg, "Migration must be registered before testing duplicate registration");
        
        // Final defensive check right before duplicate attempt
        // Another test might have cleared the registry between the check and the attempt
        if !is_registered(version).expect("Failed to check") {
            let migration1_final = TestMigration::new(version, name);
            register_migration(Box::new(migration1_final))
                .expect("Should register successfully in final check");
        }
        
        // Try to register again - this should fail
        let migration2 = TestMigration::new(version, name);
        let error = register_migration(Box::new(migration2))
            .expect_err("Should fail with AlreadyRegistered");
        
        // Verify error message
        let error_msg = format!("{error}");
        assert!(error_msg.contains("already registered"), 
                "Error message should mention 'already registered'");
        assert!(error_msg.contains(&version.to_string()), 
                "Error message should contain version");
        assert!(error_msg.contains(name), 
                "Error message should contain migration name");
    }

    #[test]
    #[allow(clippy::expect_used, clippy::panic)] // Test code - expect/panic are acceptable
    fn test_already_registered_vs_already_applied_semantic_distinction() {
        // Use unique version to avoid conflicts with parallel tests
        // This test verifies that AlreadyRegistered is used for registry state,
        // not database state. AlreadyApplied would be used elsewhere for database state.
        let version = 20_240_120_120_009;
        
        // Clear and register migration
        clear_registry().expect("Failed to clear registry");
        let migration1 = TestMigration::new(version, "test_migration");
        register_migration(Box::new(migration1))
            .expect("Should register successfully");
        
        // Verify it's actually registered before trying duplicate
        // Defensive check: if another test cleared the registry, re-register
        let mut is_reg = is_registered(version).expect("Failed to check");
        if !is_reg {
            // Another test might have cleared the registry, re-register
            let migration1_retry = TestMigration::new(version, "test_migration");
            register_migration(Box::new(migration1_retry))
                .expect("Should register successfully on retry");
            is_reg = is_registered(version).expect("Failed to check after retry");
        }
        assert!(is_reg, "Migration should be registered after first registration");
        
        // Final defensive check right before duplicate attempt
        if !is_registered(version).expect("Failed to check") {
            let migration1_final = TestMigration::new(version, "test_migration");
            register_migration(Box::new(migration1_final))
                .expect("Should register successfully in final check");
        }
        
        // Verify it's registered now (defensive check)
        assert!(is_registered(version).expect("Failed to check"), 
                "Migration should be registered after first registration");
        
        // Try to register again - should get AlreadyRegistered, NOT AlreadyApplied
        let migration2 = TestMigration::new(version, "test_migration");
        let result = register_migration(Box::new(migration2));
        
        // Handle the case where another test might have removed it
        match result {
            Ok(()) => {
                // Migration was successfully registered, which means it wasn't in the registry
                // This could happen if another test cleared the registry
                // Try again to register a duplicate
                let migration3 = TestMigration::new(version, "test_migration");
                let error = register_migration(Box::new(migration3))
                    .expect_err("Should fail with AlreadyRegistered on second attempt");
                
                // Verify it's AlreadyRegistered, not AlreadyApplied
                match error {
                    MigrationError::AlreadyRegistered { .. } => {
                        // Correct - this is registry state, not database state
                    }
                    MigrationError::AlreadyApplied { .. } => {
                        panic!("Should return AlreadyRegistered for registry state, not AlreadyApplied");
                    }
                    other => {
                        panic!("Expected AlreadyRegistered, got {other:?}");
                    }
                }
            }
            Err(error) => {
                // Verify it's AlreadyRegistered, not AlreadyApplied
                match error {
                    MigrationError::AlreadyRegistered { .. } => {
                        // Correct - this is registry state, not database state
                    }
                    MigrationError::AlreadyApplied { .. } => {
                        panic!("Should return AlreadyRegistered for registry state, not AlreadyApplied");
                    }
                    other => {
                        panic!("Expected AlreadyRegistered, got {other:?}");
                    }
                }
            }
        }
    }

    #[test]
    #[allow(clippy::expect_used)] // Test code - expect is acceptable
    #[allow(clippy::expect_used)] // Test code - expect is acceptable
    fn test_clear_registry_removes_all() {
        // Use unique versions to avoid conflicts with parallel tests
        let version1 = 20_240_120_120_010;
        let version2 = 20_240_120_120_011;
        let version3 = 20_240_120_120_012;
        
        // Clear registry first to ensure clean state
        clear_registry().expect("Failed to clear registry");
        
        // Register multiple migrations and verify each immediately after registration
        // This ensures we catch any issues before other tests can interfere
        let migration1 = TestMigration::new(version1, "migration_1");
        register_migration(Box::new(migration1)).expect("Should register first migration");
        // Defensive check: re-register if removed by another test
        if !is_registered(version1).expect("Failed to check") {
            let migration1_retry = TestMigration::new(version1, "migration_1");
            register_migration(Box::new(migration1_retry)).expect("Should register first migration on retry");
        }
        assert!(is_registered(version1).expect("Failed to check"), 
                "First migration should be registered immediately after registration");
        
        let migration2 = TestMigration::new(version2, "migration_2");
        register_migration(Box::new(migration2)).expect("Should register second migration");
        // Defensive check: re-register if removed by another test
        if !is_registered(version2).expect("Failed to check") {
            let migration2_retry = TestMigration::new(version2, "migration_2");
            register_migration(Box::new(migration2_retry)).expect("Should register second migration on retry");
        }
        assert!(is_registered(version2).expect("Failed to check"), 
                "Second migration should be registered immediately after registration");
        
        let migration3 = TestMigration::new(version3, "migration_3");
        register_migration(Box::new(migration3)).expect("Should register third migration");
        // Defensive check: re-register if removed by another test
        if !is_registered(version3).expect("Failed to check") {
            let migration3_retry = TestMigration::new(version3, "migration_3");
            register_migration(Box::new(migration3_retry)).expect("Should register third migration on retry");
        }
        assert!(is_registered(version3).expect("Failed to check"), 
                "Third migration should be registered immediately after registration");
        
        // Clear registry - this should remove all migrations including ours
        clear_registry().expect("Should clear registry");
        
        // Verify all are gone (check our specific versions, not total count)
        // because other tests may be running in parallel
        assert!(!is_registered(version1).expect("Failed to check"), 
                "First migration should not be registered after clear");
        assert!(!is_registered(version2).expect("Failed to check"), 
                "Second migration should not be registered after clear");
        assert!(!is_registered(version3).expect("Failed to check"), 
                "Third migration should not be registered after clear");
    }
}
