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
    /// Migration already registered in memory
    AlreadyRegistered { version: i64, name: String },
    /// Migration failed during execution
    ExecutionFailed {
        version: i64,
        name: String,
        error: String,
    },
    /// Invalid migration version (preserves original invalid string for diagnostics)
    InvalidVersion(String),
    /// Missing migration file
    MissingFile { version: i64, name: String },
    /// Migration file already exists
    FileAlreadyExists { path: String },
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::Database(e) => write!(f, "Database error: {e}"),
            MigrationError::FileNotFound(path) => write!(f, "Migration file not found: {path}"),
            MigrationError::InvalidFormat(msg) => write!(f, "Invalid migration format: {msg}"),
            MigrationError::ChecksumMismatch {
                version,
                name,
                stored,
                current,
            } => {
                write!(
                    f,
                    "Migration '{name}' (version {version}) has been modified after being applied.\n\
                     Stored checksum: {stored}\n\
                     Current checksum: {current}\n\
                     This indicates the migration file was edited after deployment."
                )
            }
            MigrationError::LockTimeout(msg) => {
                write!(
                    f,
                    "Migration lock timeout: {msg}\n\
                     Another process may be running migrations. If this persists, check for:\n\
                     - Stuck migration process\n\
                     - Database connection issues\n\
                     - Manual lock in lifeguard_migration_lock table"
                )
            }
            MigrationError::AlreadyApplied { version, name } => {
                write!(
                    f,
                    "Migration '{name}' (version {version}) has already been applied"
                )
            }
            MigrationError::AlreadyRegistered { version, name } => {
                write!(
                    f,
                    "Migration '{name}' (version {version}) is already registered in the migration registry"
                )
            }
            MigrationError::ExecutionFailed { version, name, error } => {
                write!(
                    f,
                    "Migration '{name}' (version {version}) failed during execution: {error}"
                )
            }
            MigrationError::InvalidVersion(version_str) => {
                write!(f, "Invalid migration version: '{version_str}' (expected format: YYYYMMDDHHMMSS)")
            }
            MigrationError::MissingFile { version, name } => {
                write!(
                    f,
                    "Applied migration file not found: m{version}_{name}.rs\n\
                     Suggestion: Ensure all migration files are present in migrations directory"
                )
            }
            MigrationError::FileAlreadyExists { path } => {
                write!(
                    f,
                    "Migration file already exists: {path}\n\
                     This can happen if:\n\
                     - Two migrations are generated within the same second with the same name\n\
                     - A migration file with this name already exists\n\
                     Suggestion: Use a different migration name or wait a second before generating again"
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
