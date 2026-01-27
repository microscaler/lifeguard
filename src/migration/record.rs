//! `MigrationRecord` - Represents entries in the `lifeguard_migrations` state table

use chrono::{DateTime, Utc};

/// Represents a migration record in the `lifeguard_migrations` state table
///
/// This struct matches the schema defined in the migration state tracking table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationRecord {
    /// Migration version (timestamp: YYYYMMDDHHMMSS)
    pub version: i64,
    
    /// Human-readable migration name
    pub name: String,
    
    /// `SHA-256` checksum of migration file content
    pub checksum: String,
    
    /// When the migration was applied
    pub applied_at: DateTime<Utc>,
    
    /// Execution time in milliseconds (`None` if not recorded)
    pub execution_time_ms: Option<i64>,
    
    /// Whether the migration completed successfully
    pub success: bool,
}

impl MigrationRecord {
    /// Create a new `MigrationRecord`
    #[must_use]
    pub fn new(
        version: i64,
        name: String,
        checksum: String,
        applied_at: DateTime<Utc>,
        execution_time_ms: Option<i64>,
        success: bool,
    ) -> Self {
        Self {
            version,
            name,
            checksum,
            applied_at,
            execution_time_ms,
            success,
        }
    }
    
    /// Create a `MigrationRecord` from database row
    ///
    /// Expected column order: `version`, `name`, `checksum`, `applied_at`, `execution_time_ms`, `success`
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if the row data cannot be parsed or if timestamp parsing fails.
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if the row data cannot be parsed or if timestamp parsing fails.
    // Note: Result<T, E> is already #[must_use], so we don't need the attribute here
    #[allow(clippy::double_must_use)] // Result is already must_use, but from_row() is a conversion method
    pub fn from_row(row: &may_postgres::Row) -> Result<Self, crate::LifeError> {
        let version: i64 = row.get(0);
        let name: String = row.get(1);
        let checksum: String = row.get(2);
        
        // `PostgreSQL` `TIMESTAMP` is returned as a string in `may_postgres`
        // Parse it to `DateTime<Utc>`
        // Note: `may_postgres` returns timestamps as strings, so we need to parse them
        let applied_at_str: String = row.get(3);
        let applied_at = {
            // Try different timestamp formats
            if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&applied_at_str, "%Y-%m-%d %H:%M:%S%.f") {
                naive.and_utc()
            } else if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&applied_at_str, "%Y-%m-%d %H:%M:%S") {
                naive.and_utc()
            } else if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&applied_at_str, "%Y-%m-%dT%H:%M:%S%.f") {
                naive.and_utc()
            } else if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&applied_at_str, "%Y-%m-%dT%H:%M:%S") {
                naive.and_utc()
            } else {
                return Err(crate::LifeError::Other(format!(
                    "Failed to parse timestamp '{applied_at_str}': unrecognized format"
                )));
            }
        };
        
        let execution_time_ms: Option<i64> = row.get(4);
        let success: bool = row.get(5);
        
        Ok(Self {
            version,
            name,
            checksum,
            applied_at,
            execution_time_ms,
            success,
        })
    }
}
