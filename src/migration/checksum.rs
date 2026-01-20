//! Checksum calculation for migration files

use crate::LifeError;
use sha2::{Sha256, Digest};
use std::fs;
use std::path::Path;

/// Calculate SHA-256 checksum of a migration file
///
/// This is used to validate that migration files haven't been modified
/// after being applied to the database.
///
/// # Arguments
///
/// * `migration_file_path` - Path to the migration file
///
/// # Returns
///
/// Returns the hexadecimal SHA-256 hash of the file content
///
/// # Errors
///
/// Returns `LifeError::Other` if the file cannot be read
pub fn calculate_checksum(migration_file_path: &Path) -> Result<String, LifeError> {
    // Read migration file content
    let content = fs::read_to_string(migration_file_path)
        .map_err(|e| LifeError::Other(format!("Failed to read migration file {}: {}", migration_file_path.display(), e)))?;
    
    // Calculate SHA-256 hash
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let hash = hasher.finalize();
    
    // Return hexadecimal representation
    Ok(format!("{:x}", hash))
}

/// Validate checksum against stored value
///
/// # Arguments
///
/// * `stored_checksum` - The checksum stored in the database
/// * `current_checksum` - The checksum calculated from the current file
///
/// # Returns
///
/// Returns `Ok(())` if checksums match, or an error if they don't
pub fn validate_checksum(stored_checksum: &str, current_checksum: &str) -> Result<(), LifeError> {
    if stored_checksum == current_checksum {
        Ok(())
    } else {
        Err(LifeError::Other(format!(
            "Checksum mismatch: stored={}, current={}",
            stored_checksum, current_checksum
        )))
    }
}
