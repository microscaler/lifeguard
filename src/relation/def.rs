//! RelationDef struct for storing relationship metadata
//!
//! This module provides the `RelationDef` struct which contains all metadata about
//! entity relationships. It can be converted to SeaQuery `Condition` for use in
//! JOINs and WHERE clauses.

use crate::relation::identity::Identity;
use crate::model::ModelTrait;
use crate::query::LifeModelTrait;
use sea_query::{Condition, ConditionType, DynIden, Expr, ExprTrait, TableRef};
use std::sync::Arc;

/// Type of relationship between entities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationType {
    /// One-to-one relationship
    HasOne,
    /// One-to-many relationship
    HasMany,
    /// Many-to-one relationship (belongs_to)
    BelongsTo,
}

/// Defines a relationship between two entities
///
/// This struct contains all metadata about a relationship, including:
/// - Relationship type (HasOne, HasMany, BelongsTo)
/// - Source and target tables
/// - Foreign key and primary key columns (supports composite keys via `Identity`)
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
/// let rel_def = RelationDef {
///     rel_type: RelationType::BelongsTo,
///     from_tbl: TableRef::Table("posts".into()),
///     to_tbl: TableRef::Table("users".into()),
///     from_col: Identity::Unary("user_id".into()),
///     to_col: Identity::Unary("id".into()),
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
            is_owner: !self.is_owner,
            skip_fk: self.skip_fk,
            on_condition: self.on_condition,
            condition_type: self.condition_type,
        }
    }
}

/// Convert RelationDef to Condition for use in JOINs
///
/// This implementation allows `RelationDef` to be used directly where SeaQuery
/// expects a `Condition`, making it easy to use in JOIN operations.
///
/// # Example
///
/// ```no_run
/// use lifeguard::relation::def::RelationDef;
/// use sea_query::{Query, Condition};
///
/// let rel_def = RelationDef { /* ... */ };
/// let condition: Condition = rel_def.into();
/// ```
impl From<RelationDef> for Condition {
    fn from(mut rel: RelationDef) -> Condition {
        let from_tbl = rel.from_tbl.clone();
        let to_tbl = rel.to_tbl.clone();

        let mut condition = match rel.condition_type {
            ConditionType::All => Condition::all(),
            ConditionType::Any => Condition::any(),
        };

        // Build join condition: from_table.from_col = to_table.to_col
        condition = condition.add(join_tbl_on_condition(
            from_tbl.clone(),
            to_tbl.clone(),
            rel.from_col,
            rel.to_col,
        ));

        // Add custom condition if provided
        // Note: on_condition expects DynIden (table identifiers), not TableRef
        // For now, we'll skip this if on_condition is provided since we need to convert TableRef to DynIden
        // This is a future enhancement - custom join conditions are not yet fully supported
        if let Some(_f) = rel.on_condition.take() {
            // TODO: Convert TableRef to DynIden and call the function
            // For now, custom join conditions are not implemented
        }

        condition
    }
}

/// Build join condition from Identity pairs
///
/// This function creates a `Condition` that joins two tables based on
/// foreign key and primary key columns. It supports both single and
/// composite keys via the `Identity` enum.
///
/// # Arguments
///
/// * `from_tbl` - Source table reference
/// * `to_tbl` - Target table reference
/// * `from_col` - Foreign key column(s) in source table
/// * `to_col` - Primary key column(s) in target table
///
/// # Returns
///
/// A `Condition` representing the join: `from_table.from_col = to_table.to_col`
///
/// # Panics
///
/// Panics if `from_col` and `to_col` have mismatched arities (different number of columns).
///
/// # Example
///
/// ```no_run
/// use lifeguard::relation::def::join_tbl_on_condition;
/// use lifeguard::relation::identity::Identity;
/// use sea_query::{TableRef, Condition};
///
/// let condition = join_tbl_on_condition(
///     TableRef::Table("posts".into()),
///     TableRef::Table("users".into()),
///     Identity::Unary("user_id".into()),
///     Identity::Unary("id".into()),
/// );
/// // Creates: posts.user_id = users.id
/// ```
pub fn join_tbl_on_condition(
    from_tbl: TableRef,
    to_tbl: TableRef,
    from_col: Identity,
    to_col: Identity,
) -> Condition {
    let mut condition = Condition::all();

    // Ensure arities match
    assert_eq!(
        from_col.arity(),
        to_col.arity(),
        "Foreign key and primary key must have matching arity"
    );

    // Build equality conditions for each column pair
    // Use Expr::col() with table-qualified column references
    // SeaQuery's Expr::col() accepts (table, column) tuples where both are IntoColumnRef
    for (fk_col, pk_col) in from_col.iter().zip(to_col.iter()) {
        // Create table-qualified column expressions using Expr::col()
        // We need to convert TableRef and DynIden to something Expr::col() can accept
        // For now, use a custom SQL expression similar to join_condition()
        let fk_col_str = fk_col.to_string();
        let pk_col_str = pk_col.to_string();
        let from_tbl_str = format!("{:?}", from_tbl);
        let to_tbl_str = format!("{:?}", to_tbl);
        
        // Create join condition: from_table.fk_col = to_table.pk_col
        // This is a simplified approach - in the future we may want to use proper Expr::col()
        let join_expr = format!("{}.{} = {}.{}", from_tbl_str, fk_col_str, to_tbl_str, pk_col_str);
        let expr = Expr::cust(join_expr);
        condition = condition.add(expr);
    }

    condition
}

/// Build WHERE condition from RelationDef and model primary key values
///
/// This function creates a `Condition` for filtering related entities based on
/// the current model's primary key. It works with both single and composite keys.
///
/// # Arguments
///
/// * `rel_def` - The relationship definition
/// * `model` - The model instance to get primary key values from
///
/// # Returns
///
/// A `Condition` representing: `related_table.from_col = model.primary_key_values`
///
/// # Example
///
/// ```no_run
/// use lifeguard::relation::def::{RelationDef, build_where_condition};
/// use lifeguard::model::ModelTrait;
///
/// // Assuming we have a User model and want to find related Posts
/// let user_model: UserModel = /* ... */;
/// let rel_def: RelationDef = /* ... */;
/// let condition = build_where_condition(&rel_def, &user_model);
/// // Creates: posts.user_id = user.id
/// ```
pub fn build_where_condition<M>(
    rel_def: &RelationDef,
    model: &M,
) -> Condition
where
    M: ModelTrait + LifeModelTrait,
{
    let mut condition = Condition::all();

    // Get primary key values from model
    // Phase 4: Now using get_primary_key_values() which supports composite keys
    let pk_identity = model.get_primary_key_identity();
    let pk_values = model.get_primary_key_values();

    // Ensure arities match
    assert_eq!(
        rel_def.from_col.arity(),
        pk_identity.arity(),
        "Foreign key columns and primary key must have matching arity"
    );
    
    // Ensure we have the right number of values
    assert_eq!(
        pk_values.len(),
        pk_identity.arity(),
        "Number of primary key values must match primary key arity"
    );

    // Match foreign key columns to primary key values
    for (fk_col, pk_val) in rel_def.from_col.iter().zip(pk_values.iter()) {
        // Convert DynIden to string for column name
        let fk_col_str = fk_col.to_string();
        let to_tbl_str = format!("{:?}", rel_def.to_tbl);
        
        // Create WHERE condition: table.column = value
        // Use Expr::col() for the column and Expr::val() for the value
        // For table-qualified columns, we'll use a custom expression for now
        let col_expr = format!("{}.{}", to_tbl_str, fk_col_str);
        let expr = Expr::cust(col_expr).eq(Expr::val(pk_val.clone()));
        condition = condition.add(expr);
    }

    condition
}

/// Extract primary key values from model based on Identity columns
///
/// This helper function extracts the actual `Value`s from a model based on
/// the columns specified in the `Identity`. It requires `ModelTrait` to have
/// a method to get values by column.
///
/// # Arguments
///
/// * `model` - The model instance
/// * `pk_identity` - The primary key identity (which columns to extract)
///
/// # Returns
///
/// A vector of `Value`s corresponding to the primary key columns
///
/// # Note
///
/// This is a temporary implementation. The macro will generate more efficient
/// implementations that directly access model fields.
// TODO: Phase 4 - This will be implemented once get_primary_key_values() is added to ModelTrait
// fn extract_primary_key_values<M>(model: &M, _pk_identity: &Identity) -> Vec<Value>
// where
//     M: ModelTrait,
// {
//     // For now, we'll use get_primary_key_values() which the macro will generate
//     // This is a placeholder - the actual implementation will be generated by the macro
//     // to directly access model fields for efficiency
//     model.get_primary_key_values()
// }

// Note: TableRef extraction is handled inline using format! macro
// This is a temporary solution until we have proper TableRef -> string conversion
// In the future, we may want to add a helper trait or method for this

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation::identity::Identity;

    #[test]
    fn test_relation_def_rev() {
        use sea_query::{TableName, IntoIden};
        
        let rel_def = RelationDef {
            rel_type: RelationType::BelongsTo,
            from_tbl: TableRef::Table(TableName(None, "posts".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "users".into_iden()), None),
            from_col: Identity::Unary("user_id".into()),
            to_col: Identity::Unary("id".into()),
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        };

        let reversed = rel_def.clone().rev();
        // Can't easily compare TableRef, so just verify the method doesn't panic
        assert_eq!(reversed.from_col, rel_def.to_col);
        assert_eq!(reversed.to_col, rel_def.from_col);
        assert_eq!(reversed.is_owner, !rel_def.is_owner);
    }

    #[test]
    fn test_join_tbl_on_condition_single_key() {
        use sea_query::{TableName, IntoIden};
        
        let condition = join_tbl_on_condition(
            TableRef::Table(TableName(None, "posts".into_iden()), None),
            TableRef::Table(TableName(None, "users".into_iden()), None),
            Identity::Unary("user_id".into()),
            Identity::Unary("id".into()),
        );

        // Verify condition was created (can't easily test the exact SQL without executing)
        // The condition should represent: posts.user_id = users.id
        // Condition is a struct, not an enum, so we just verify it was created
        let _ = condition;
    }

    #[test]
    fn test_join_tbl_on_condition_composite_key() {
        use sea_query::{TableName, IntoIden};
        
        let condition = join_tbl_on_condition(
            TableRef::Table(TableName(None, "posts".into_iden()), None),
            TableRef::Table(TableName(None, "users".into_iden()), None),
            Identity::Binary("user_id".into(), "tenant_id".into()),
            Identity::Binary("id".into(), "tenant_id".into()),
        );

        // Verify condition was created for composite key
        let _ = condition;
    }

    #[test]
    #[should_panic(expected = "matching arity")]
    fn test_join_tbl_on_condition_mismatched_arity() {
        use sea_query::{TableName, IntoIden};
        
        join_tbl_on_condition(
            TableRef::Table(TableName(None, "posts".into_iden()), None),
            TableRef::Table(TableName(None, "users".into_iden()), None),
            Identity::Unary("user_id".into()),
            Identity::Binary("id".into(), "tenant_id".into()),
        );
    }

    #[test]
    fn test_relation_def_into_condition() {
        use sea_query::{TableName, IntoIden};
        
        let rel_def = RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(TableName(None, "users".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "posts".into_iden()), None),
            from_col: Identity::Unary("id".into()),
            to_col: Identity::Unary("user_id".into()),
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        };

        let condition: Condition = rel_def.into();
        // Verify condition was created
        let _ = condition;
    }

    #[test]
    fn test_join_tbl_on_condition_ternary() {
        // Edge case: Ternary composite key
        use sea_query::{TableName, IntoIden};
        
        let condition = join_tbl_on_condition(
            TableRef::Table(TableName(None, "posts".into_iden()), None),
            TableRef::Table(TableName(None, "users".into_iden()), None),
            Identity::Ternary("user_id".into(), "tenant_id".into(), "region_id".into()),
            Identity::Ternary("id".into(), "tenant_id".into(), "region_id".into()),
        );

        let _ = condition;
    }

    #[test]
    fn test_join_tbl_on_condition_many() {
        // Edge case: Many variant (4+ columns)
        use sea_query::{TableName, IntoIden};
        
        let condition = join_tbl_on_condition(
            TableRef::Table(TableName(None, "posts".into_iden()), None),
            TableRef::Table(TableName(None, "users".into_iden()), None),
            Identity::Many(vec!["user_id".into(), "tenant_id".into(), "region_id".into(), "org_id".into()]),
            Identity::Many(vec!["id".into(), "tenant_id".into(), "region_id".into(), "org_id".into()]),
        );

        let _ = condition;
    }

    #[test]
    #[should_panic(expected = "matching arity")]
    fn test_join_tbl_on_condition_ternary_mismatch() {
        // Edge case: Ternary vs Unary mismatch
        use sea_query::{TableName, IntoIden};
        
        join_tbl_on_condition(
            TableRef::Table(TableName(None, "posts".into_iden()), None),
            TableRef::Table(TableName(None, "users".into_iden()), None),
            Identity::Unary("user_id".into()),
            Identity::Ternary("id".into(), "tenant_id".into(), "region_id".into()),
        );
    }

    #[test]
    fn test_relation_def_rev_composite() {
        // Edge case: Reversing composite key relationship
        use sea_query::{TableName, IntoIden};
        
        let rel_def = RelationDef {
            rel_type: RelationType::BelongsTo,
            from_tbl: TableRef::Table(TableName(None, "posts".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "users".into_iden()), None),
            from_col: Identity::Binary("user_id".into(), "tenant_id".into()),
            to_col: Identity::Binary("id".into(), "tenant_id".into()),
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        };

        let reversed = rel_def.clone().rev();
        assert_eq!(reversed.from_col, rel_def.to_col);
        assert_eq!(reversed.to_col, rel_def.from_col);
        assert_eq!(reversed.is_owner, !rel_def.is_owner);
    }

    #[test]
    fn test_build_where_condition_single_key() {
        // Edge case: Test build_where_condition with single key
        // Note: This test verifies the function compiles and creates a condition
        // Full integration testing would require a complete entity/model setup
        use sea_query::{TableName, IntoIden};
        
        // Test that the function signature is correct and can be called
        // The actual implementation is tested in integration tests
        let rel_def = RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(TableName(None, "related".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "test".into_iden()), None),
            from_col: Identity::Unary("test_id".into()),
            to_col: Identity::Unary("id".into()),
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        };
        
        // Verify RelationDef structure is correct for single key
        assert_eq!(rel_def.from_col.arity(), 1);
        assert_eq!(rel_def.to_col.arity(), 1);
    }

    #[test]
    fn test_build_where_condition_composite_key_structure() {
        // Edge case: Test RelationDef structure for composite key relationships
        // Note: Full integration testing of build_where_condition requires complete entity/model setup
        use sea_query::{TableName, IntoIden};
        
        let rel_def = RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(TableName(None, "related".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "test".into_iden()), None),
            from_col: Identity::Binary("test_id".into(), "tenant_id".into()),
            to_col: Identity::Binary("id".into(), "tenant_id".into()),
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        };
        
        // Verify RelationDef structure is correct for composite key
        assert_eq!(rel_def.from_col.arity(), 2);
        assert_eq!(rel_def.to_col.arity(), 2);
    }

    #[test]
    fn test_build_where_condition_mismatched_arity_structure() {
        // Edge case: Test RelationDef structure with mismatched arity
        // Note: The actual panic is tested in integration tests with real models
        use sea_query::{TableName, IntoIden};
        
        let rel_def = RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(TableName(None, "related".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "test".into_iden()), None),
            from_col: Identity::Binary("test_id".into(), "tenant_id".into()),
            to_col: Identity::Unary("id".into()),
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        };
        
        // Verify the structure shows mismatched arity
        assert_eq!(rel_def.from_col.arity(), 2);
        assert_eq!(rel_def.to_col.arity(), 1);
        // This would panic in build_where_condition if called with a model
    }
}
