//! Relation type definitions.
//!
//! This module provides the `RelationType` enum which represents the type
//! of relationship between entities.

/// Type of relationship between entities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationType {
    /// One-to-one relationship
    HasOne,
    /// One-to-many relationship
    HasMany,
    /// Many-to-one relationship (`belongs_to`)
    BelongsTo,
    /// Many-to-many relationship through a join table
    HasManyThrough,
}
