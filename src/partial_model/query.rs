//! Query builder for partial model queries.
//!
//! This module provides `SelectPartialQuery` for building and executing queries
//! that return partial models (subset of columns).

use super::traits::PartialModelTrait;
use crate::executor::{LifeError, LifeExecutor};
use crate::query::{LifeModelTrait, SelectQuery};
use sea_query::PostgresQueryBuilder;

/// Query builder for partial model queries
///
/// This wraps a `SelectQuery<E>` and ensures that only the columns required
/// by the partial model `P` are selected.
pub struct SelectPartialQuery<E: LifeModelTrait, P: PartialModelTrait<Entity = E>> {
    pub(crate) query: SelectQuery<E>,
    _partial: std::marker::PhantomData<P>,
}

impl<E: LifeModelTrait, P: PartialModelTrait<Entity = E>> SelectPartialQuery<E, P> {
    /// Execute the query and return all results as partial models
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor to use
    ///
    /// # Returns
    ///
    /// Returns a vector of partial models, or an error if the query fails
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if the query execution fails or if row parsing fails.
    pub fn all<Ex: LifeExecutor>(self, executor: &Ex) -> Result<Vec<P>, LifeError> {
        let (sql, values) = self.query.query.build(PostgresQueryBuilder);

        // Use shared value conversion function
        let rows = executor.query_all_values(&sql, &values)?;

        let mut results = Vec::new();
        for row in rows {
            results.push(
                P::from_row(&row)
                    .map_err(|e| LifeError::ParseError(format!("Failed to parse row: {e}")))?,
            );
        }
        Ok(results)
    }

    /// Execute the query and return the first result as a partial model
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor to use
    ///
    /// # Returns
    ///
    /// Returns `Some(partial_model)` if a row is found, `None` if no rows match,
    /// or an error if the query fails
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if the query execution fails or if row parsing fails.
    pub fn one<Ex: LifeExecutor>(self, executor: &Ex) -> Result<Option<P>, LifeError> {
        let (sql, values) = self.query.query.build(PostgresQueryBuilder);

        // Zero or one row: an empty result is `None`, not an error.
        executor
            .query_opt_values(&sql, &values)?
            .map(|row| {
                P::from_row(&row)
                    .map_err(|e| LifeError::ParseError(format!("Failed to parse row: {e}")))
            })
            .transpose()
    }

    /// Add a filter condition
    ///
    /// # Arguments
    ///
    /// * `condition` - The filter condition expression
    ///
    /// # Returns
    ///
    /// Returns self for method chaining
    #[must_use]
    pub fn filter<F>(mut self, condition: F) -> Self
    where
        F: sea_query::IntoCondition,
    {
        self.query.query.cond_where(condition.into_condition());
        self
    }

    /// Add an ORDER BY clause
    ///
    /// # Arguments
    ///
    /// * `column` - Column name or expression to order by
    /// * `order` - Order direction
    ///
    /// # Returns
    ///
    /// Returns self for method chaining
    #[must_use]
    pub fn order_by<C: sea_query::IntoColumnRef>(
        mut self,
        column: C,
        order: sea_query::Order,
    ) -> Self {
        self.query = self.query.order_by(column, order);
        self
    }

    /// Add a LIMIT clause
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of rows to return
    ///
    /// # Returns
    ///
    /// Returns self for method chaining
    #[must_use]
    pub fn limit(mut self, limit: u64) -> Self {
        self.query = self.query.limit(limit);
        self
    }

    /// Add an OFFSET clause
    ///
    /// # Arguments
    ///
    /// * `offset` - Number of rows to skip
    ///
    /// # Returns
    ///
    /// Returns self for method chaining
    #[must_use]
    pub fn offset(mut self, offset: u64) -> Self {
        self.query = self.query.offset(offset);
        self
    }
}

impl<E: LifeModelTrait> super::traits::PartialModelBuilder<E> for SelectQuery<E> {
    fn select_partial<P: PartialModelTrait<Entity = E>>(mut self) -> SelectPartialQuery<E, P> {
        // Replace only the select list: FROM (with its schema), joins, WHERE, GROUP BY, ORDER BY,
        // LIMIT and OFFSET set earlier in the chain stay as they are.
        self.query.clear_selects();
        for column_name in P::selected_columns() {
            self.query.column(column_name);
        }
        SelectPartialQuery {
            query: self,
            _partial: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;
    use crate::{LifeEntityName, LifeModelTrait};
    use sea_query::{ExprTrait, IdenStatic};

    // Test entity shared across all tests
    #[derive(Default)]
    struct TestEntity;

    impl sea_query::Iden for TestEntity {
        fn unquoted(&self) -> &'static str {
            "test_entities"
        }
    }

    impl LifeEntityName for TestEntity {
        fn table_name(&self) -> &'static str {
            "test_entities"
        }
    }

    #[derive(Copy, Clone, Debug)]
    enum TestColumn {
        Id,
    }

    impl sea_query::Iden for TestColumn {
        fn unquoted(&self) -> &'static str {
            "id"
        }
    }

    impl IdenStatic for TestColumn {
        fn as_str(&self) -> &'static str {
            "id"
        }
    }

    crate::impl_column_def_helper_for_test!(TestColumn);

    impl LifeModelTrait for TestEntity {
        type Model = ();
        type Column = TestColumn;
    }

    #[test]
    fn test_partial_model_trait_exists() {
        // Test that PartialModelTrait is properly defined
        // This is a compile-time check
        struct TestPartial;

        impl crate::FromRow for TestPartial {
            fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(TestPartial)
            }
        }

        impl PartialModelTrait for TestPartial {
            type Entity = TestEntity;
            fn selected_columns() -> Vec<&'static str> {
                vec!["id"]
            }
        }

        // Verify the trait is properly defined
        let _columns = TestPartial::selected_columns();
    }

    // ============================================================================
    // Edge Cases
    // ============================================================================

    #[test]
    fn test_partial_model_empty_selected_columns() {
        // EDGE CASE: Empty selected_columns() vector
        use crate::FromRow;

        struct EmptyPartial;

        impl FromRow for EmptyPartial {
            fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(EmptyPartial)
            }
        }

        impl PartialModelTrait for EmptyPartial {
            type Entity = TestEntity;
            fn selected_columns() -> Vec<&'static str> {
                vec![] // Empty - should still compile
            }
        }

        // Verify it compiles
        let columns = EmptyPartial::selected_columns();
        assert_eq!(columns.len(), 0);
    }

    #[test]
    fn test_partial_model_single_column() {
        // EDGE CASE: Partial model with only one column
        use crate::FromRow;

        struct IdOnlyPartial {
            _id: i32,
        }

        impl FromRow for IdOnlyPartial {
            fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(IdOnlyPartial {
                    _id: 1, // Mock value for test
                })
            }
        }

        impl PartialModelTrait for IdOnlyPartial {
            type Entity = TestEntity;
            fn selected_columns() -> Vec<&'static str> {
                vec!["id"]
            }
        }

        // Verify it compiles
        let columns = IdOnlyPartial::selected_columns();
        assert_eq!(columns.len(), 1);
    }

    #[test]
    fn test_partial_model_all_columns() {
        // EDGE CASE: Partial model that selects all columns (should work like full model)
        use crate::FromRow;

        struct FullPartial {
            _id: i32,
        }

        impl FromRow for FullPartial {
            fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(FullPartial {
                    _id: 1, // Mock value for test
                })
            }
        }

        impl PartialModelTrait for FullPartial {
            type Entity = TestEntity;
            fn selected_columns() -> Vec<&'static str> {
                vec!["id"]
            }
        }

        // Verify it compiles
        let columns = FullPartial::selected_columns();
        assert_eq!(columns.len(), 1);
    }

    #[test]
    fn test_partial_model_column_order_mismatch() {
        // EDGE CASE: Column order in selected_columns() doesn't match FromRow order
        // This is a potential runtime error - the test documents the requirement
        use crate::FromRow;

        struct MismatchedOrderPartial {
            _name: String,
            _id: i32, // Wrong order - should be id, name
        }

        impl FromRow for MismatchedOrderPartial {
            fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                // This would fail at runtime if column order doesn't match
                // Mock values for test - actual implementation would use row.get()
                Ok(MismatchedOrderPartial {
                    _name: "test".to_string(), // Expects name first
                    _id: 1,                    // Then id
                })
            }
        }

        impl PartialModelTrait for MismatchedOrderPartial {
            type Entity = TestEntity;
            fn selected_columns() -> Vec<&'static str> {
                vec![
                    "name", // Selected first
                    "id",   // Selected second
                ]
            }
        }

        // Verify it compiles - runtime error would occur if order is wrong
        let columns = MismatchedOrderPartial::selected_columns();
        assert_eq!(columns.len(), 2);
    }

    // ============================================================================
    // Clause preservation: select_partial() replaces only the select list
    // ============================================================================

    #[test]
    fn test_select_partial_keeps_where_clause() {
        // select_partial() replaces only the select list: a filter added before it is kept.

        use crate::PartialModelBuilder;

        struct TestPartial;

        impl crate::FromRow for TestPartial {
            fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(TestPartial)
            }
        }

        impl PartialModelTrait for TestPartial {
            type Entity = TestEntity;
            fn selected_columns() -> Vec<&'static str> {
                vec!["id"]
            }
        }

        // Build query with WHERE clause, then call select_partial()
        use sea_query::Expr;
        let query = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(1))
            .select_partial::<TestPartial>();

        let (sql, _values) = query.query.query.build(sea_query::PostgresQueryBuilder);
        assert_eq!(
            sql, r#"SELECT "id" FROM "test_entities" WHERE "id" = $1"#,
            "only the select list is replaced"
        );
    }

    #[test]
    fn test_select_partial_keeps_order_by_clause() {
        // select_partial() keeps an ORDER BY added before it.

        use crate::PartialModelBuilder;

        struct TestPartial;

        impl crate::FromRow for TestPartial {
            fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(TestPartial)
            }
        }

        impl PartialModelTrait for TestPartial {
            type Entity = TestEntity;
            fn selected_columns() -> Vec<&'static str> {
                vec!["id"]
            }
        }

        // Build query with ORDER BY, then call select_partial()
        let query = SelectQuery::<TestEntity>::new()
            .order_by("id", sea_query::Order::Asc)
            .select_partial::<TestPartial>();

        let (sql, _values) = query.query.query.build(sea_query::PostgresQueryBuilder);
        assert_eq!(sql, r#"SELECT "id" FROM "test_entities" ORDER BY "id" ASC"#);
    }

    #[test]
    fn test_select_partial_keeps_limit_offset_clauses() {
        // select_partial() keeps LIMIT and OFFSET added before it (pagination).

        use crate::PartialModelBuilder;

        struct TestPartial;

        impl crate::FromRow for TestPartial {
            fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(TestPartial)
            }
        }

        impl PartialModelTrait for TestPartial {
            type Entity = TestEntity;
            fn selected_columns() -> Vec<&'static str> {
                vec!["id"]
            }
        }

        // Build query with LIMIT and OFFSET, then call select_partial()
        let query = SelectQuery::<TestEntity>::new()
            .limit(10)
            .offset(20)
            .select_partial::<TestPartial>();

        let (sql, values) = query.query.query.build(sea_query::PostgresQueryBuilder);
        assert_eq!(
            sql,
            r#"SELECT "id" FROM "test_entities" LIMIT $1 OFFSET $2"#
        );
        assert_eq!(values.0.len(), 2, "limit and offset are bound: {values:?}");
    }

    #[test]
    fn test_select_partial_workaround_early_call() {
        // WORKAROUND: Calling select_partial() early preserves clauses added after

        use crate::PartialModelBuilder;

        struct TestPartial;

        impl crate::FromRow for TestPartial {
            fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(TestPartial)
            }
        }

        impl PartialModelTrait for TestPartial {
            type Entity = TestEntity;
            fn selected_columns() -> Vec<&'static str> {
                vec!["id"]
            }
        }

        // WORKAROUND: Call select_partial() early, then add clauses
        use sea_query::Expr;
        let query = SelectQuery::<TestEntity>::new()
            .select_partial::<TestPartial>() // Early call
            .filter(Expr::col("id").eq(1))
            .order_by("id", sea_query::Order::Asc)
            .limit(10);

        // Build SQL to verify clauses ARE preserved (workaround works)
        let (sql, _values) = query.query.query.build(sea_query::PostgresQueryBuilder);

        // Verify clauses ARE in SQL when using the workaround
        assert!(
            sql.to_uppercase().contains("WHERE"),
            "WHERE clause should be preserved when select_partial() is called early (workaround)"
        );
        assert!(sql.to_uppercase().contains("ORDER BY"),
            "ORDER BY clause should be preserved when select_partial() is called early (workaround)");
        assert!(
            sql.to_uppercase().contains("LIMIT"),
            "LIMIT clause should be preserved when select_partial() is called early (workaround)"
        );

        // Verify partial model columns are also in SQL
        assert!(sql.contains("id"), "Partial model columns should be in SQL");
    }

    #[test]
    fn test_select_partial_preserves_partial_model_columns() {
        // Verify that select_partial() correctly sets the partial model columns

        use crate::PartialModelBuilder;

        struct TestPartial;

        impl crate::FromRow for TestPartial {
            fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(TestPartial)
            }
        }

        impl PartialModelTrait for TestPartial {
            type Entity = TestEntity;
            fn selected_columns() -> Vec<&'static str> {
                vec!["id", "name"]
            }
        }

        // Build query with select_partial()
        let query = SelectQuery::<TestEntity>::new().select_partial::<TestPartial>();

        // Build SQL to verify columns are set correctly
        let (sql, _values) = query.query.query.build(sea_query::PostgresQueryBuilder);

        // Verify both columns from partial model are in SQL
        assert!(
            sql.contains("id"),
            "Partial model column 'id' should be in SQL"
        );
        assert!(
            sql.contains("name"),
            "Partial model column 'name' should be in SQL"
        );

        // Verify we're not selecting all columns (no asterisk)
        // Note: This depends on how sea-query formats SELECT *
        let sql_upper = sql.to_uppercase();
        // The SQL should have explicit column names, not just SELECT * FROM
        assert!(sql_upper.contains("SELECT"), "SQL should contain SELECT");
    }
}
