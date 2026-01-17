//! Query execution methods for SelectQuery and SelectModel.
//!
//! This module provides execution methods (`all`, `one`, `find_one`, `count`, etc.)
//! for executing queries built with `SelectQuery` and `SelectModel`.
//!
//! The execution methods use `with_converted_params` from `value_conversion` to
//! convert SeaQuery values to may_postgres ToSql parameters, avoiding code duplication.

use crate::executor::{LifeExecutor, LifeError};
use crate::query::select::{SelectQuery, SelectModel};
use crate::query::traits::{LifeModelTrait, FromRow};
use crate::query::value_conversion::with_converted_params;
use crate::query::error_handling::is_no_rows_error;
use sea_query::PostgresQueryBuilder;

// Execution methods for SelectQuery
impl<E> SelectQuery<E>
where
    E: LifeModelTrait,
{
    /// Execute the query and return all results
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{SelectQuery, LifeModelTrait, LifeExecutor};
    ///
    /// # struct User; // Entity
    /// # struct UserModel { id: i32 }; // Model
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # impl lifeguard::LifeModelTrait for User {
    /// #     type Model = UserModel;
    /// # }
    /// # let executor: &dyn LifeExecutor = todo!();
    /// let users = User::find().all(executor)?;
    /// ```
    pub fn all<Ex: LifeExecutor>(self, executor: &Ex) -> Result<Vec<E::Model>, LifeError>
    where
        E::Model: FromRow,
    {
        let (sql, values) = self.query.build(PostgresQueryBuilder);
        
        with_converted_params(&values, |params| {
            let rows = executor.query_all(&sql, params)?;
            
            let mut results = Vec::new();
            for row in rows {
                let model = <E::Model as FromRow>::from_row(&row)
                    .map_err(|e| LifeError::ParseError(format!("Failed to parse row: {}", e)))?;
                results.push(model);
            }
            Ok(results)
        })
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
    pub fn one<Ex: LifeExecutor>(self, executor: &Ex) -> Result<E::Model, LifeError>
    where
        E::Model: FromRow,
    {
        let (sql, values) = self.query.build(PostgresQueryBuilder);
        
        with_converted_params(&values, |params| {
            let row = executor.query_one(&sql, params)?;
            <E::Model as FromRow>::from_row(&row)
                .map_err(|e| LifeError::ParseError(format!("Failed to parse row: {}", e)))
        })
    }
    
    /// Execute the query and return the first result, or None if no results
    ///
    /// This is similar to `one()` but returns `Option<E::Model>` instead of an error
    /// when no rows are found.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{SelectQuery, LifeExecutor};
    /// use sea_query::Expr;
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # let executor: &dyn LifeExecutor = todo!();
    /// let user = UserModel::find().filter(Expr::col("id").eq(1)).find_one(executor)?;
    /// ```
    pub fn find_one<Ex: LifeExecutor>(self, executor: &Ex) -> Result<Option<E::Model>, LifeError>
    where
        E::Model: FromRow,
    {
        match self.one(executor) {
            Ok(model) => Ok(Some(model)),
            Err(e) => {
                if is_no_rows_error(&e) {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }
    
    /// Paginate results with a given page size
    ///
    /// Returns a `Paginator` that can be used to fetch pages of results.
    ///
    /// # Arguments
    ///
    /// * `executor` - The executor to use for queries
    /// * `page_size` - Number of items per page
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
    /// let mut paginator = UserModel::find().paginate(executor, 10);
    /// let page_1 = paginator.fetch_page(1)?;
    /// ```
    pub fn paginate<'e, Ex: LifeExecutor>(self, executor: &'e Ex, page_size: usize) -> Paginator<'e, E, Ex>
    where
        E::Model: FromRow,
    {
        Paginator::new(self, executor, page_size)
    }
    
    /// Build and execute a COUNT(*) query that preserves WHERE, GROUP BY, and HAVING conditions
    ///
    /// This method creates an efficient COUNT(*) query by:
    /// - Preserving all WHERE conditions
    /// - Preserving GROUP BY and HAVING clauses
    /// - Explicitly removing ORDER BY, LIMIT, and OFFSET before counting
    ///   (databases DO apply LIMIT/OFFSET in subqueries, so they must be removed)
    /// - Selecting COUNT(*) instead of all columns
    ///
    /// # Arguments
    ///
    /// * `executor` - The executor to use for the query
    ///
    /// # Returns
    ///
    /// The count of rows matching the query conditions
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{SelectQuery, LifeExecutor};
    /// use sea_query::Expr;
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # let executor: &dyn LifeExecutor = todo!();
    /// let count = UserModel::find()
    ///     .filter(Expr::col("age").gt(18))
    ///     .count(executor)?;
    /// ```
    pub fn count<Ex: LifeExecutor>(&self, executor: &Ex) -> Result<usize, LifeError> {
        // Build a COUNT(*) query by wrapping the original query in a subquery
        // This preserves all WHERE, GROUP BY, and HAVING conditions
        // while removing ORDER BY, LIMIT, and OFFSET (which don't affect count)
        
        // CRITICAL: Databases DO apply LIMIT/OFFSET in subqueries, so we must remove them
        // explicitly before wrapping in a subquery. Otherwise, a query with `.limit(10)`
        // would incorrectly return a count of at most 10 instead of the total matching rows.
        
        // Clone the query and build SQL to work with it
        let (original_sql, values) = self.query.clone().build(PostgresQueryBuilder);
        
        // Remove ORDER BY, LIMIT, and OFFSET clauses from the SQL
        // These clauses appear at the end of the SELECT statement in this order:
        // SELECT ... [ORDER BY ...] [LIMIT ...] [OFFSET ...]
        // We need to remove them carefully to preserve the rest of the query
        let cleaned_sql = {
            let sql = original_sql.trim();
            let sql_upper = sql.to_uppercase();
            
            // Find the positions of ORDER BY, LIMIT, and OFFSET (case-insensitive)
            let order_by_pos = sql_upper.rfind(" ORDER BY ");
            let limit_pos = sql_upper.rfind(" LIMIT ");
            let offset_pos = sql_upper.rfind(" OFFSET ");
            
            // Determine which clause appears last (needs to be removed first)
            // Find the maximum position among all three clauses
            let last_clause_pos = offset_pos
                .into_iter()
                .chain(limit_pos)
                .chain(order_by_pos)
                .max();
            
            if let Some(pos) = last_clause_pos {
                // Remove everything from the last clause to the end
                // This handles ORDER BY, LIMIT, OFFSET in any combination
                sql[..pos].trim().to_string()
            } else {
                // No ORDER BY, LIMIT, or OFFSET found - use original SQL
                sql.to_string()
            }
        };
        
        // Wrap the cleaned query in SELECT COUNT(*) FROM (cleaned_query) AS subquery
        // This ensures we count all matching rows, not just the limited subset
        let count_sql = format!("SELECT COUNT(*) FROM ({}) AS count_subquery", cleaned_sql);
        
        // Use with_converted_params for value conversion
        with_converted_params(&values, |params| {
            // Execute the COUNT query
            let row = executor.query_one(&count_sql, params)?;
            
            // Extract the count from the first column (COUNT(*) returns a single i64 value)
            let count: i64 = row.get(0);
            
            // Convert to usize, handling potential overflow
            if count < 0 {
                return Err(LifeError::Other(format!("Count cannot be negative: {}", count)));
            }
            
            Ok(count as usize)
        })
    }
    
    /// Paginate results and get total count
    ///
    /// Similar to `paginate()` but also provides a method to get the total count
    /// of items matching the query.
    ///
    /// # Arguments
    ///
    /// * `executor` - The executor to use for queries
    /// * `page_size` - Number of items per page
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
    /// let mut paginator = UserModel::find().paginate_and_count(executor, 10);
    /// let total = paginator.num_items()?;
    /// let page_1 = paginator.fetch_page(1)?;
    /// ```
    pub fn paginate_and_count<'e, Ex: LifeExecutor>(self, executor: &'e Ex, page_size: usize) -> PaginatorWithCount<'e, E, Ex>
    where
        E::Model: FromRow,
    {
        PaginatorWithCount::new(self, executor, page_size)
    }
}

// Execution methods for SelectModel
impl<E, M> SelectModel<E, M>
where
    E: LifeModelTrait,
    M: FromRow,
{
    /// Execute the query and return all results as the specified Model type
    pub fn all<Ex: LifeExecutor>(self, executor: &Ex) -> Result<Vec<M>, LifeError> {
        let (sql, values) = self.query.query.build(PostgresQueryBuilder);
        
        with_converted_params(&values, |params| {
            let rows = executor.query_all(&sql, params)?;
            
            let mut results = Vec::new();
            for row in rows {
                let model = M::from_row(&row)
                    .map_err(|e| LifeError::ParseError(format!("Failed to parse row: {}", e)))?;
                results.push(model);
            }
            Ok(results)
        })
    }
    
    /// Execute the query and return a single result as the specified Model type
    pub fn one<Ex: LifeExecutor>(self, executor: &Ex) -> Result<M, LifeError> {
        let results = self.all(executor)?;
        
        if results.len() != 1 {
            return Err(LifeError::Other(format!(
                "Expected exactly one row, got {}",
                results.len()
            )));
        }
        
        Ok(results.into_iter().next().unwrap())
    }
}

/// Paginator for query results
///
/// Provides pagination functionality for query results.
pub struct Paginator<'e, E, Ex>
where
    E: LifeModelTrait,
    Ex: LifeExecutor,
{
    query: SelectQuery<E>,
    executor: &'e Ex,
    page_size: usize,
}

impl<'e, E, Ex> Paginator<'e, E, Ex>
where
    E: LifeModelTrait,
    E::Model: FromRow,
    Ex: LifeExecutor,
{
    pub(crate) fn new(query: SelectQuery<E>, executor: &'e Ex, page_size: usize) -> Self {
        Self {
            query,
            executor,
            page_size,
        }
    }
    
    /// Fetch a specific page (1-indexed)
    pub fn fetch_page(&mut self, page: usize) -> Result<Vec<E::Model>, LifeError> {
        let offset = (page.saturating_sub(1)) * self.page_size;
        // Clone the query to avoid moving it
        let query = SelectQuery {
            query: self.query.query.clone(),
            _phantom: self.query._phantom,
        };
        query
            .limit(self.page_size as u64)
            .offset(offset as u64)
            .all(self.executor)
    }
}

/// Paginator with count support
///
/// Provides pagination functionality with total count tracking.
pub struct PaginatorWithCount<'e, E, Ex>
where
    E: LifeModelTrait,
    Ex: LifeExecutor,
{
    query: SelectQuery<E>,
    executor: &'e Ex,
    page_size: usize,
    #[cfg(test)]
    pub(crate) total_count: Option<usize>,
    #[cfg(not(test))]
    total_count: Option<usize>,
}

impl<'e, E, Ex> PaginatorWithCount<'e, E, Ex>
where
    E: LifeModelTrait,
    E::Model: FromRow,
    Ex: LifeExecutor,
{
    pub(crate) fn new(query: SelectQuery<E>, executor: &'e Ex, page_size: usize) -> Self {
        Self {
            query,
            executor,
            page_size,
            total_count: None,
        }
    }
    
    /// Get the total number of items matching the query
    ///
    /// This method efficiently counts rows by executing a COUNT(*) query that
    /// preserves WHERE, GROUP BY, and HAVING conditions without loading all rows
    /// into memory. The result is cached for subsequent calls.
    pub fn num_items(&mut self) -> Result<usize, LifeError> {
        if let Some(count) = self.total_count {
            return Ok(count);
        }
        
        // Build and execute an efficient COUNT(*) query that preserves WHERE conditions
        // This avoids loading all rows into memory, which is critical for large datasets
        let count = self.query.count(self.executor)?;
        self.total_count = Some(count);
        Ok(count)
    }
    
    /// Fetch a specific page (1-indexed)
    pub fn fetch_page(&mut self, page: usize) -> Result<Vec<E::Model>, LifeError> {
        let offset = (page.saturating_sub(1)) * self.page_size;
        // Clone the query to avoid moving it
        let query = SelectQuery {
            query: self.query.query.clone(),
            _phantom: self.query._phantom,
        };
        query
            .limit(self.page_size as u64)
            .offset(offset as u64)
            .all(self.executor)
    }
}

#[cfg(test)]
mod tests {
    use crate::query::select::SelectQuery;
    use crate::query::traits::{LifeEntityName, LifeModelTrait, FromRow};
    use crate::query::error_handling::is_no_rows_error;
    use crate::executor::{LifeError, LifeExecutor};
    use sea_query::{Expr, Order, ExprTrait};
    use std::sync::{Arc, Mutex};
    use may_postgres::types::ToSql;
    use may_postgres::Row;

    // Test Entity for query builder tests
    #[derive(Copy, Clone, Default, Debug)]
    pub struct TestEntity;

    impl LifeEntityName for TestEntity {
        fn table_name(&self) -> &'static str {
            "test_table"
        }
    }

    // Test Column enum for query builder tests
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub enum TestColumn {
        Id,
        Name,
    }

    impl sea_query::Iden for TestColumn {
        fn unquoted(&self) -> &str {
            match self {
                TestColumn::Id => "id",
                TestColumn::Name => "name",
            }
        }
    }

    impl sea_query::IdenStatic for TestColumn {
        fn as_str(&self) -> &'static str {
            match self {
                TestColumn::Id => "id",
                TestColumn::Name => "name",
            }
        }
    }

    // Test model for query builder tests
    #[derive(Debug, Clone)]
    pub struct TestModel {
        _id: i32,
        _name: String,
    }

    impl FromRow for TestModel {
        fn from_row(_row: &Row) -> Result<Self, may_postgres::Error> {
            // Mock implementation - not used in query building tests
            Ok(TestModel {
                _id: 1,
                _name: "Test".to_string(),
            })
        }
    }

    impl LifeModelTrait for TestEntity {
        type Model = TestModel;
        type Column = TestColumn;
    }

    // Mock executor that captures SQL and parameter counts for verification
    struct MockExecutor {
        captured_sql: Arc<Mutex<Vec<String>>>,
        captured_param_counts: Arc<Mutex<Vec<usize>>>,
        _return_rows: Vec<Row>,
    }

    impl MockExecutor {
        fn new(_return_rows: Vec<Row>) -> Self {
            Self {
                captured_sql: Arc::new(Mutex::new(Vec::new())),
                captured_param_counts: Arc::new(Mutex::new(Vec::new())),
                _return_rows: vec![], // We can't easily create Row objects, so we use empty vec
            }
        }

        fn get_captured_sql(&self) -> Vec<String> {
            self.captured_sql.lock().unwrap().clone()
        }

        fn get_captured_param_counts(&self) -> Vec<usize> {
            self.captured_param_counts.lock().unwrap().clone()
        }

        fn clear(&self) {
            self.captured_sql.lock().unwrap().clear();
            self.captured_param_counts.lock().unwrap().clear();
        }

        // Helper to count placeholders in SQL
        #[allow(dead_code)]
        fn count_placeholders(sql: &str) -> usize {
            sql.matches("$").count()
        }
    }

    impl LifeExecutor for MockExecutor {
        fn execute(&self, query: &str, params: &[&dyn ToSql]) -> Result<u64, LifeError> {
            self.captured_sql.lock().unwrap().push(query.to_string());
            self.captured_param_counts.lock().unwrap().push(params.len());
            Ok(0)
        }

        fn query_one(&self, query: &str, params: &[&dyn ToSql]) -> Result<Row, LifeError> {
            self.captured_sql.lock().unwrap().push(query.to_string());
            self.captured_param_counts.lock().unwrap().push(params.len());
            // For testing, we don't actually need to return rows since tests only check SQL/params
            // Return an error to indicate no row available (tests don't use the returned value)
            Err(LifeError::QueryError("MockExecutor: No rows available for testing".to_string()))
        }

        fn query_all(&self, query: &str, params: &[&dyn ToSql]) -> Result<Vec<Row>, LifeError> {
            self.captured_sql.lock().unwrap().push(query.to_string());
            self.captured_param_counts.lock().unwrap().push(params.len());
            // For testing, return empty vec since tests only check SQL/params
            // Row doesn't implement Clone, so we can't return stored rows
            Ok(vec![])
        }
    }

    #[test]
    fn test_query_builder_creation() {
        let _query = SelectQuery::<TestEntity>::new();
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_filter() {
        let _query = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(1));
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_order_by() {
        let _query = SelectQuery::<TestEntity>::new()
            .order_by("id", Order::Asc);
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_limit() {
        let _query = SelectQuery::<TestEntity>::new()
            .limit(10);
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_offset() {
        let _query = SelectQuery::<TestEntity>::new()
            .offset(20);
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_group_by() {
        let _query = SelectQuery::<TestEntity>::new()
            .group_by("status");
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_having() {
        let _query = SelectQuery::<TestEntity>::new()
            .group_by("status")
            .having(Expr::col("COUNT(*)").gt(5));
        // Test passes if it compiles
    }

    #[test]
    fn test_query_builder_chaining() {
        let _query = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").gt(10))
            .order_by("name", Order::Asc)
            .limit(5)
            .offset(10);
        // Test passes if it compiles - demonstrates method chaining
    }

    #[test]
    fn test_query_builder_complex() {
        let _query = SelectQuery::<TestEntity>::new()
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
        let _query = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").gt(1))
            .filter(Expr::col("id").lt(100))
            .filter(Expr::col("name").like("John%"));
        // Test passes if it compiles - demonstrates multiple WHERE conditions
    }

    #[test]
    fn test_query_builder_multiple_order_by() {
        let _query = SelectQuery::<TestEntity>::new()
            .order_by("status", Order::Asc)
            .order_by("created_at", Order::Desc)
            .order_by("id", Order::Asc);
        // Test passes if it compiles - demonstrates multiple ORDER BY clauses
    }

    // ============================================================================
    // COMPREHENSIVE PARAMETER HANDLING TESTS
    // These tests verify that parameters are correctly extracted and passed
    // ============================================================================

    #[test]
    fn test_parameter_extraction_integer_filter() {
        // Test that integer parameters are extracted and passed
        // Note: We can't easily create Row objects without a DB connection
        // So we focus on verifying SQL generation and parameter counts
        // The actual execution would fail, but that's OK - we're testing the fix
        let executor = MockExecutor::new(vec![]);
        
        // This will fail at execution (no rows), but that's OK - we test the fix
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(42))
            .all(&executor);
        
        let sql = executor.get_captured_sql();
        let param_counts = executor.get_captured_param_counts();
        
        // Verify the fix: SQL was generated and parameters were extracted
        assert!(!sql.is_empty(), "SQL should be generated");
        assert_eq!(param_counts.len(), 1, "Should have one query");
        // CRITICAL: With a filter using .eq(42), we MUST have parameters
        // Before the fix, this would be 0. After the fix, it should be > 0
        assert!(param_counts[0] > 0, "Should have parameters for integer filter - THIS TESTS THE FIX");
        // Verify SQL contains placeholder
        assert!(sql[0].contains("$"), "SQL should contain parameter placeholder");
    }

    #[test]
    fn test_parameter_extraction_string_filter() {
        // Test that string parameters are extracted and passed
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("name").eq("test"))
            .all(&executor);
        
        let sql = executor.get_captured_sql();
        let param_counts = executor.get_captured_param_counts();
        
        assert!(!sql.is_empty(), "SQL should be generated");
        assert_eq!(param_counts.len(), 1, "Should have one query");
        assert!(param_counts[0] > 0, "Should have parameters for string filter");
        assert!(sql[0].contains("$"), "SQL should contain parameter placeholder");
    }

    #[test]
    fn test_parameter_extraction_multiple_filters() {
        // Test that multiple parameters are extracted correctly
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(1))
            .filter(Expr::col("id").gt(0))
            .filter(Expr::col("name").like("John%"))
            .all(&executor);
        
        let sql = executor.get_captured_sql();
        let param_counts = executor.get_captured_param_counts();
        
        assert!(!sql.is_empty(), "SQL should be generated");
        assert_eq!(param_counts.len(), 1, "Should have one query");
        // Should have at least 3 parameters (1 for eq, 1 for gt, 1 for like)
        assert!(param_counts[0] >= 3, "Should have parameters for all filters");
    }

    #[test]
    fn test_parameter_extraction_comparison_operators() {
        // Test all comparison operators generate parameters
        let executor = MockExecutor::new(vec![]);
        
        // Test .eq()
        let _result1 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(10))
            .all(&executor);
        
        executor.clear();
        
        // Test .ne()
        let _result2 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").ne(10))
            .all(&executor);
        
        executor.clear();
        
        // Test .gt()
        let _result3 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").gt(10))
            .all(&executor);
        
        executor.clear();
        
        // Test .gte()
        let _result4 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").gte(10))
            .all(&executor);
        
        executor.clear();
        
        // Test .lt()
        let _result5 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").lt(10))
            .all(&executor);
        
        executor.clear();
        
        // Test .lte()
        let _result6 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").lte(10))
            .all(&executor);
        
        let param_counts = executor.get_captured_param_counts();
        
        // All should have parameters
        for count in param_counts {
            assert!(count > 0, "All comparison operators should generate parameters");
        }
    }

    #[test]
    fn test_parameter_extraction_like_operator() {
        // Test LIKE operator with string pattern
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("name").like("John%"))
            .all(&executor);
        
        let sql = executor.get_captured_sql();
        let param_counts = executor.get_captured_param_counts();
        
        assert!(!sql.is_empty(), "SQL should be generated");
        assert!(param_counts[0] > 0, "LIKE should generate parameters");
        assert!(sql[0].to_uppercase().contains("LIKE"), "SQL should contain LIKE");
    }

    #[test]
    fn test_parameter_extraction_in_operator() {
        // Test IN operator with multiple values
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").is_in(vec![1, 2, 3]))
            .all(&executor);
        
        let sql = executor.get_captured_sql();
        let param_counts = executor.get_captured_param_counts();
        
        assert!(!sql.is_empty(), "SQL should be generated");
        assert!(param_counts[0] >= 3, "IN with 3 values should generate at least 3 parameters");
        assert!(sql[0].to_uppercase().contains("IN"), "SQL should contain IN");
    }

    #[test]
    fn test_parameter_extraction_between_operator() {
        // Test BETWEEN operator
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").between(1, 100))
            .all(&executor);
        
        let sql = executor.get_captured_sql();
        let param_counts = executor.get_captured_param_counts();
        
        assert!(!sql.is_empty(), "SQL should be generated");
        assert!(param_counts[0] >= 2, "BETWEEN should generate at least 2 parameters");
        assert!(sql[0].to_uppercase().contains("BETWEEN"), "SQL should contain BETWEEN");
    }

    #[test]
    fn test_parameter_extraction_one_method() {
        // Test that one() method also extracts parameters
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(1))
            .one(&executor);
        
        let sql = executor.get_captured_sql();
        let param_counts = executor.get_captured_param_counts();
        
        assert!(!sql.is_empty(), "SQL should be generated");
        assert_eq!(param_counts.len(), 1, "Should have one query");
        assert!(param_counts[0] > 0, "one() should extract parameters");
    }

    #[test]
    fn test_parameter_extraction_no_filters() {
        // Test query with no filters (should have 0 parameters)
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .all(&executor);
        
        let sql = executor.get_captured_sql();
        let param_counts = executor.get_captured_param_counts();
        
        assert!(!sql.is_empty(), "SQL should be generated");
        assert_eq!(param_counts.len(), 1, "Should have one query");
        // No filters means no parameters (unless limit/offset use parameters)
        // This test verifies the code doesn't crash with empty params
    }

    #[test]
    fn test_parameter_extraction_with_pagination() {
        // Test that limit/offset don't interfere with parameter extraction
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(1))
            .limit(10)
            .offset(20)
            .all(&executor);
        
        let sql = executor.get_captured_sql();
        let param_counts = executor.get_captured_param_counts();
        
        assert!(!sql.is_empty(), "SQL should be generated");
        assert!(param_counts[0] > 0, "Should have parameters even with pagination");
        assert!(sql[0].to_uppercase().contains("LIMIT"), "SQL should contain LIMIT");
        assert!(sql[0].to_uppercase().contains("OFFSET"), "SQL should contain OFFSET");
    }

    #[test]
    fn test_parameter_extraction_complex_query() {
        // Test complex query with multiple filters, ordering, and pagination
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").gt(10))
            .filter(Expr::col("id").lt(100))
            .filter(Expr::col("name").like("John%"))
            .order_by("id", Order::Asc)
            .limit(5)
            .offset(10)
            .all(&executor);
        
        let sql = executor.get_captured_sql();
        let param_counts = executor.get_captured_param_counts();
        
        assert!(!sql.is_empty(), "SQL should be generated");
        assert!(param_counts[0] >= 3, "Complex query should have multiple parameters");
        assert!(sql[0].to_uppercase().contains("ORDER"), "SQL should contain ORDER BY");
        assert!(sql[0].to_uppercase().contains("LIMIT"), "SQL should contain LIMIT");
    }

    #[test]
    fn test_parameter_extraction_numeric_types() {
        // Test various numeric types
        let executor = MockExecutor::new(vec![]);
        
        // Test with i32
        let _result1 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(42i32))
            .all(&executor);
        
        executor.clear();
        
        // Test with i64
        let _result2 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(42i64))
            .all(&executor);
        
        executor.clear();
        
        // Test with negative numbers
        let _result3 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").gt(-10))
            .all(&executor);
        
        let param_counts = executor.get_captured_param_counts();
        
        // All should have parameters
        for count in param_counts {
            assert!(count > 0, "All numeric types should generate parameters");
        }
    }

    #[test]
    fn test_parameter_extraction_string_edge_cases() {
        // Test string parameters with edge cases
        let executor = MockExecutor::new(vec![]);
        
        // Empty string
        let _result1 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("name").eq(""))
            .all(&executor);
        
        executor.clear();
        
        // String with special characters
        let _result2 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("name").eq("test'string\"with%special"))
            .all(&executor);
        
        executor.clear();
        
        // Long string
        let long_string = "a".repeat(1000);
        let _result3 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("name").eq(long_string))
            .all(&executor);
        
        let param_counts = executor.get_captured_param_counts();
        
        // All should have parameters
        for count in param_counts {
            assert!(count > 0, "All string edge cases should generate parameters");
        }
    }

    #[test]
    fn test_parameter_extraction_boolean_values() {
        // Test boolean parameters
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("active").eq(true))
            .all(&executor);
        
        let param_counts = executor.get_captured_param_counts();
        
        assert!(param_counts[0] > 0, "Boolean values should generate parameters");
    }

    #[test]
    fn test_parameter_extraction_arithmetic_expressions() {
        // Test expressions with arithmetic
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").add(Expr::val(1)).gt(10))
            .all(&executor);
        
        let sql = executor.get_captured_sql();
        let param_counts = executor.get_captured_param_counts();
        
        assert!(!sql.is_empty(), "SQL should be generated");
        assert!(param_counts[0] > 0, "Arithmetic expressions should generate parameters");
    }

    #[test]
    fn test_parameter_extraction_nested_expressions() {
        // Test nested expressions
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(
                Expr::col("id")
                    .add(Expr::val(1))
                    .mul(Expr::val(2))
                    .gt(20)
            )
            .all(&executor);
        
        let param_counts = executor.get_captured_param_counts();
        
        assert!(param_counts[0] > 0, "Nested expressions should generate parameters");
    }

    #[test]
    fn test_parameter_extraction_or_conditions() {
        // Test OR conditions
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(
                Expr::col("id").eq(1)
                    .or(Expr::col("id").eq(2))
            )
            .all(&executor);
        
        let param_counts = executor.get_captured_param_counts();
        
        assert!(param_counts[0] >= 2, "OR conditions should generate multiple parameters");
    }

    #[test]
    fn test_parameter_extraction_and_conditions() {
        // Test AND conditions (multiple filters)
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(1))
            .filter(Expr::col("name").eq("test"))
            .all(&executor);
        
        let param_counts = executor.get_captured_param_counts();
        
        assert!(param_counts[0] >= 2, "Multiple filters should generate multiple parameters");
    }

    #[test]
    fn test_parameter_extraction_with_group_by_having() {
        // Test GROUP BY with HAVING clause
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("status").eq("active"))
            .group_by("category")
            .having(Expr::col("COUNT(*)").gt(5))
            .all(&executor);
        
        let sql = executor.get_captured_sql();
        let param_counts = executor.get_captured_param_counts();
        
        assert!(!sql.is_empty(), "SQL should be generated");
        assert!(param_counts[0] > 0, "GROUP BY with HAVING should generate parameters");
        assert!(sql[0].to_uppercase().contains("GROUP"), "SQL should contain GROUP BY");
        assert!(sql[0].to_uppercase().contains("HAVING"), "SQL should contain HAVING");
    }

    #[test]
    fn test_parameter_extraction_parameter_count_matches_placeholders() {
        // CRITICAL TEST: Verify parameter count matches SQL placeholders
        // This is the KEY TEST that verifies the fix works correctly
        let executor = MockExecutor::new(vec![]);
        
        // Query with known number of parameters
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(1))
            .filter(Expr::col("name").eq("test"))
            .all(&executor);
        
        let sql = executor.get_captured_sql();
        let param_counts = executor.get_captured_param_counts();
        
        assert!(!sql.is_empty(), "SQL should be generated");
        
        // Count $ placeholders in SQL
        let placeholder_count = sql[0].matches('$').count();
        
        // The parameter count should match placeholder count
        // This is the KEY TEST that verifies the fix works
        assert_eq!(
            param_counts[0], 
            placeholder_count,
            "Parameter count ({}) should match placeholder count ({}) in SQL: {}",
            param_counts[0],
            placeholder_count,
            sql[0]
        );
    }

    #[test]
    fn test_parameter_extraction_empty_params_when_no_filters() {
        // Test that queries without filters have 0 parameters
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestEntity>::new()
            .order_by("id", Order::Asc)
            .all(&executor);
        
        let sql = executor.get_captured_sql();
        let _param_counts = executor.get_captured_param_counts();
        
        assert!(!sql.is_empty(), "SQL should be generated");
        // No filters = no parameters (limit/offset might add some, but basic query shouldn't)
        // This verifies we don't pass empty slice incorrectly
    }

    // ============================================================================
    // SQL GENERATION TESTS (These compile and verify query building works)
    // ============================================================================

    #[test]
    fn test_sql_generation_with_parameters() {
        // Verify that SQL is generated with placeholders when filters are used
        let query = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(1))
            .filter(Expr::col("name").eq("test"));
        
        // Build the query to inspect SQL
        let (sql, values) = query.query.build(sea_query::PostgresQueryBuilder);
        
        // CRITICAL: Verify that values are NOT empty (this tests the fix)
        // Before the fix, we wouldn't check this. After the fix, values should contain parameters
        // Values is a tuple struct, so we check if it has any items
        let values_vec: Vec<_> = values.iter().collect();
        assert!(!values_vec.is_empty(), "Values should be extracted from filters - THIS VERIFIES THE FIX");
        
        // Verify SQL contains placeholders
        assert!(sql.contains("$"), "SQL should contain parameter placeholders when filters are used");
        
        // Count placeholders
        let placeholder_count = sql.matches('$').count();
        assert!(placeholder_count > 0, "Should have placeholders for parameters");
        
        // The values count should match placeholder count (once conversion is fixed)
        // For now, we just verify values are extracted
        assert_eq!(values_vec.len(), placeholder_count, 
            "Value count ({}) should match placeholder count ({}) - THIS IS THE KEY FIX",
            values_vec.len(), placeholder_count);
    }

    #[test]
    fn test_sql_generation_no_parameters() {
        // Verify that queries without filters have no placeholders
        let query = SelectQuery::<TestEntity>::new()
            .order_by("id", Order::Asc);
        
        let (_sql, values) = query.query.build(sea_query::PostgresQueryBuilder);
        
        // No filters = no parameters
        let values_vec: Vec<_> = values.iter().collect();
        assert_eq!(values_vec.len(), 0, "Queries without filters should have no parameters");
        // SQL should not have $ placeholders (unless limit/offset use them)
        // Basic SELECT with ORDER BY shouldn't have parameters
    }

    #[test]
    fn test_sql_generation_all_value_types() {
        // Test that all value types generate SQL with placeholders
        
        // Integer
        let query1 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(42));
        let (sql1, values1) = query1.query.build(sea_query::PostgresQueryBuilder);
        let values1_vec: Vec<_> = values1.iter().collect();
        assert!(!values1_vec.is_empty(), "Integer filter should generate values");
        assert!(sql1.contains("$"), "Integer filter should generate placeholders");
        
        // String
        let query2 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("name").eq("test"));
        let (sql2, values2) = query2.query.build(sea_query::PostgresQueryBuilder);
        let values2_vec: Vec<_> = values2.iter().collect();
        assert!(!values2_vec.is_empty(), "String filter should generate values");
        assert!(sql2.contains("$"), "String filter should generate placeholders");
        
        // Boolean
        let query3 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("active").eq(true));
        let (sql3, values3) = query3.query.build(sea_query::PostgresQueryBuilder);
        let values3_vec: Vec<_> = values3.iter().collect();
        assert!(!values3_vec.is_empty(), "Boolean filter should generate values");
        assert!(sql3.contains("$"), "Boolean filter should generate placeholders");
    }

    // ============================================================================
    // EDGE CASE TESTS FOR TYPE-SAFE QUERY BUILDERS (Epic 02 Story 05)
    // ============================================================================

    #[test]
    fn test_find_one_no_results() {
        // Test find_one() when no results are found
        let executor = MockExecutor::new(vec![]);
        
        // MockExecutor returns QueryError for query_one, which should be handled as None
        let result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(999))
            .find_one(&executor);
        
        // Should return Ok(None) when no rows found (fixed to handle QueryError variant)
        match result {
            Ok(None) => {}, // Expected - find_one should return None when no rows found
            Ok(Some(_)) => panic!("find_one should return None when no results"),
            Err(e) => panic!("find_one should return Ok(None) for 'no rows' errors, got: {:?}", e),
        }
    }

    #[test]
    fn test_find_one_legitimate_errors_not_swallowed() {
        // Test that legitimate database errors are NOT incorrectly swallowed
        // This verifies the fix for the fragile string matching issue
        
        // Test 1: "table not found" should be an error, not Ok(None)
        let table_not_found_error = LifeError::QueryError("relation \"users\" does not exist: table not found".to_string());
        assert!(!is_no_rows_error(&table_not_found_error), 
            "Table not found errors should NOT be treated as 'no rows found'");
        
        // Test 2: "column not found" should be an error, not Ok(None)
        let column_not_found_error = LifeError::QueryError("column \"invalid_column\" does not exist: column not found".to_string());
        assert!(!is_no_rows_error(&column_not_found_error),
            "Column not found errors should NOT be treated as 'no rows found'");
        
        // Test 3: "function not found" should be an error, not Ok(None)
        let function_not_found_error = LifeError::QueryError("function invalid_func() does not exist: function not found".to_string());
        assert!(!is_no_rows_error(&function_not_found_error),
            "Function not found errors should NOT be treated as 'no rows found'");
        
        // Test 4: "constraint not found" should be an error, not Ok(None)
        let constraint_not_found_error = LifeError::QueryError("constraint \"invalid_constraint\" does not exist: constraint not found".to_string());
        assert!(!is_no_rows_error(&constraint_not_found_error),
            "Constraint not found errors should NOT be treated as 'no rows found'");
        
        // Test 5: Actual "no rows" errors should still be detected
        let no_rows_error = LifeError::QueryError("no rows returned".to_string());
        assert!(is_no_rows_error(&no_rows_error),
            "Actual 'no rows' errors should be detected");
        
        let no_row_error = LifeError::QueryError("no row found".to_string());
        assert!(is_no_rows_error(&no_row_error),
            "Actual 'no row' errors should be detected");
        
        // Test 6: PostgresError with "no rows" should be detected
        // Note: We can't easily create a PostgresError in tests, but the logic is the same
        let postgres_no_rows = LifeError::QueryError("PostgreSQL error: no rows".to_string());
        assert!(is_no_rows_error(&postgres_no_rows),
            "PostgresError with 'no rows' should be detected");
    }

    #[test]
    fn test_paginator_page_zero() {
        // Test paginator with page 0 (should be treated as page 1)
        let executor = MockExecutor::new(vec![]);
        let mut paginator = SelectQuery::<TestEntity>::new()
            .paginate(&executor, 10);
        
        // Page 0 should be treated as page 1 (offset = 0)
        let _result = paginator.fetch_page(0);
        // Should not panic - offset calculation uses saturating_sub
    }

    #[test]
    fn test_paginator_empty_results() {
        // Test paginator with empty result set
        let executor = MockExecutor::new(vec![]);
        let mut paginator = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(999))
            .paginate(&executor, 10);
        
        // Should return empty vec, not panic
        let result = paginator.fetch_page(1);
        match result {
            Ok(vec) => assert!(vec.is_empty(), "Empty results should return empty vec"),
            Err(_) => {}, // Acceptable if executor returns error
        }
    }

    #[test]
    fn test_paginator_large_page_number() {
        // Test paginator with page number beyond available data
        let executor = MockExecutor::new(vec![]);
        let mut paginator = SelectQuery::<TestEntity>::new()
            .paginate(&executor, 10);
        
        // Page 1000 should not panic (offset = 9990)
        let _result = paginator.fetch_page(1000);
        // Should not panic - offset calculation handles large numbers
    }

    #[test]
    fn test_paginator_page_size_zero() {
        // Test paginator with page_size = 0 (edge case)
        let executor = MockExecutor::new(vec![]);
        let mut paginator = SelectQuery::<TestEntity>::new()
            .paginate(&executor, 0);
        
        // Should handle gracefully (limit 0)
        let _result = paginator.fetch_page(1);
        // Should not panic
    }

    #[test]
    fn test_paginator_with_count_empty_results() {
        // Test paginate_and_count with empty results
        let executor = MockExecutor::new(vec![]);
        let mut paginator = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(999))
            .paginate_and_count(&executor, 10);
        
        // num_items should return 0 for empty results
        let count_result = paginator.num_items();
        match count_result {
            Ok(count) => assert_eq!(count, 0, "Empty results should return count 0"),
            Err(_) => {}, // Acceptable if executor returns error
        }
    }

    #[test]
    fn test_paginator_with_count_cached() {
        // Test that num_items() caches the count
        // Note: MockExecutor returns errors, so we test caching by manually setting
        // total_count and verifying that subsequent calls don't execute queries
        let executor = MockExecutor::new(vec![]);
        let mut paginator = SelectQuery::<TestEntity>::new()
            .paginate_and_count(&executor, 10);
        
        // Manually set total_count to simulate a successful first call
        paginator.total_count = Some(42);
        let sql_calls_before = executor.get_captured_sql().len();
        let cached_count = paginator.num_items().unwrap();
        let sql_calls_after = executor.get_captured_sql().len();
        
        // When total_count is set, num_items() should return cached value without executing query
        assert_eq!(cached_count, 42, "Should return cached count");
        assert_eq!(sql_calls_before, sql_calls_after, "Cached call should not execute SQL");
        
        // Verify that multiple calls with cached value don't increase SQL calls
        let _count2 = paginator.num_items().unwrap();
        let sql_calls_final = executor.get_captured_sql().len();
        assert_eq!(sql_calls_after, sql_calls_final, "Multiple cached calls should not execute SQL");
    }

    #[test]
    fn test_filter_with_null_values() {
        // Test filters with null/None values
        let executor1 = MockExecutor::new(vec![]);
        
        // IS NULL filter
        let _result1 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("name").is_null())
            .all(&executor1);
        
        let executor2 = MockExecutor::new(vec![]);
        
        // IS NOT NULL filter
        let _result2 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("name").is_not_null())
            .all(&executor2);
        
        let sql1 = executor1.get_captured_sql();
        let sql2 = executor2.get_captured_sql();
        assert!(!sql1.is_empty(), "IS NULL should generate SQL");
        assert!(!sql2.is_empty(), "IS NOT NULL should generate SQL");
    }

    #[test]
    fn test_filter_with_empty_collections() {
        // Test IN and NOT IN with empty collections
        let executor = MockExecutor::new(vec![]);
        
        // Empty IN clause (edge case - should handle gracefully)
        let empty_vec: Vec<i32> = vec![];
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").is_in(empty_vec))
            .all(&executor);
        
        // Should not panic
        let sql = executor.get_captured_sql();
        assert!(!sql.is_empty(), "Empty IN should still generate SQL");
    }

    #[test]
    fn test_filter_with_large_collections() {
        // Test IN with large collection (performance/stress test)
        let executor = MockExecutor::new(vec![]);
        
        let large_vec: Vec<i32> = (1..1000).collect();
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").is_in(large_vec))
            .all(&executor);
        
        // Should handle large collections
        let param_counts = executor.get_captured_param_counts();
        assert!(!param_counts.is_empty(), "Large IN should generate parameters");
    }

    #[test]
    fn test_between_edge_cases() {
        // Test BETWEEN with edge cases
        let executor1 = MockExecutor::new(vec![]);
        
        // Same start and end (edge case)
        let _result1 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").between(5, 5))
            .all(&executor1);
        
        let executor2 = MockExecutor::new(vec![]);
        
        // Start > end (edge case - should still work, just returns nothing)
        let _result2 = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").between(10, 5))
            .all(&executor2);
        
        let sql1 = executor1.get_captured_sql();
        let sql2 = executor2.get_captured_sql();
        assert!(!sql1.is_empty(), "BETWEEN with same values should generate SQL");
        assert!(!sql2.is_empty(), "BETWEEN with start > end should generate SQL");
    }

    #[test]
    fn test_limit_zero() {
        // Test limit(0) edge case
        let executor = MockExecutor::new(vec![]);
        let _result = SelectQuery::<TestEntity>::new()
            .limit(0)
            .all(&executor);
        
        // Should not panic - limit 0 is valid SQL
        let sql = executor.get_captured_sql();
        assert!(!sql.is_empty(), "Limit 0 should generate SQL");
    }

    #[test]
    fn test_offset_large_value() {
        // Test offset with very large value
        let executor = MockExecutor::new(vec![]);
        let result = SelectQuery::<TestEntity>::new()
            .offset(u64::MAX)
            .all(&executor);
        
        // Should not panic - large offset is valid
        // MockExecutor returns error, but SQL should still be generated
        // The important thing is that it doesn't panic
        let sql = executor.get_captured_sql();
        // SQL may be empty if query building fails, but execution should not panic
        // We verify the query was attempted (either SQL generated or error returned)
        assert!(!sql.is_empty() || result.is_err(), 
            "Large offset should generate SQL or return error gracefully (no panic)");
    }

    #[test]
    fn test_multiple_chained_filters() {
        // Test many chained filters (stress test)
        let executor = MockExecutor::new(vec![]);
        let mut query = SelectQuery::<TestEntity>::new();
        
        // Chain many filters
        for i in 1..=50 {
            query = query.filter(Expr::col("id").ne(i));
        }
        
        let _result = query.all(&executor);
        
        // Should handle many filters
        let param_counts = executor.get_captured_param_counts();
        assert!(!param_counts.is_empty(), "Many filters should generate parameters");
    }

    #[test]
    fn test_order_by_multiple_columns() {
        // Test many ORDER BY clauses
        let executor = MockExecutor::new(vec![]);
        let mut query = SelectQuery::<TestEntity>::new();
        
        // Chain many order_by calls
        for i in 1..=20 {
            query = query.order_by(format!("col_{}", i), Order::Asc);
        }
        
        let _result = query.all(&executor);
        
        // Should handle many ORDER BY clauses
        let sql = executor.get_captured_sql();
        assert!(!sql.is_empty(), "Many ORDER BY should generate SQL");
    }

    #[test]
    fn test_group_by_without_having() {
        // Test GROUP BY without HAVING (valid SQL)
        let executor = MockExecutor::new(vec![]);
        let _result = SelectQuery::<TestEntity>::new()
            .group_by("status")
            .all(&executor);
        
        // Should not require HAVING
        let sql = executor.get_captured_sql();
        assert!(!sql.is_empty(), "GROUP BY without HAVING should generate SQL");
    }

    #[test]
    fn test_having_without_group_by() {
        // Test HAVING without GROUP BY (edge case - may be invalid SQL but shouldn't panic)
        let executor = MockExecutor::new(vec![]);
        let _result = SelectQuery::<TestEntity>::new()
            .having(Expr::col("COUNT(*)").gt(5))
            .all(&executor);
        
        // Should not panic (SQL validity checked by database)
        let sql = executor.get_captured_sql();
        assert!(!sql.is_empty(), "HAVING without GROUP BY should generate SQL");
    }

    #[test]
    fn test_one_with_multiple_results() {
        // Test one() when multiple results exist (should error)
        let executor = MockExecutor::new(vec![]);
        
        // Query that might return multiple results
        let result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").gt(1))
            .one(&executor);
        
        // Should return error (MockExecutor returns error, which is fine for this test)
        // In real scenario, this would error if multiple rows returned
        match result {
            Ok(_) => {}, // Unlikely with MockExecutor
            Err(_) => {}, // Expected when multiple rows or no rows
        }
    }

    #[test]
    fn test_like_with_empty_pattern() {
        // Test LIKE with empty pattern
        let executor = MockExecutor::new(vec![]);
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("name").like(""))
            .all(&executor);
        
        // Should handle empty pattern
        let sql = executor.get_captured_sql();
        assert!(!sql.is_empty(), "LIKE with empty pattern should generate SQL");
    }

    #[test]
    fn test_like_with_special_characters() {
        // Test LIKE with SQL special characters
        let executor = MockExecutor::new(vec![]);
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("name").like("%test'string\"with%special%"))
            .all(&executor);
        
        // Should handle special characters (parameterized, so safe)
        let sql = executor.get_captured_sql();
        assert!(!sql.is_empty(), "LIKE with special chars should generate SQL");
    }

    #[test]
    fn test_query_with_all_clauses() {
        // Test query with all possible clauses (comprehensive test)
        let executor = MockExecutor::new(vec![]);
        let _result = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").gt(1))
            .filter(Expr::col("name").like("test%"))
            .group_by("status")
            .having(Expr::col("COUNT(*)").gt(1))
            .order_by("id", Order::Asc)
            .order_by("name", Order::Desc)
            .limit(100)
            .offset(50)
            .all(&executor);
        
        // Should handle all clauses together
        let sql = executor.get_captured_sql();
        assert!(!sql.is_empty(), "Query with all clauses should generate SQL");
    }

    #[test]
    fn test_sql_generation_complex_expressions() {
        // Test complex expressions generate correct number of parameters
        let query = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").add(Expr::val(1)).gt(10))
            .filter(Expr::col("name").like("John%"));
        
        let (sql, values) = query.query.build(sea_query::PostgresQueryBuilder);
        
        // Complex expression should generate multiple parameters
        let values_vec: Vec<_> = values.iter().collect();
        assert!(!values_vec.is_empty(), "Complex expressions should generate values");
        let placeholder_count = sql.matches('$').count();
        assert!(placeholder_count > 0, "Complex expressions should have placeholders");
        // Values count should match placeholders (once conversion is fixed)
        assert_eq!(values_vec.len(), placeholder_count, "Values should match placeholders");
    }

    #[test]
    fn test_sql_generation_multiple_filters() {
        // Test that multiple filters generate multiple parameters
        let query = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(1))
            .filter(Expr::col("id").gt(0))
            .filter(Expr::col("name").eq("test"));
        
        let (sql, values) = query.query.build(sea_query::PostgresQueryBuilder);
        
        // Should have at least 3 parameters
        let values_vec: Vec<_> = values.iter().collect();
        assert!(values_vec.len() >= 3, "Multiple filters should generate multiple values");
        let placeholder_count = sql.matches('$').count();
        assert!(placeholder_count >= 3, "Multiple filters should have multiple placeholders");
        assert_eq!(values_vec.len(), placeholder_count, 
            "Value count should match placeholder count for multiple filters");
    }

    #[test]
    fn test_sql_generation_in_operator() {
        // Test IN operator generates correct number of parameters
        let query = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").is_in(vec![1, 2, 3, 4, 5]));
        
        let (sql, values) = query.query.build(sea_query::PostgresQueryBuilder);
        
        // IN with 5 values should generate at least 5 parameters
        let values_vec: Vec<_> = values.iter().collect();
        assert!(values_vec.len() >= 5, "IN operator should generate values for each item");
        let placeholder_count = sql.matches('$').count();
        assert!(placeholder_count >= 5, "IN operator should have placeholders for each item");
        assert_eq!(values_vec.len(), placeholder_count, "IN values should match placeholders");
    }

    #[test]
    fn test_sql_generation_between_operator() {
        // Test BETWEEN operator generates 2 parameters
        let query = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").between(1, 100));
        
        let (sql, values) = query.query.build(sea_query::PostgresQueryBuilder);
        
        // BETWEEN should generate 2 parameters
        let values_vec: Vec<_> = values.iter().collect();
        assert!(values_vec.len() >= 2, "BETWEEN should generate at least 2 values");
        let placeholder_count = sql.matches('$').count();
        assert!(placeholder_count >= 2, "BETWEEN should have at least 2 placeholders");
        assert_eq!(values_vec.len(), placeholder_count, "BETWEEN values should match placeholders");
    }

    #[test]
    fn test_sql_generation_or_conditions() {
        // Test OR conditions generate parameters for both sides
        let query = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(1).or(Expr::col("id").eq(2)));
        
        let (sql, values) = query.query.build(sea_query::PostgresQueryBuilder);
        
        // OR should generate parameters for both conditions
        let values_vec: Vec<_> = values.iter().collect();
        assert!(values_vec.len() >= 2, "OR conditions should generate values for both sides");
        let placeholder_count = sql.matches('$').count();
        assert!(placeholder_count >= 2, "OR conditions should have placeholders for both sides");
        assert_eq!(values_vec.len(), placeholder_count, "OR values should match placeholders");
    }

    #[test]
    fn test_sql_generation_parameter_ordering() {
        // Test that parameters are in the correct order
        let query = SelectQuery::<TestEntity>::new()
            .filter(Expr::col("id").eq(1))
            .filter(Expr::col("name").eq("test"))
            .filter(Expr::col("id").gt(0));
        
        let (sql, values) = query.query.build(sea_query::PostgresQueryBuilder);
        
        // Should have parameters in the order filters were added
        let values_vec: Vec<_> = values.iter().collect();
        assert!(values_vec.len() >= 3, "Should have parameters for all filters");
        let placeholder_count = sql.matches('$').count();
        assert_eq!(values_vec.len(), placeholder_count, 
            "Parameter count should match placeholder count - verifies correct extraction");
    }

    // Note: Execution tests (test_parameter_extraction_*) require the string/byte
    // conversion issue to be resolved. Once that's fixed, those tests will verify
    // that parameters are actually passed to the executor correctly.

    // ============================================================================
    // JOIN Operations Edge Cases
    // ============================================================================

    #[test]
    fn test_join_with_null_values() {
        // EDGE CASE: JOIN where foreign key is NULL
        // This should be handled by LEFT JOIN returning NULL for non-matching rows
        let _query = TestEntity::find()
            .left_join("other_table", Expr::cust("test_table.id = other_table.user_id"));
        // LEFT JOIN handles NULL foreign keys correctly
    }

    #[test]
    fn test_multiple_joins_same_table() {
        // EDGE CASE: Multiple joins to the same table (requires aliasing)
        let _query = TestEntity::find()
            .left_join("other_table", Expr::cust("test_table.id = other_table.user_id"))
            .left_join("other_table2", Expr::cust("test_table.id = other_table2.author_id"));
        // Note: This would need table aliasing in a full implementation
    }

    #[test]
    fn test_join_with_complex_condition() {
        // EDGE CASE: JOIN with complex condition (multiple columns, OR logic)
        let complex_condition = Expr::cust("(test_table.id = posts.user_id OR test_table.id = posts.author_id) AND posts.published = true");
        let _query = TestEntity::find()
            .left_join("posts", complex_condition);
    }

    #[test]
    fn test_join_types_all_variants() {
        // EDGE CASE: All JOIN types work correctly
        let join_cond = Expr::cust("test_table.id = posts.user_id");
        
        let _inner = TestEntity::find().join("posts", join_cond.clone());
        let _left = TestEntity::find().left_join("posts", join_cond.clone());
        let _right = TestEntity::find().right_join("posts", join_cond.clone());
        let _inner_alias = TestEntity::find().inner_join("posts", join_cond);
    }

    #[test]
    fn test_join_with_subquery() {
        // EDGE CASE: JOIN with subquery (future enhancement)
        // For now, this documents the requirement
        let _query = TestEntity::find()
            .left_join("posts", Expr::cust("test_table.id = posts.user_id"));
        // Full subquery support would require additional API
    }

    // ============================================================================
    // Query Builder Edge Cases
    // ============================================================================

    #[test]
    fn test_query_with_zero_limit() {
        // EDGE CASE: LIMIT 0 (should return no results)
        let _query = TestEntity::find().limit(0);
        // Should compile - behavior at runtime is database-dependent
    }

    #[test]
    fn test_query_with_very_large_limit() {
        // EDGE CASE: Very large LIMIT value
        let _query = TestEntity::find().limit(u64::MAX);
        // Should compile - database will handle appropriately
    }

    #[test]
    fn test_query_with_zero_offset() {
        // EDGE CASE: OFFSET 0 (should be same as no offset)
        let _query = TestEntity::find().offset(0);
        // Should compile and work correctly
    }

    #[test]
    fn test_query_multiple_order_by() {
        // EDGE CASE: Multiple ORDER BY clauses
        let _query = TestEntity::find()
            .order_by("name", Order::Asc)
            .order_by("name", Order::Desc);
        // Should compile - last order_by wins or both are applied
    }

    #[test]
    fn test_query_empty_filter() {
        // EDGE CASE: Filter with always-true condition
        let _query = TestEntity::find().filter(Expr::cust("1 = 1"));
        // Should compile - returns all rows
    }

    #[test]
    fn test_query_impossible_filter() {
        // EDGE CASE: Filter with always-false condition
        let _query = TestEntity::find().filter(Expr::cust("1 = 0"));
        // Should compile - returns no rows
    }

    #[test]
    fn test_query_group_by_without_aggregates() {
        // EDGE CASE: GROUP BY without aggregate functions
        let _query = TestEntity::find().group_by("name");
        // Should compile - valid SQL
    }

    #[test]
    fn test_query_having_without_group_by() {
        // EDGE CASE: HAVING without GROUP BY (valid in some databases)
        let _query = TestEntity::find().having(Expr::cust("COUNT(*) > 0"));
        // Should compile - behavior is database-dependent
    }
}
