//! `RelationDef` struct for storing relationship metadata.
//!
//! This module provides the `RelationDef` struct which contains all metadata about
//! entity relationships. It can be converted to `SeaQuery` `Condition` for use in
//! `JOIN`s and `WHERE` clauses.

use crate::relation::def::types::RelationType;
use crate::relation::identity::Identity;
use sea_query::{Condition, ConditionType, DynIden, TableRef};
use std::sync::Arc;

/// Defines a relationship between two entities
///
/// This struct contains all metadata about a relationship, including:
/// - Relationship type (`HasOne`, `HasMany`, `BelongsTo`, `HasManyThrough`)
/// - Source and target tables
/// - Foreign key and primary key columns (supports composite keys via `Identity`)
/// - Through table for `has_many_through` relationships
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
    /// Through table reference for `has_many_through` relationships
    /// `None` for direct relationships (`has_one`, `has_many`, `belongs_to`)
    pub through_tbl: Option<TableRef>,
    /// Foreign key column(s) in join table pointing to source entity (for `has_many_through`)
    /// `None` for direct relationships
    /// Example: For `Post -> PostTags -> Tags`, this would be `"post_id"` in `PostTags`
    pub through_from_col: Option<Identity>,
    /// Foreign key column(s) in join table pointing to target entity (for `has_many_through`)
    /// `None` for direct relationships
    /// Example: For `Post -> PostTags -> Tags`, this would be `"tag_id"` in `PostTags`
    pub through_to_col: Option<Identity>,
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
            .field("through_from_col", &self.through_from_col)
            .field("through_to_col", &self.through_to_col)
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
    /// For example, if you have `Post -> User` (`belongs_to`),
    /// reversing gives `User -> Post` (`has_many`).
    #[must_use]
    pub fn rev(self) -> Self {
        Self {
            rel_type: self.rel_type,
            from_tbl: self.to_tbl,
            to_tbl: self.from_tbl,
            from_col: self.to_col,
            to_col: self.from_col,
            through_tbl: self.through_tbl,
            through_from_col: self.through_to_col,
            through_to_col: self.through_from_col,
            is_owner: !self.is_owner,
            skip_fk: self.skip_fk,
            on_condition: self.on_condition,
            condition_type: self.condition_type,
        }
    }

    /// Generate a join condition expression from this `RelationDef`
    ///
    /// This method automatically generates an `Expr` that can be used in `JOIN ON` clauses
    /// based on the `from_col` and `to_col` `Identity` values. It supports both single
    /// and composite keys.
    ///
    /// For `has_many_through` relationships, use `join_on_exprs()` instead to get both joins.
    ///
    /// # Returns
    ///
    /// An `Expr` representing the join condition: `from_table.from_col = to_table.to_col`
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::relation::def::RelationDef;
    /// use sea_query::Expr;
    ///
    /// let rel_def = RelationDef { /* ... */ };
    /// let join_expr: Expr = rel_def.join_on_expr();
    /// // Use in left_join: query.left_join(related_entity, join_expr)
    /// ```
    #[must_use]
    pub fn join_on_expr(&self) -> sea_query::Expr {
        use crate::relation::def::condition::join_tbl_on_expr;
        join_tbl_on_expr(
            &self.from_tbl,
            &self.to_tbl,
            &self.from_col,
            &self.to_col,
        )
    }

    /// Generate join condition expressions for `has_many_through` relationships
    ///
    /// This method generates two join expressions needed for many-to-many relationships:
    /// 1. First join: `source_table.primary_key = through_table.through_from_col`
    /// 2. Second join: `through_table.through_to_col = target_table.primary_key`
    ///
    /// # Returns
    ///
    /// A tuple `(first_join, second_join)` where:
    /// - `first_join`: Join condition from source to through table
    /// - `second_join`: Join condition from through to target table
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Called on a non-`has_many_through` relationship
    /// - Required fields (`through_tbl`, `through_from_col`, `through_to_col`) are missing
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::relation::def::{RelationDef, RelationType};
    /// use sea_query::Expr;
    ///
    /// let rel_def = RelationDef {
    ///     rel_type: RelationType::HasManyThrough,
    ///     // ... other fields ...
    ///     # through_tbl: Some(/* ... */),
    ///     # through_from_col: Some(/* ... */),
    ///     # through_to_col: Some(/* ... */),
    /// };
    /// let (first_join, second_join) = rel_def.join_on_exprs()?;
    /// // Use in has_many_through: query.left_join(through_entity, first_join).left_join(target_entity, second_join)
    /// ```
    // Note: Result<(Expr, Expr), ...> is already #[must_use] from Result, so we don't need the attribute here
    pub fn join_on_exprs(&self) -> Result<(sea_query::Expr, sea_query::Expr), crate::executor::LifeError> {
        use crate::relation::def::condition::join_tbl_on_expr;
        use crate::relation::def::types::RelationType;
        
        if self.rel_type != RelationType::HasManyThrough {
            return Err(crate::executor::LifeError::Other(
                "join_on_exprs() can only be called on HasManyThrough relationships".to_string()
            ));
        }
        
        let through_tbl = self.through_tbl.as_ref().ok_or_else(|| {
            crate::executor::LifeError::Other("HasManyThrough relationship must have through_tbl set".to_string())
        })?;
        
        let through_from_col = self.through_from_col.as_ref().ok_or_else(|| {
            crate::executor::LifeError::Other("HasManyThrough relationship must have through_from_col set".to_string())
        })?;
        
        let through_to_col = self.through_to_col.as_ref().ok_or_else(|| {
            crate::executor::LifeError::Other("HasManyThrough relationship must have through_to_col set".to_string())
        })?;
        
        // First join: source_table.primary_key = through_table.through_from_col
        // For has_many_through, from_col is the source entity's primary key
        let first_join = join_tbl_on_expr(
            &self.from_tbl,
            through_tbl,
            &self.from_col,
            through_from_col,
        );
        
        // Second join: through_table.through_to_col = target_table.primary_key
        // For has_many_through, to_col is the target entity's primary key
        let second_join = join_tbl_on_expr(
            through_tbl,
            &self.to_tbl,
            through_to_col,
            &self.to_col,
        );
        
        Ok((first_join, second_join))
    }
}
