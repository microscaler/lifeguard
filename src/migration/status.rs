//! Migration status tracking

use crate::migration::MigrationRecord;
use std::path::PathBuf;

/// Migration status information
#[derive(Debug, Clone)]
pub struct MigrationStatus {
    /// Applied migrations (from database)
    pub applied: Vec<MigrationRecord>,
    
    /// Pending migrations (from file system)
    pub pending: Vec<PendingMigration>,
    
    /// Total number of migrations (applied + pending)
    pub total: usize,
    
    /// Number of applied migrations
    pub applied_count: usize,
    
    /// Number of pending migrations
    pub pending_count: usize,
}

/// Represents a pending migration (not yet applied)
#[derive(Debug, Clone)]
pub struct PendingMigration {
    /// Migration version
    pub version: i64,
    
    /// Migration name
    pub name: String,
    
    /// File path
    pub path: PathBuf,
    
    /// Checksum
    pub checksum: String,
}

impl MigrationStatus {
    /// Create a new `MigrationStatus`
    #[must_use]
    pub fn new(
        applied: Vec<MigrationRecord>,
        pending: Vec<PendingMigration>,
    ) -> Self {
        let applied_count = applied.len();
        let pending_count = pending.len();
        let total = applied_count + pending_count;
        
        Self {
            applied,
            pending,
            total,
            applied_count,
            pending_count,
        }
    }
    
    /// Check if all migrations are applied
    #[must_use]
    pub fn is_up_to_date(&self) -> bool {
        self.pending_count == 0
    }
    
    /// Get the latest applied migration version
    #[must_use]
    pub fn latest_applied_version(&self) -> Option<i64> {
        self.applied.iter()
            .map(|m| m.version)
            .max()
    }
    
    /// Get the next pending migration version
    #[must_use]
    pub fn next_pending_version(&self) -> Option<i64> {
        self.pending.first()
            .map(|m| m.version)
    }
}
