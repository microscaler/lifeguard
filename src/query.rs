//! Query builder for LifeModel - Epic 02 Story 04
//!
//! Provides a query builder that works with SeaQuery to build SQL queries.
//! Supports SELECT operations with filtering, ordering, pagination, and grouping.

use crate::executor::{LifeExecutor, LifeError};
use may_postgres::Row;
use sea_query::{SelectStatement, PostgresQueryBuilder, Iden, Expr, Order, IntoColumnRef};
use std::marker::PhantomData;

/// Query builder for selecting records
///
/// This is returned by `LifeModel::find()` and can be chained with filters,
/// ordering, pagination, and grouping.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{SelectQuery, FromRow, LifeExecutor};
/// use sea_query::{Expr, Order};
///
/// # struct UserModel { id: i32, name: String };
/// # impl FromRow for UserModel {
/// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
/// #         todo!()
/// #     }
/// # }
/// # let executor: &dyn LifeExecutor = todo!();
///
/// // Find users with name starting with "John", ordered by id, limit 10
/// let users = UserModel::find()
///     .filter(Expr::col("name").like("John%"))
///     .order_by("id", Order::Asc)
///     .limit(10)
///     .all(executor)?;
/// ```
pub struct SelectQuery<M> {
    query: SelectStatement,
    _phantom: PhantomData<M>,
}

impl<M> SelectQuery<M>
where
    M: FromRow,
{
    /// Create a new select query
    pub fn new(table_name: &'static str) -> Self {
        struct TableName(&'static str);
        impl Iden for TableName {
            fn unquoted(&self) -> &str {
                self.0
            }
        }
        
        let mut query = SelectStatement::default();
        query.column(sea_query::Asterisk).from(TableName(table_name));
        Self {
            query,
            _phantom: PhantomData,
        }
    }
    
    /// Add a filter condition
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::SelectQuery;
    /// use sea_query::Expr;
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # let query = UserModel::find();
    /// let filtered = query.filter(Expr::col("id").eq(1));
    /// ```
    pub fn filter(mut self, condition: Expr) -> Self {
        self.query.and_where(condition);
        self
    }
    
    /// Add an ORDER BY clause
    ///
    /// # Arguments
    ///
    /// * `column` - Column name or expression to order by
    /// * `order` - Order direction (`Order::Asc` or `Order::Desc`)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::SelectQuery;
    /// use sea_query::{Expr, Order};
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # let query = UserModel::find();
    /// let ordered = query.order_by("id", Order::Desc);
    /// ```
    pub fn order_by<C: IntoColumnRef>(mut self, column: C, order: Order) -> Self {
        self.query.order_by(column, order);
        self
    }
    
    /// Add a LIMIT clause
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of rows to return
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::SelectQuery;
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # let query = UserModel::find();
    /// let limited = query.limit(10);
    /// ```
    pub fn limit(mut self, limit: u64) -> Self {
        self.query.limit(limit);
        self
    }
    
    /// Add an OFFSET clause
    ///
    /// # Arguments
    ///
    /// * `offset` - Number of rows to skip
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::SelectQuery;
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # let query = UserModel::find();
    /// let offset = query.offset(20);
    /// ```
    pub fn offset(mut self, offset: u64) -> Self {
        self.query.offset(offset);
        self
    }
    
    /// Add a GROUP BY clause
    ///
    /// # Arguments
    ///
    /// * `column` - Column name or expression to group by
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::SelectQuery;
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # let query = UserModel::find();
    /// let grouped = query.group_by("status");
    /// ```
    pub fn group_by<C: IntoColumnRef>(mut self, column: C) -> Self {
        self.query.group_by_col(column);
        self
    }
    
    /// Add a HAVING clause (for use with GROUP BY)
    ///
    /// # Arguments
    ///
    /// * `condition` - Expression to filter grouped results
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::SelectQuery;
    /// use sea_query::Expr;
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # let query = UserModel::find();
    /// let having = query.group_by("status").having(Expr::col("COUNT(*)").gt(5));
    /// ```
    pub fn having(mut self, condition: Expr) -> Self {
        self.query.and_having(condition);
        self
    }
    
    /// Execute the query and return all results
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{SelectQuery, LifeExecutor};
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # let executor: &dyn LifeExecutor = todo!();
    /// let users = UserModel::find().all(executor)?;
    /// ```
    pub fn all<E: LifeExecutor>(self, executor: &E) -> Result<Vec<M>, LifeError> {
        let (sql, _values) = self.query.build(PostgresQueryBuilder);
        let rows = executor.query_all(&sql, &[])?;
        
        let mut results = Vec::new();
        for row in rows {
            let model = M::from_row(&row)
                .map_err(|e| LifeError::ParseError(format!("Failed to parse row: {}", e)))?;
            results.push(model);
        }
        Ok(results)
    }
    
    /// Execute the query and return a single result
    ///
    /// Returns an error if zero or more than one row is returned.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{SelectQuery, LifeExecutor};
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # let executor: &dyn LifeExecutor = todo!();
    /// let user = UserModel::find().one(executor)?;
    /// ```
    pub fn one<E: LifeExecutor>(self, executor: &E) -> Result<M, LifeError> {
        let (sql, _values) = self.query.build(PostgresQueryBuilder);
        let row = executor.query_one(&sql, &[])?;
        M::from_row(&row).map_err(|e| LifeError::ParseError(format!("Failed to parse row: {}", e)))
    }
}

/// Trait for types that can be created from a database row
pub trait FromRow: Sized {
    fn from_row(row: &Row) -> Result<Self, may_postgres::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_query::{Expr, Order, ExprTrait};

    // Test model for query builder tests
    #[derive(Debug, Clone)]
    struct TestModel {
        id: i32,
        name: String,
    }

    impl FromRow for TestModel {
        fn from_row(_row: &Row) -> Result<Self, may_postgres::Error> {
            // Mock implementation - not used in query building tests
            Ok(TestModel {
                id: 1,
                name: "Test".to_string(),
            })
        }
    }

    #[test]
    fn test_query_builder_creation() {
        let _query = SelectQuery::<TestModel>::new("test_table");
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_filter() {
        let _query = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("id").eq(1));
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_order_by() {
        let _query = SelectQuery::<TestModel>::new("test_table")
            .order_by("id", Order::Asc);
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_limit() {
        let _query = SelectQuery::<TestModel>::new("test_table")
            .limit(10);
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_offset() {
        let _query = SelectQuery::<TestModel>::new("test_table")
            .offset(20);
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_group_by() {
        let _query = SelectQuery::<TestModel>::new("test_table")
            .group_by("status");
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_having() {
        let _query = SelectQuery::<TestModel>::new("test_table")
            .group_by("status")
            .having(Expr::col("COUNT(*)").gt(5));
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_chaining() {
        let _query = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("id").gt(10))
            .order_by("name", Order::Asc)
            .limit(5)
            .offset(10);
        // Test passes if it compiles - demonstrates method chaining
    }

    #[test]
    fn test_query_builder_complex() {
        let _query = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("status").eq("active"))
            .filter(Expr::col("created_at").gte(Expr::cust("NOW() - INTERVAL '30 days'")))
            .group_by("category")
            .having(Expr::col("COUNT(*)").gt(1))
            .order_by("category", Order::Asc)
            .order_by("name", Order::Desc)
            .limit(100)
            .offset(0);
        // Test passes if it compiles - demonstrates complex query building
    }

    #[test]
    fn test_query_builder_multiple_filters() {
        let _query = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("id").gt(1))
            .filter(Expr::col("id").lt(100))
            .filter(Expr::col("name").like("John%"));
        // Test passes if it compiles - demonstrates multiple WHERE conditions
    }

    #[test]
    fn test_query_builder_multiple_order_by() {
        let _query = SelectQuery::<TestModel>::new("test_table")
            .order_by("status", Order::Asc)
            .order_by("created_at", Order::Desc)
            .order_by("id", Order::Asc);
        // Test passes if it compiles - demonstrates multiple ORDER BY clauses
    }
}
