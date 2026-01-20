//! Migration-specific error types

use crate::LifeError;

/// Migration-specific errors
#[derive(Debug)]
pub enum MigrationError {
    /// Database execution error
    Database(LifeError),
    /// Migration file not found
    FileNotFound(String),
    /// Invalid migration file format
    InvalidFormat(String),
    /// Checksum mismatch
    ChecksumMismatch {
        version: i64,
        name: String,
        stored: String,
        current: String,
    },
    /// Migration lock timeout
    LockTimeout(String),
    /// Migration already applied
    AlreadyApplied { version: i64, name: String },
    /// Migration failed during execution
    ExecutionFailed {
        version: i64,
        name: String,
        error: String,
    },
    /// Invalid migration version
    InvalidVersion(i64),
    /// Missing migration file
    MissingFile { version: i64, name: String },
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::Database(e) => write!(f, "Database error: {}", e),
            MigrationError::FileNotFound(path) => write!(f, "Migration file not found: {}", path),
            MigrationError::InvalidFormat(msg) => write!(f, "Invalid migration format: {}", msg),
            MigrationError::ChecksumMismatch {
                version,
                name,
                stored,
                current,
            } => {
                write!(
                    f,
                    "Migration '{}' (version {}) has been modified after being applied.\n\
                     Stored checksum: {}\n\
                     Current checksum: {}\n\
                     This indicates the migration file was edited after deployment.",
                    name, version, stored, current
                )
            }
            MigrationError::LockTimeout(msg) => {
                write!(
                    f,
                    "Migration lock timeout: {}\n\
                     Another process may be running migrations. If this persists, check for:\n\
                     - Stuck migration process\n\
                     - Database connection issues\n\
                     - Manual lock in lifeguard_migration_lock table",
                    msg
                )
            }
            MigrationError::AlreadyApplied { version, name } => {
                write!(
                    f,
                    "Migration '{}' (version {}) has already been applied",
                    name, version
                )
            }
            MigrationError::ExecutionFailed { version, name, error } => {
                write!(
                    f,
                    "Migration '{}' (version {}) failed during execution: {}",
                    name, version, error
                )
            }
            MigrationError::InvalidVersion(version) => {
                write!(f, "Invalid migration version: {}", version)
            }
            MigrationError::MissingFile { version, name } => {
                write!(
                    f,
                    "Applied migration file not found: m{}_{}.rs\n\
                     Suggestion: Ensure all migration files are present in migrations directory",
                    version, name
                )
            }
        }
    }
}

impl std::error::Error for MigrationError {}

impl From<LifeError> for MigrationError {
    fn from(error: LifeError) -> Self {
        MigrationError::Database(error)
    }
}
