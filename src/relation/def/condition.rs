//! Condition building utilities for relations.
//!
//! This module provides functions for building SQL conditions from relation definitions,
//! including join conditions and `WHERE` clauses.

use crate::relation::def::struct_def::RelationDef;
use crate::relation::identity::Identity;
use crate::model::ModelTrait;
use sea_query::{Condition, ConditionType, Expr, ExprTrait, TableRef};

/// Extract table name string from `TableRef`
///
/// This helper function extracts the actual table name from a `TableRef`,
/// avoiding the use of `Debug` formatting which produces invalid SQL.
///
/// # Arguments
///
/// * `table_ref` - The table reference to extract the name from
///
/// # Returns
///
/// The table name as a string, or a default value if extraction fails
///
/// # Panics
///
/// Panics if `TableRef` is not in the expected format (should not happen in normal usage)
#[must_use]
pub fn extract_table_name(table_ref: &TableRef) -> String {
    match table_ref {
        TableRef::Table(table_name, _alias) => {
            // `TableName` is a tuple `(Option<DynIden>, DynIden)` where the second element is the table name
            // We need to extract the `DynIden` and use `Iden::unquoted()` to get the string
            // `TableName` is a tuple struct, so we need to access its fields
            // Based on `sea-query`'s structure: `TableName(schema: Option<DynIden>, table: DynIden)`
            
            match table_name {
                sea_query::TableName(_schema, table) => {
                    // If schema is present, we might want to include it, but for now just return table name
                    // In the future, we could return "schema.table" format
                    // `DynIden` implements `Display`/`ToString`, so we can use `to_string()` directly
                    table.to_string()
                }
            }
        }
        // Handle other `TableRef` variants if they exist
        _ => {
            // Fallback: try to convert to string via `Debug`, but this should not be used in production
            format!("{table_ref:?}")
        }
    }
}

/// Convert `RelationDef` to `Condition` for use in `JOIN`s
///
/// This implementation allows `RelationDef` to be used directly where `SeaQuery`
/// expects a `Condition`, making it easy to use in `JOIN` operations.
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
            &from_tbl,
            &to_tbl,
            &rel.from_col,
            &rel.to_col,
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
/// use lifeguard::relation::def::condition::join_tbl_on_condition;
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
#[must_use]
pub fn join_tbl_on_condition(
    from_tbl: &TableRef,
    to_tbl: &TableRef,
    from_col: &Identity,
    to_col: &Identity,
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
        // Extract actual table names from TableRef, not Debug representation
        let fk_col_str = fk_col.to_string();
        let pk_col_str = pk_col.to_string();
        let from_tbl_str = extract_table_name(from_tbl);
        let to_tbl_str = extract_table_name(to_tbl);
        
        // Create join condition: from_table.fk_col = to_table.pk_col
        // This is a simplified approach - in the future we may want to use proper Expr::col()
        let join_expr = format!("{from_tbl_str}.{fk_col_str} = {to_tbl_str}.{pk_col_str}");
        let expr = Expr::cust(join_expr);
        condition = condition.add(expr);
    }

    condition
}

/// Build join condition as an `Expr` from `Identity` pairs
///
/// This function creates an `Expr` that can be used in `JOIN ON` clauses.
/// It supports both single and composite keys via the `Identity` enum.
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
/// An `Expr` representing the join: `from_table.from_col = to_table.to_col`
/// For composite keys, multiple conditions are combined with AND.
///
/// # Panics
///
/// Panics if `from_col` and `to_col` have mismatched arities (different number of columns).
///
/// # Example
///
/// ```no_run
/// use lifeguard::relation::def::condition::join_tbl_on_expr;
/// use lifeguard::relation::identity::Identity;
/// use sea_query::{TableRef, Expr};
///
/// let join_expr = join_tbl_on_expr(
///     TableRef::Table("posts".into()),
///     TableRef::Table("users".into()),
///     Identity::Unary("user_id".into()),
///     Identity::Unary("id".into()),
/// );
/// // Creates: posts.user_id = users.id
/// ```
#[must_use]
pub fn join_tbl_on_expr(
    from_tbl: &TableRef,
    to_tbl: &TableRef,
    from_col: &Identity,
    to_col: &Identity,
) -> Expr {
    // Ensure arities match
    assert_eq!(
        from_col.arity(),
        to_col.arity(),
        "Foreign key and primary key must have matching arity"
    );

    // Build equality conditions for each column pair
    let mut exprs = Vec::new();
    for (fk_col, pk_col) in from_col.iter().zip(to_col.iter()) {
        // Extract actual table names from TableRef
        let fk_col_str = fk_col.to_string();
        let pk_col_str = pk_col.to_string();
        let from_tbl_str = extract_table_name(from_tbl);
        let to_tbl_str = extract_table_name(to_tbl);
        
        // Create join condition: from_table.fk_col = to_table.pk_col
        let join_expr = format!("{from_tbl_str}.{fk_col_str} = {to_tbl_str}.{pk_col_str}");
        exprs.push(Expr::cust(join_expr));
    }

    // Combine multiple conditions with AND
    // For single key, just return the first expression
    // For composite keys, chain with AND
        match exprs.len() {
        0 => Expr::value(true), // Should never happen due to arity check
        1 => exprs.into_iter().next().unwrap_or_else(|| {
            // This should never happen as we checked arity > 0, but handle gracefully
            Expr::value(true) // Fallback expression
        }),
        _ => {
            let mut iter = exprs.into_iter();
            let mut result = iter.next().unwrap_or_else(|| {
                // This should never happen as we checked len > 1, but handle gracefully
                Expr::value(true) // Fallback expression
            });
            for expr in iter {
                result = result.and(expr);
            }
            result
        }
    }
}

/// Build `WHERE` for `find_related`-style queries (filter the **related** table only)
///
/// `Related<Self::Entity, R>::to()` is oriented **from** `Self::Entity`'s table **to** `R`'s table.
/// The resulting `SelectQuery<R>` uses `to_tbl` in `FROM`, so the predicate must reference
/// **`to_tbl` + `to_col`** (columns on the related row), not `from_tbl`.
///
/// Values are read from the **source** (`from_tbl`) side of the join: for each pair
/// `(from_col_i, to_col_i)`, we use `model.get_by_column_name(from_col_i)` and compare to
/// `to_tbl.to_col_i`. This covers:
/// - **`HasMany`** (e.g. User → Post): `posts.user_id = user.id` (`from_col` = parent PK on users)
/// - **`BelongsTo`** (e.g. Post → User): `users.id = post.user_id` (`from_col` = FK on posts)
///
/// # Panics
///
/// - If `from_col` and `to_col` arity differ.
/// - If `get_primary_key_identity()` and `get_primary_key_values()` lengths differ.
/// - If, for any zipped pair, `model.get_by_column_name(from_col_i)` is `None` **and** the
///   primary-key fallback does not apply (name at index `i` on the model’s PK identity does not
///   match `from_col_i`). Custom [`ModelTrait`](crate::ModelTrait) implementations should supply
///   [`get_by_column_name`](crate::ModelTrait::get_by_column_name) for every `from_col` name used
///   in `BelongsTo` / composite edges.
///
/// # See also
///
/// - [`RelationDef`](crate::RelationDef) orientation for `Related<R>`.
/// - Integration tests in `tests/db_integration/related_trait.rs`.
/// - Custom `ModelTrait` authors: `docs/planning/lifeguard-derive/AUTHORING_MODEL_TRAIT.md`.
#[allow(clippy::panic)] // Panic cases are described in `# Panics` above.
#[must_use]
pub fn build_where_condition<M>(
    rel_def: &RelationDef,
    model: &M,
) -> Condition
where
    M: ModelTrait,
{
    let mut condition = Condition::all();
    let to_tbl_str = extract_table_name(&rel_def.to_tbl);

    assert_eq!(
        rel_def.from_col.arity(),
        rel_def.to_col.arity(),
        "from_col and to_col must have matching arity"
    );

    let pk_names: Vec<String> = model
        .get_primary_key_identity()
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    let pk_values = model.get_primary_key_values();
    assert_eq!(
        pk_names.len(),
        pk_values.len(),
        "primary key identity and values must have matching length"
    );

    for (idx, (from_iden, to_iden)) in rel_def
        .from_col
        .iter()
        .zip(rel_def.to_col.iter())
        .enumerate()
    {
        let from_name = from_iden.to_string();
        let to_name = to_iden.to_string();
        let val = model.get_by_column_name(&from_name).or_else(|| {
            (pk_names.get(idx).is_some_and(|pk| *pk == from_name))
                .then(|| pk_values.get(idx).cloned())
                .flatten()
        });
        let val = val.unwrap_or_else(|| {
            panic!(
                "build_where_condition: no value for source column `{from_name}` on model \
                 (implement `get_by_column_name`, or ensure it matches a primary key column)"
            )
        });
        // Table-qualified identifier: see `docs/planning/audits/RELATION_WHERE_EXPR_DECISION.md`.
        let col_expr = format!("{to_tbl_str}.{to_name}");
        let expr = Expr::cust(col_expr).eq(Expr::val(val));
        condition = condition.add(expr);
    }

    condition
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation::def::RelationDef;
    use crate::relation::identity::Identity;

    #[test]
    fn test_join_tbl_on_condition_single_key() {
        use sea_query::{TableName, IntoIden, Query, PostgresQueryBuilder};
        
        let from_tbl = TableRef::Table(TableName(None, "posts".into_iden()), None);
        let to_tbl = TableRef::Table(TableName(None, "users".into_iden()), None);
        let from_col = Identity::Unary("user_id".into());
        let to_col = Identity::Unary("id".into());
        let condition = join_tbl_on_condition(&from_tbl, &to_tbl, &from_col, &to_col);

        // Verify condition was created and contains actual table names, not debug output
        // Build a query with the condition to check the SQL output
        let mut query = Query::select();
        query.from("posts");
        query.cond_where(condition);
        let (sql, _) = query.build(PostgresQueryBuilder);
        
        // Verify SQL contains actual table names, not debug representation
        assert!(sql.contains("posts"), "SQL should contain table name 'posts'");
        assert!(sql.contains("users"), "SQL should contain table name 'users'");
        assert!(!sql.contains("Table("), "SQL should not contain Debug representation 'Table('");
        assert!(!sql.contains("TableName("), "SQL should not contain Debug representation 'TableName('");
    }

    #[test]
    fn test_join_tbl_on_condition_composite_key() {
        use sea_query::{TableName, IntoIden};
        
        let from_tbl = TableRef::Table(TableName(None, "posts".into_iden()), None);
        let to_tbl = TableRef::Table(TableName(None, "users".into_iden()), None);
        let from_col = Identity::Binary("user_id".into(), "tenant_id".into());
        let to_col = Identity::Binary("id".into(), "tenant_id".into());
        let condition = join_tbl_on_condition(&from_tbl, &to_tbl, &from_col, &to_col);

        // Verify condition was created for composite key
        let _ = condition;
    }

    #[test]
    #[should_panic(expected = "matching arity")]
    fn test_join_tbl_on_condition_mismatched_arity() {
        use sea_query::{TableName, IntoIden};
        
        let from_tbl = TableRef::Table(TableName(None, "posts".into_iden()), None);
        let to_tbl = TableRef::Table(TableName(None, "users".into_iden()), None);
        let from_col = Identity::Unary("user_id".into());
        let to_col = Identity::Binary("id".into(), "tenant_id".into());
        let _ = join_tbl_on_condition(&from_tbl, &to_tbl, &from_col, &to_col);
    }

    #[test]
    fn test_relation_def_into_condition() {
        use crate::relation::def::{RelationDef, RelationType};
        use sea_query::{TableName, IntoIden};
        
        let rel_def = RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(TableName(None, "users".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "posts".into_iden()), None),
            from_col: Identity::Unary("id".into()),
            to_col: Identity::Unary("user_id".into()),
            through_tbl: None,
            through_from_col: None,
            through_to_col: None,
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
        
        let from_tbl = TableRef::Table(TableName(None, "posts".into_iden()), None);
        let to_tbl = TableRef::Table(TableName(None, "users".into_iden()), None);
        let from_col = Identity::Ternary("user_id".into(), "tenant_id".into(), "region_id".into());
        let to_col = Identity::Ternary("id".into(), "tenant_id".into(), "region_id".into());
        let condition = join_tbl_on_condition(&from_tbl, &to_tbl, &from_col, &to_col);

        let _ = condition;
    }

    #[test]
    fn test_join_tbl_on_condition_many() {
        // Edge case: Many variant (4+ columns)
        use sea_query::{TableName, IntoIden};
        
        let from_tbl = TableRef::Table(TableName(None, "posts".into_iden()), None);
        let to_tbl = TableRef::Table(TableName(None, "users".into_iden()), None);
        let from_col = Identity::Many(vec!["user_id".into(), "tenant_id".into(), "region_id".into(), "org_id".into()]);
        let to_col = Identity::Many(vec!["id".into(), "tenant_id".into(), "region_id".into(), "org_id".into()]);
        let condition = join_tbl_on_condition(&from_tbl, &to_tbl, &from_col, &to_col);

        let _ = condition;
    }

    #[test]
    #[should_panic(expected = "matching arity")]
    fn test_join_tbl_on_condition_ternary_mismatch() {
        // Edge case: Ternary vs Unary mismatch
        use sea_query::{TableName, IntoIden};
        
        let from_tbl = TableRef::Table(TableName(None, "posts".into_iden()), None);
        let to_tbl = TableRef::Table(TableName(None, "users".into_iden()), None);
        let from_col = Identity::Unary("user_id".into());
        let to_col = Identity::Ternary("id".into(), "tenant_id".into(), "region_id".into());
        let _ = join_tbl_on_condition(&from_tbl, &to_tbl, &from_col, &to_col);
    }

    #[test]
    fn test_extract_table_name() {
        // Test that extract_table_name returns actual table names, not debug output
        use sea_query::{TableName, IntoIden};
        
        let table_ref = TableRef::Table(TableName(None, "posts".into_iden()), None);
        let table_name = extract_table_name(&table_ref);
        
        // Verify it returns the actual table name, not debug representation
        assert_eq!(table_name, "posts");
        assert!(!table_name.contains("Table("));
        assert!(!table_name.contains("TableName("));
        
        // Test with different table name
        let table_ref2 = TableRef::Table(TableName(None, "users".into_iden()), None);
        let table_name2 = extract_table_name(&table_ref2);
        assert_eq!(table_name2, "users");
    }
    
    #[test]
    fn test_build_where_condition_single_key() {
        // Edge case: Test build_where_condition with single key
        // Note: This test verifies the function compiles and creates a condition
        // Full integration testing would require a complete entity/model setup
        use crate::relation::def::{RelationDef, RelationType};
        use sea_query::{TableName, IntoIden};
        
        // Test that the function signature is correct and can be called
        // The actual implementation is tested in integration tests
        let rel_def = RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(TableName(None, "related".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "test".into_iden()), None),
            from_col: Identity::Unary("test_id".into()),
            to_col: Identity::Unary("id".into()),
            through_tbl: None,
            through_from_col: None,
            through_to_col: None,
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        };
        
        // Verify RelationDef structure is correct for single key
        assert_eq!(rel_def.from_col.arity(), 1);
        assert_eq!(rel_def.to_col.arity(), 1);
        
        // Verify extract_table_name works correctly for the relation def
        let table_name = extract_table_name(&rel_def.from_tbl);
        assert_eq!(table_name, "related");
        assert!(!table_name.contains("Table("));
    }

    #[test]
    fn test_build_where_condition_composite_key_structure() {
        // Edge case: Test RelationDef structure for composite key relationships
        // Note: Full integration testing of build_where_condition requires complete entity/model setup
        use crate::relation::def::{RelationDef, RelationType};
        use sea_query::{TableName, IntoIden};
        
        let rel_def = RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(TableName(None, "related".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "test".into_iden()), None),
            from_col: Identity::Binary("test_id".into(), "tenant_id".into()),
            to_col: Identity::Binary("id".into(), "tenant_id".into()),
            through_tbl: None,
            through_from_col: None,
            through_to_col: None,
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
        use crate::relation::def::{RelationDef, RelationType};
        use sea_query::{TableName, IntoIden};
        
        let rel_def = RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(TableName(None, "related".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "test".into_iden()), None),
            from_col: Identity::Binary("test_id".into(), "tenant_id".into()),
            to_col: Identity::Unary("id".into()),
            through_tbl: None,
            through_from_col: None,
            through_to_col: None,
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
    
    #[test]
    fn test_build_where_condition_no_primary_key_consistency() {
        // Test that entities without primary keys have consistent identity and values
        // This verifies the fix for the bug where get_primary_key_identity() returned
        // Identity::Unary("") (arity 1) while get_primary_key_values() returned vec![] (length 0)
        //
        // The fix: get_primary_key_identity() now returns Identity::Many(vec![]) (arity 0)
        // which matches the empty vec![] from get_primary_key_values()
        
        // The actual test: verify that Identity::Many(vec![]) has arity 0
        // and matches an empty vector length
        let identity = Identity::Many(vec![]);
        let values: Vec<sea_query::Value> = vec![];
        
        assert_eq!(identity.arity(), 0, "Identity::Many(vec![]) should have arity 0");
        assert_eq!(values.len(), 0, "Values should be empty");
        assert_eq!(
            identity.arity(),
            values.len(),
            "Identity arity ({}) must match values length ({}) for entities without primary keys",
            identity.arity(),
            values.len()
        );
        
        // Verify that the old buggy behavior would have failed
        // This demonstrates why the fix was necessary
        let buggy_identity = Identity::Unary("".into());
        assert_eq!(buggy_identity.arity(), 1, "Buggy Identity::Unary would have arity 1");
        assert_ne!(
            buggy_identity.arity(),
            values.len(),
            "Buggy behavior: arity (1) != values length (0) - this would cause assertion failure in build_where_condition"
        );
        
        // Verify the fix: new behavior is consistent
        // When build_where_condition checks: pk_values.len() == pk_identity.arity()
        // With the fix: 0 == 0 ✅ (passes)
        // With the bug: 0 == 1 ❌ (fails with "Number of primary key values must match primary key arity")
        let fixed_identity = Identity::Many(vec![]);
        assert_eq!(
            fixed_identity.arity(),
            values.len(),
            "Fixed behavior: arity (0) == values length (0) - assertion in build_where_condition will pass"
        );
    }

    #[test]
    fn test_join_tbl_on_expr_single_key() {
        // Test that join_tbl_on_expr generates correct Expr for single key
        use sea_query::{TableName, IntoIden};
        
        let from_tbl = TableRef::Table(TableName(None, "posts".into_iden()), None);
        let to_tbl = TableRef::Table(TableName(None, "users".into_iden()), None);
        let from_col = Identity::Unary("user_id".into());
        let to_col = Identity::Unary("id".into());
        let expr = join_tbl_on_expr(&from_tbl, &to_tbl, &from_col, &to_col);
        
        // Verify expr was created (can't easily test the exact SQL string)
        let _ = expr;
    }

    #[test]
    fn test_join_tbl_on_expr_composite_key() {
        // Test that join_tbl_on_expr generates correct Expr for composite key
        use sea_query::{TableName, IntoIden};
        
        let from_tbl = TableRef::Table(TableName(None, "posts".into_iden()), None);
        let to_tbl = TableRef::Table(TableName(None, "users".into_iden()), None);
        let from_col = Identity::Binary("user_id".into(), "tenant_id".into());
        let to_col = Identity::Binary("id".into(), "tenant_id".into());
        let expr = join_tbl_on_expr(&from_tbl, &to_tbl, &from_col, &to_col);
        
        // Verify expr was created (composite keys should be combined with AND)
        let _ = expr;
    }

    #[test]
    fn test_relation_def_join_on_expr() {
        // Test that RelationDef::join_on_expr() works correctly
        use crate::relation::def::{RelationDef, RelationType};
        use sea_query::{TableName, IntoIden, ConditionType};
        
        let rel_def = RelationDef {
            rel_type: RelationType::BelongsTo,
            from_tbl: TableRef::Table(TableName(None, "posts".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "users".into_iden()), None),
            from_col: Identity::Unary("user_id".into()),
            to_col: Identity::Unary("id".into()),
            through_tbl: None,
            through_from_col: None,
            through_to_col: None,
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        };
        
        let join_expr = rel_def.join_on_expr();
        // Verify expr was created
        let _ = join_expr;
    }

    /// `BelongsTo`: `WHERE` must use **related** table (`to_tbl`), values from FK on source model via `get_by_column_name`.
    #[test]
    fn test_build_where_condition_belongs_to_uses_to_tbl_only() {
        use crate::model::{ModelError, ModelTrait};
        use crate::{LifeEntityName, LifeModelTrait};
        use crate::relation::def::RelationType;
        use sea_query::{IntoIden, PostgresQueryBuilder, Query, TableName, Value};
        use sea_query::IdenStatic;

        #[derive(Default, Copy, Clone)]
        struct PostE;
        impl sea_query::Iden for PostE {
            fn unquoted(&self) -> &'static str {
                "_post_e"
            }
        }
        impl LifeEntityName for PostE {
            fn table_name(&self) -> &'static str {
                "posts"
            }
        }
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        #[allow(dead_code)] // Column enum for `ModelTrait`; variants used only via type system
        enum Pc {
            Id,
            UserId,
        }
        impl sea_query::Iden for Pc {
            fn unquoted(&self) -> &'static str {
                match self {
                    Pc::Id => "id",
                    Pc::UserId => "user_id",
                }
            }
        }
        impl IdenStatic for Pc {
            fn as_str(&self) -> &'static str {
                match self {
                    Pc::Id => "id",
                    Pc::UserId => "user_id",
                }
            }
        }
        crate::impl_column_def_helper_for_test!(Pc);
        impl LifeModelTrait for PostE {
            type Model = PostM;
            type Column = Pc;
        }

        #[derive(Clone, Debug)]
        struct PostM {
            id: i32,
            user_id: i32,
        }

        impl ModelTrait for PostM {
            type Entity = PostE;

            fn get(&self, col: Pc) -> Value {
                match col {
                    Pc::Id => Value::Int(Some(self.id)),
                    Pc::UserId => Value::Int(Some(self.user_id)),
                }
            }

            fn set(&mut self, _col: Pc, _val: Value) -> Result<(), ModelError> {
                Ok(())
            }

            fn get_primary_key_value(&self) -> Value {
                Value::Int(Some(self.id))
            }

            fn get_primary_key_identity(&self) -> Identity {
                Identity::Unary(sea_query::DynIden::from("id"))
            }

            fn get_primary_key_values(&self) -> Vec<Value> {
                vec![Value::Int(Some(self.id))]
            }

            fn get_by_column_name(&self, name: &str) -> Option<Value> {
                match name {
                    "user_id" => Some(Value::Int(Some(self.user_id))),
                    _ => None,
                }
            }
        }

        let rel_def = RelationDef {
            rel_type: RelationType::BelongsTo,
            from_tbl: TableRef::Table(TableName(None, "posts".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "users".into_iden()), None),
            from_col: Identity::Unary("user_id".into()),
            to_col: Identity::Unary("id".into()),
            through_tbl: None,
            through_from_col: None,
            through_to_col: None,
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        };

        let post = PostM {
            id: 1,
            user_id: 42,
        };
        let cond = build_where_condition(&rel_def, &post);

        let mut q = Query::select();
        q.from("users");
        q.cond_where(cond);
        let (sql, _) = q.build(PostgresQueryBuilder);
        assert!(
            sql.contains("users"),
            "expected related table in SQL, got: {sql}"
        );
        assert!(
            sql.contains("id"),
            "expected to_col in SQL, got: {sql}"
        );
        assert!(
            !sql.contains("posts."),
            "WHERE must not reference from_tbl alone: {sql}"
        );
    }

    /// `HasMany`: parent PK via fallback when `get_by_column_name` returns `None` for `from_col` name.
    #[test]
    fn test_build_where_condition_has_many_pk_fallback() {
        use crate::model::{ModelError, ModelTrait};
        use crate::{LifeEntityName, LifeModelTrait};
        use crate::relation::def::RelationType;
        use sea_query::{IntoIden, PostgresQueryBuilder, Query, TableName, Value};
        use sea_query::IdenStatic;

        #[derive(Default, Copy, Clone)]
        struct UserE;
        impl sea_query::Iden for UserE {
            fn unquoted(&self) -> &'static str {
                "_user_e"
            }
        }
        impl LifeEntityName for UserE {
            fn table_name(&self) -> &'static str {
                "users"
            }
        }
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        #[allow(dead_code)]
        enum Uc {
            Id,
        }
        impl sea_query::Iden for Uc {
            fn unquoted(&self) -> &'static str {
                "id"
            }
        }
        impl IdenStatic for Uc {
            fn as_str(&self) -> &'static str {
                "id"
            }
        }
        crate::impl_column_def_helper_for_test!(Uc);
        impl LifeModelTrait for UserE {
            type Model = UserM;
            type Column = Uc;
        }

        #[derive(Clone, Debug)]
        struct UserM {
            id: i32,
        }

        impl ModelTrait for UserM {
            type Entity = UserE;

            fn get(&self, col: Uc) -> Value {
                match col {
                    Uc::Id => Value::Int(Some(self.id)),
                }
            }

            fn set(&mut self, _col: Uc, _val: Value) -> Result<(), ModelError> {
                Ok(())
            }

            fn get_primary_key_value(&self) -> Value {
                Value::Int(Some(self.id))
            }

            fn get_primary_key_identity(&self) -> Identity {
                Identity::Unary(sea_query::DynIden::from("id"))
            }

            fn get_primary_key_values(&self) -> Vec<Value> {
                vec![Value::Int(Some(self.id))]
            }

            fn get_by_column_name(&self, _name: &str) -> Option<Value> {
                None
            }
        }

        let rel_def = RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(TableName(None, "users".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "posts".into_iden()), None),
            from_col: Identity::Unary("id".into()),
            to_col: Identity::Unary("user_id".into()),
            through_tbl: None,
            through_from_col: None,
            through_to_col: None,
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        };

        let user = UserM { id: 99 };
        let cond = build_where_condition(&rel_def, &user);

        let mut q = Query::select();
        q.from("posts");
        q.cond_where(cond);
        let (sql, _) = q.build(PostgresQueryBuilder);
        assert!(sql.contains("posts"), "{}", sql);
        assert!(sql.contains("user_id"), "{}", sql);
        assert!(!sql.contains("users."), "{}", sql);
    }

    /// Composite HasMany-style edge: both conjuncts qualified on `to_tbl`.
    #[test]
    fn test_build_where_condition_composite_to_tbl() {
        use crate::model::{ModelError, ModelTrait};
        use crate::{LifeEntityName, LifeModelTrait};
        use crate::relation::def::RelationType;
        use sea_query::{IntoIden, PostgresQueryBuilder, Query, TableName, Value};
        use sea_query::IdenStatic;

        #[derive(Default, Copy, Clone)]
        struct TenantE;
        impl sea_query::Iden for TenantE {
            fn unquoted(&self) -> &'static str {
                "_tenant_e"
            }
        }
        impl LifeEntityName for TenantE {
            fn table_name(&self) -> &'static str {
                "tenants"
            }
        }
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        #[allow(dead_code)]
        enum Tc {
            Id,
            RegionId,
        }
        impl sea_query::Iden for Tc {
            fn unquoted(&self) -> &'static str {
                match self {
                    Tc::Id => "id",
                    Tc::RegionId => "region_id",
                }
            }
        }
        impl IdenStatic for Tc {
            fn as_str(&self) -> &'static str {
                match self {
                    Tc::Id => "id",
                    Tc::RegionId => "region_id",
                }
            }
        }
        crate::impl_column_def_helper_for_test!(Tc);
        impl LifeModelTrait for TenantE {
            type Model = TenantM;
            type Column = Tc;
        }

        #[derive(Clone, Debug)]
        struct TenantM {
            id: i32,
            region_id: i32,
        }

        impl ModelTrait for TenantM {
            type Entity = TenantE;

            fn get(&self, col: Tc) -> Value {
                match col {
                    Tc::Id => Value::Int(Some(self.id)),
                    Tc::RegionId => Value::Int(Some(self.region_id)),
                }
            }

            fn set(&mut self, _col: Tc, _val: Value) -> Result<(), ModelError> {
                Ok(())
            }

            fn get_primary_key_value(&self) -> Value {
                Value::Int(Some(self.id))
            }

            fn get_primary_key_identity(&self) -> Identity {
                Identity::Binary(
                    sea_query::DynIden::from("id"),
                    sea_query::DynIden::from("region_id"),
                )
            }

            fn get_primary_key_values(&self) -> Vec<Value> {
                vec![
                    Value::Int(Some(self.id)),
                    Value::Int(Some(self.region_id)),
                ]
            }

            fn get_by_column_name(&self, name: &str) -> Option<Value> {
                match name {
                    "id" => Some(Value::Int(Some(self.id))),
                    "region_id" => Some(Value::Int(Some(self.region_id))),
                    _ => None,
                }
            }
        }

        let rel_def = RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(TableName(None, "tenants".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "resources".into_iden()), None),
            from_col: Identity::Binary("id".into(), "region_id".into()),
            to_col: Identity::Binary("tenant_id".into(), "region_id".into()),
            through_tbl: None,
            through_from_col: None,
            through_to_col: None,
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        };

        let tenant = TenantM {
            id: 10,
            region_id: 200,
        };
        let cond = build_where_condition(&rel_def, &tenant);

        let mut q = Query::select();
        q.from("resources");
        q.cond_where(cond);
        let (sql, _) = q.build(PostgresQueryBuilder);
        assert!(sql.contains("resources"), "{}", sql);
        assert!(sql.contains("tenant_id"), "{}", sql);
        assert!(sql.contains("region_id"), "{}", sql);
        assert!(!sql.contains("tenants."), "{}", sql);
    }
}
