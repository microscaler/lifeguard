//! Table definition metadata for entity-driven migrations.
//!
//! This module provides `TableDefinition` which stores table-level metadata
//! including composite unique constraints, indexes, CHECK constraints, and table comments.

/// Table definition metadata
///
/// Stores information about table-level constraints, indexes, and metadata.
/// This is used for entity-driven migration generation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub struct TableDefinition {
    /// Table comment/documentation
    pub table_comment: Option<String>,
    /// Composite unique constraints (multi-column unique)
    /// Each entry is a vector of column names
    pub composite_unique: Vec<Vec<String>>,
    /// Index definitions
    /// Each entry is: (`index_name`, `columns`, `unique`, `partial_where`)
    pub indexes: Vec<IndexDefinition>,
    /// Table-level `CHECK` constraints
    /// Each entry is a tuple of (`constraint_name`, `expression`)
    /// If `constraint_name` is `None`, a default name will be generated from the table name
    pub check_constraints: Vec<(Option<String>, String)>,
}


/// Index definition metadata
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDefinition {
    /// Index name
    pub name: String,
    /// Column names (for composite indexes)
    pub columns: Vec<String>,
    /// Whether this is a unique index
    pub unique: bool,
    /// Partial index WHERE clause (if any)
    pub partial_where: Option<String>,
}
