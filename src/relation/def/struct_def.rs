//! RelationDef struct for storing relationship metadata.
//!
//! This module provides the `RelationDef` struct which contains all metadata about
//! entity relationships. It can be converted to SeaQuery `Condition` for use in
//! JOINs and WHERE clauses.

use crate::relation::def::types::RelationType;
use crate::relation::identity::Identity;
use sea_query::{Condition, ConditionType, DynIden, TableRef};
use std::sync::Arc;

/// Defines a relationship between two entities
///
/// This struct contains all metadata about a relationship, including:
/// - Relationship type (HasOne, HasMany, BelongsTo, HasManyThrough)
/// - Source and target tables
/// - Foreign key and primary key columns (supports composite keys via `Identity`)
/// - Through table for has_many_through relationships
/// - Additional metadata (ownership, foreign key constraints, etc.)
///
/// # Example
///
/// ```no_run
/// use lifeguard::relation::def::{RelationDef, RelationType};
/// use lifeguard::relation::identity::Identity;
/// use sea_query::{TableRef, ConditionType};
///
/// // Create a belongs_to relationship: Post -> User
/// use sea_query::{TableName, IntoIden};
/// let rel_def = RelationDef {
///     rel_type: RelationType::BelongsTo,
///     from_tbl: TableRef::Table(TableName(None, "posts".into_iden()), None),
///     to_tbl: TableRef::Table(TableName(None, "users".into_iden()), None),
///     from_col: Identity::Unary("user_id".into()),
///     to_col: Identity::Unary("id".into()),
///     through_tbl: None,
///     is_owner: true,
///     skip_fk: false,
///     on_condition: None,
///     condition_type: ConditionType::All,
/// };
/// ```
#[derive(Clone)]
pub struct RelationDef {
    /// Type of relationship
    pub rel_type: RelationType,
    /// Source table reference
    pub from_tbl: TableRef,
    /// Target table reference
    pub to_tbl: TableRef,
    /// Foreign key column(s) in source table
    pub from_col: Identity,
    /// Primary key column(s) in target table
    pub to_col: Identity,
    /// Through table reference for has_many_through relationships
    /// None for direct relationships (has_one, has_many, belongs_to)
    pub through_tbl: Option<TableRef>,
    /// Whether this entity owns the relationship
    pub is_owner: bool,
    /// Skip foreign key constraint generation
    pub skip_fk: bool,
    /// Optional custom join condition
    pub on_condition: Option<Arc<dyn Fn(DynIden, DynIden) -> Condition + Send + Sync>>,
    /// Condition type (All/Any)
    pub condition_type: ConditionType,
}

impl std::fmt::Debug for RelationDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelationDef")
            .field("rel_type", &self.rel_type)
            .field("from_tbl", &self.from_tbl)
            .field("to_tbl", &self.to_tbl)
            .field("from_col", &self.from_col)
            .field("to_col", &self.to_col)
            .field("through_tbl", &self.through_tbl)
            .field("is_owner", &self.is_owner)
            .field("skip_fk", &self.skip_fk)
            .field("on_condition", &if self.on_condition.is_some() { "Some" } else { "None" })
            .field("condition_type", &self.condition_type)
            .finish()
    }
}

impl RelationDef {
    /// Reverse this relation (swap from and to)
    ///
    /// This is useful for reversing a relationship direction.
    /// For example, if you have Post -> User (belongs_to),
    /// reversing gives User -> Post (has_many).
    pub fn rev(self) -> Self {
        Self {
            rel_type: self.rel_type,
            from_tbl: self.to_tbl,
            to_tbl: self.from_tbl,
            from_col: self.to_col,
            to_col: self.from_col,
            through_tbl: self.through_tbl,
            is_owner: !self.is_owner,
            skip_fk: self.skip_fk,
            on_condition: self.on_condition,
            condition_type: self.condition_type,
        }
    }
}
