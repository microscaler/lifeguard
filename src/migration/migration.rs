//! Migration trait definition

use crate::LifeError;
use super::schema_manager::SchemaManager;

/// Trait that all migrations must implement
///
/// Each migration file should define a struct that implements this trait
/// with `up()` and `down()` methods for applying and rolling back the migration.
pub trait Migration: Send + Sync {
    /// Get the migration name (human-readable identifier)
    fn name(&self) -> &str;
    
    /// Get the migration version (timestamp: YYYYMMDDHHMMSS)
    fn version(&self) -> i64;
    
    /// Apply the migration (forward migration)
    ///
    /// This method should contain the logic to apply the migration,
    /// such as creating tables, adding columns, creating indexes, etc.
    ///
    /// Note: Lifeguard uses coroutines (may runtime), so this is synchronous,
    /// not async. The executor handles coroutine scheduling internally.
    fn up(&self, manager: &SchemaManager<'_>) -> Result<(), LifeError>;
    
    /// Rollback the migration (reverse migration)
    ///
    /// This method should contain the logic to undo the migration,
    /// such as dropping tables, removing columns, dropping indexes, etc.
    ///
    /// Note: This is optional - migrations without `down()` implementations
    /// cannot be rolled back.
    ///
    /// Note: Lifeguard uses coroutines (may runtime), so this is synchronous,
    /// not async. The executor handles coroutine scheduling internally.
    fn down(&self, manager: &SchemaManager<'_>) -> Result<(), LifeError>;
}
