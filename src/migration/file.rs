//! Migration file discovery and parsing

use crate::migration::MigrationError;
use std::path::{Path, PathBuf};
use std::fs;
use regex::Regex;

/// Represents a discovered migration file
#[derive(Debug, Clone)]
pub struct MigrationFile {
    /// Path to the migration file
    pub path: PathBuf,
    
    /// Migration version (timestamp: YYYYMMDDHHMMSS)
    pub version: i64,
    
    /// Human-readable migration name
    pub name: String,
    
    /// SHA-256 checksum of the file content
    pub checksum: String,
}

impl MigrationFile {
    /// Create a new MigrationFile
    pub fn new(path: PathBuf, version: i64, name: String, checksum: String) -> Self {
        Self {
            path,
            version,
            name,
            checksum,
        }
    }
    
    /// Parse migration file name to extract version and name
    ///
    /// Expected format: `m{YYYYMMDDHHMMSS}_{name}.rs`
    ///
    /// # Example
    /// - `m20240120120000_create_users_table.rs` → version: 20240120120000, name: "create_users_table"
    pub fn parse_filename(filename: &str) -> Result<(i64, String), MigrationError> {
        // Pattern: m{14 digits}_{name}.rs
        let re = Regex::new(r"^m(\d{14})_(.+)\.rs$")
            .map_err(|e| MigrationError::InvalidFormat(format!("Invalid regex: {}", e)))?;
        
        if let Some(caps) = re.captures(filename) {
            let version_str = caps.get(1).unwrap().as_str();
            let name = caps.get(2).unwrap().as_str().to_string();
            
            let version = version_str.parse::<i64>()
                .map_err(|e| MigrationError::InvalidVersion(
                    version_str.parse::<i64>().unwrap_or(0)
                ))?;
            
            Ok((version, name))
        } else {
            Err(MigrationError::InvalidFormat(format!(
                "Migration file name '{}' does not match expected pattern: m{{YYYYMMDDHHMMSS}}_{{name}}.rs",
                filename
            )))
        }
    }
}

/// Discover all migration files in a directory
///
/// Scans the migrations directory for files matching the pattern `m{YYYYMMDDHHMMSS}_{name}.rs`,
/// parses their metadata, and returns them sorted by version (ascending).
///
/// # Arguments
///
/// * `migrations_dir` - Path to the migrations directory
///
/// # Returns
///
/// Returns a vector of `MigrationFile` structs, sorted by version (oldest first).
///
/// # Errors
///
/// Returns errors if:
/// - The directory doesn't exist or can't be read
/// - Migration files have invalid names
/// - Checksum calculation fails
pub fn discover_migrations(migrations_dir: &Path) -> Result<Vec<MigrationFile>, MigrationError> {
    // Check if directory exists
    if !migrations_dir.exists() {
        return Err(MigrationError::FileNotFound(
            migrations_dir.to_string_lossy().to_string()
        ));
    }
    
    if !migrations_dir.is_dir() {
        return Err(MigrationError::InvalidFormat(format!(
            "Path is not a directory: {}",
            migrations_dir.display()
        )));
    }
    
    let mut migrations = Vec::new();
    
    // Read directory entries
    let entries = fs::read_dir(migrations_dir)
        .map_err(|e| MigrationError::FileNotFound(format!(
            "Failed to read migrations directory {}: {}",
            migrations_dir.display(), e
        )))?;
    
    for entry in entries {
        let entry = entry.map_err(|e| MigrationError::FileNotFound(format!(
            "Failed to read directory entry: {}", e
        )))?;
        
        let path = entry.path();
        
        // Only process .rs files
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        
        // Extract filename
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| MigrationError::InvalidFormat(format!(
                "Invalid filename: {}", path.display()
            )))?;
        
        // Parse version and name from filename
        let (version, name) = MigrationFile::parse_filename(filename)?;
        
        // Calculate checksum
        let checksum = crate::migration::calculate_checksum(&path)
            .map_err(|e| MigrationError::InvalidFormat(format!(
                "Failed to calculate checksum for {}: {}",
                path.display(), e
            )))?;
        
        migrations.push(MigrationFile::new(path, version, name, checksum));
    }
    
    // Sort by version (ascending - oldest first)
    migrations.sort_by_key(|m| m.version);
    
    Ok(migrations)
}
