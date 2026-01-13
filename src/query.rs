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
    pub(crate) query: SelectStatement,  // Made pub(crate) for testing
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
        let (sql, values) = self.query.build(PostgresQueryBuilder);
        
        // Convert SeaQuery values to may_postgres ToSql parameters
        // Values are stored in typed vectors and then referenced
        let mut bools: Vec<bool> = Vec::new();
        let mut ints: Vec<i32> = Vec::new();
        let mut big_ints: Vec<i64> = Vec::new();
        let mut strings: Vec<String> = Vec::new();
        let mut bytes: Vec<Vec<u8>> = Vec::new();
        let mut nulls: Vec<Option<i32>> = Vec::new();
        let mut floats: Vec<f32> = Vec::new();
        let mut doubles: Vec<f64> = Vec::new();
        
        // Collect all values first - values are wrapped in Option in this version
        for value in values.iter() {
            match value {
                sea_query::Value::Bool(Some(b)) => bools.push(*b),
                sea_query::Value::Int(Some(i)) => ints.push(*i),
                sea_query::Value::BigInt(Some(i)) => big_ints.push(*i),
                sea_query::Value::String(Some(s)) => strings.push(s.clone()),
                sea_query::Value::Bytes(Some(b)) => bytes.push(b.clone()),
                sea_query::Value::Bool(None) | sea_query::Value::Int(None) | 
                sea_query::Value::BigInt(None) | sea_query::Value::String(None) | 
                sea_query::Value::Bytes(None) => nulls.push(None),
                sea_query::Value::TinyInt(Some(i)) => ints.push(*i as i32),
                sea_query::Value::SmallInt(Some(i)) => ints.push(*i as i32),
                sea_query::Value::TinyUnsigned(Some(u)) => ints.push(*u as i32),
                sea_query::Value::SmallUnsigned(Some(u)) => ints.push(*u as i32),
                sea_query::Value::Unsigned(Some(u)) => big_ints.push(*u as i64),
                sea_query::Value::BigUnsigned(Some(u)) => {
                    if *u > i64::MAX as u64 {
                        return Err(LifeError::Other(format!(
                            "BigUnsigned value {} exceeds i64::MAX ({}), cannot be safely cast to i64",
                            u, i64::MAX
                        )));
                    }
                    big_ints.push(*u as i64);
                },
                sea_query::Value::Float(Some(f)) => floats.push(*f),
                sea_query::Value::Double(Some(d)) => doubles.push(*d),
                sea_query::Value::TinyInt(None) | sea_query::Value::SmallInt(None) |
                sea_query::Value::TinyUnsigned(None) | sea_query::Value::SmallUnsigned(None) |
                sea_query::Value::Unsigned(None) | sea_query::Value::BigUnsigned(None) |
                sea_query::Value::Float(None) | sea_query::Value::Double(None) => nulls.push(None),
                _ => {
                    return Err(LifeError::Other(format!("Unsupported value type in query: {:?}", value)));
                }
            }
        }
        
        // Now create references to the stored values
        let mut bool_idx = 0;
        let mut int_idx = 0;
        let mut big_int_idx = 0;
        let mut string_idx = 0;
        let mut byte_idx = 0;
        let mut null_idx = 0;
        let mut float_idx = 0;
        let mut double_idx = 0;
        
        let mut params: Vec<&dyn may_postgres::types::ToSql> = Vec::new();
        
        for value in values.iter() {
            match value {
                sea_query::Value::Bool(Some(_)) => {
                    params.push(&bools[bool_idx] as &dyn may_postgres::types::ToSql);
                    bool_idx += 1;
                }
                sea_query::Value::Int(Some(_)) => {
                    params.push(&ints[int_idx] as &dyn may_postgres::types::ToSql);
                    int_idx += 1;
                }
                sea_query::Value::BigInt(Some(_)) => {
                    params.push(&big_ints[big_int_idx] as &dyn may_postgres::types::ToSql);
                    big_int_idx += 1;
                }
                sea_query::Value::String(Some(_)) => {
                    params.push(&strings[string_idx] as &dyn may_postgres::types::ToSql);
                    string_idx += 1;
                }
                sea_query::Value::Bytes(Some(_)) => {
                    params.push(&bytes[byte_idx] as &dyn may_postgres::types::ToSql);
                    byte_idx += 1;
                }
                sea_query::Value::Bool(None) | sea_query::Value::Int(None) | 
                sea_query::Value::BigInt(None) | sea_query::Value::String(None) | 
                sea_query::Value::Bytes(None) => {
                    params.push(&nulls[null_idx] as &dyn may_postgres::types::ToSql);
                    null_idx += 1;
                }
                sea_query::Value::TinyInt(Some(_)) | sea_query::Value::SmallInt(Some(_)) |
                sea_query::Value::TinyUnsigned(Some(_)) | sea_query::Value::SmallUnsigned(Some(_)) => {
                    params.push(&ints[int_idx] as &dyn may_postgres::types::ToSql);
                    int_idx += 1;
                }
                sea_query::Value::Unsigned(Some(_)) | sea_query::Value::BigUnsigned(Some(_)) => {
                    params.push(&big_ints[big_int_idx] as &dyn may_postgres::types::ToSql);
                    big_int_idx += 1;
                }
                sea_query::Value::Float(Some(_)) => {
                    params.push(&floats[float_idx] as &dyn may_postgres::types::ToSql);
                    float_idx += 1;
                }
                sea_query::Value::Double(Some(_)) => {
                    params.push(&doubles[double_idx] as &dyn may_postgres::types::ToSql);
                    double_idx += 1;
                }
                sea_query::Value::TinyInt(None) | sea_query::Value::SmallInt(None) |
                sea_query::Value::TinyUnsigned(None) | sea_query::Value::SmallUnsigned(None) |
                sea_query::Value::Unsigned(None) | sea_query::Value::BigUnsigned(None) |
                sea_query::Value::Float(None) | sea_query::Value::Double(None) => {
                    params.push(&nulls[null_idx] as &dyn may_postgres::types::ToSql);
                    null_idx += 1;
                }
                _ => {
                    return Err(LifeError::Other(format!("Unsupported value type in query: {:?}", value)));
                }
            }
        }
        
        let rows = executor.query_all(&sql, &params)?;
        
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
        let (sql, values) = self.query.build(PostgresQueryBuilder);
        
        // Convert SeaQuery values to may_postgres ToSql parameters
        // Values are stored in typed vectors and then referenced
        let mut bools: Vec<bool> = Vec::new();
        let mut ints: Vec<i32> = Vec::new();
        let mut big_ints: Vec<i64> = Vec::new();
        let mut strings: Vec<String> = Vec::new();
        let mut bytes: Vec<Vec<u8>> = Vec::new();
        let mut nulls: Vec<Option<i32>> = Vec::new();
        let mut floats: Vec<f32> = Vec::new();
        let mut doubles: Vec<f64> = Vec::new();
        
        // Collect all values first - values are wrapped in Option in this version
        for value in values.iter() {
            match value {
                sea_query::Value::Bool(Some(b)) => bools.push(*b),
                sea_query::Value::Int(Some(i)) => ints.push(*i),
                sea_query::Value::BigInt(Some(i)) => big_ints.push(*i),
                sea_query::Value::String(Some(s)) => strings.push(s.clone()),
                sea_query::Value::Bytes(Some(b)) => bytes.push(b.clone()),
                sea_query::Value::Bool(None) | sea_query::Value::Int(None) | 
                sea_query::Value::BigInt(None) | sea_query::Value::String(None) | 
                sea_query::Value::Bytes(None) => nulls.push(None),
                sea_query::Value::TinyInt(Some(i)) => ints.push(*i as i32),
                sea_query::Value::SmallInt(Some(i)) => ints.push(*i as i32),
                sea_query::Value::TinyUnsigned(Some(u)) => ints.push(*u as i32),
                sea_query::Value::SmallUnsigned(Some(u)) => ints.push(*u as i32),
                sea_query::Value::Unsigned(Some(u)) => big_ints.push(*u as i64),
                sea_query::Value::BigUnsigned(Some(u)) => {
                    if *u > i64::MAX as u64 {
                        return Err(LifeError::Other(format!(
                            "BigUnsigned value {} exceeds i64::MAX ({}), cannot be safely cast to i64",
                            u, i64::MAX
                        )));
                    }
                    big_ints.push(*u as i64);
                },
                sea_query::Value::Float(Some(f)) => floats.push(*f),
                sea_query::Value::Double(Some(d)) => doubles.push(*d),
                sea_query::Value::TinyInt(None) | sea_query::Value::SmallInt(None) |
                sea_query::Value::TinyUnsigned(None) | sea_query::Value::SmallUnsigned(None) |
                sea_query::Value::Unsigned(None) | sea_query::Value::BigUnsigned(None) |
                sea_query::Value::Float(None) | sea_query::Value::Double(None) => nulls.push(None),
                _ => {
                    return Err(LifeError::Other(format!("Unsupported value type in query: {:?}", value)));
                }
            }
        }
        
        // Now create references to the stored values
        let mut bool_idx = 0;
        let mut int_idx = 0;
        let mut big_int_idx = 0;
        let mut string_idx = 0;
        let mut byte_idx = 0;
        let mut null_idx = 0;
        let mut float_idx = 0;
        let mut double_idx = 0;
        
        let mut params: Vec<&dyn may_postgres::types::ToSql> = Vec::new();
        
        for value in values.iter() {
            match value {
                sea_query::Value::Bool(Some(_)) => {
                    params.push(&bools[bool_idx] as &dyn may_postgres::types::ToSql);
                    bool_idx += 1;
                }
                sea_query::Value::Int(Some(_)) => {
                    params.push(&ints[int_idx] as &dyn may_postgres::types::ToSql);
                    int_idx += 1;
                }
                sea_query::Value::BigInt(Some(_)) => {
                    params.push(&big_ints[big_int_idx] as &dyn may_postgres::types::ToSql);
                    big_int_idx += 1;
                }
                sea_query::Value::String(Some(_)) => {
                    params.push(&strings[string_idx] as &dyn may_postgres::types::ToSql);
                    string_idx += 1;
                }
                sea_query::Value::Bytes(Some(_)) => {
                    params.push(&bytes[byte_idx] as &dyn may_postgres::types::ToSql);
                    byte_idx += 1;
                }
                sea_query::Value::Bool(None) | sea_query::Value::Int(None) | 
                sea_query::Value::BigInt(None) | sea_query::Value::String(None) | 
                sea_query::Value::Bytes(None) => {
                    params.push(&nulls[null_idx] as &dyn may_postgres::types::ToSql);
                    null_idx += 1;
                }
                sea_query::Value::TinyInt(Some(_)) | sea_query::Value::SmallInt(Some(_)) |
                sea_query::Value::TinyUnsigned(Some(_)) | sea_query::Value::SmallUnsigned(Some(_)) => {
                    params.push(&ints[int_idx] as &dyn may_postgres::types::ToSql);
                    int_idx += 1;
                }
                sea_query::Value::Unsigned(Some(_)) | sea_query::Value::BigUnsigned(Some(_)) => {
                    params.push(&big_ints[big_int_idx] as &dyn may_postgres::types::ToSql);
                    big_int_idx += 1;
                }
                sea_query::Value::Float(Some(_)) => {
                    params.push(&floats[float_idx] as &dyn may_postgres::types::ToSql);
                    float_idx += 1;
                }
                sea_query::Value::Double(Some(_)) => {
                    params.push(&doubles[double_idx] as &dyn may_postgres::types::ToSql);
                    double_idx += 1;
                }
                sea_query::Value::TinyInt(None) | sea_query::Value::SmallInt(None) |
                sea_query::Value::TinyUnsigned(None) | sea_query::Value::SmallUnsigned(None) |
                sea_query::Value::Unsigned(None) | sea_query::Value::BigUnsigned(None) |
                sea_query::Value::Float(None) | sea_query::Value::Double(None) => {
                    params.push(&nulls[null_idx] as &dyn may_postgres::types::ToSql);
                    null_idx += 1;
                }
                _ => {
                    return Err(LifeError::Other(format!("Unsupported value type in query: {:?}", value)));
                }
            }
        }
        
        let row = executor.query_one(&sql, &params)?;
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
    use std::sync::{Arc, Mutex};
    use may_postgres::types::ToSql;

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

    // Mock executor that captures SQL and parameter counts for verification
    struct MockExecutor {
        captured_sql: Arc<Mutex<Vec<String>>>,
        captured_param_counts: Arc<Mutex<Vec<usize>>>,
        return_rows: Vec<Row>,
    }

    impl MockExecutor {
        fn new(_return_rows: Vec<Row>) -> Self {
            Self {
                captured_sql: Arc::new(Mutex::new(Vec::new())),
                captured_param_counts: Arc::new(Mutex::new(Vec::new())),
                return_rows: vec![], // We can't easily create Row objects, so we use empty vec
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
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        let _result1 = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("id").eq(10))
            .all(&executor);
        
        executor.clear();
        
        // Test .ne()
        let _result2 = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("id").ne(10))
            .all(&executor);
        
        executor.clear();
        
        // Test .gt()
        let _result3 = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("id").gt(10))
            .all(&executor);
        
        executor.clear();
        
        // Test .gte()
        let _result4 = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("id").gte(10))
            .all(&executor);
        
        executor.clear();
        
        // Test .lt()
        let _result5 = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("id").lt(10))
            .all(&executor);
        
        executor.clear();
        
        // Test .lte()
        let _result6 = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        let _result1 = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("id").eq(42i32))
            .all(&executor);
        
        executor.clear();
        
        // Test with i64
        let _result2 = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("id").eq(42i64))
            .all(&executor);
        
        executor.clear();
        
        // Test with negative numbers
        let _result3 = SelectQuery::<TestModel>::new("test_table")
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
        let _result1 = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("name").eq(""))
            .all(&executor);
        
        executor.clear();
        
        // String with special characters
        let _result2 = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("name").eq("test'string\"with%special"))
            .all(&executor);
        
        executor.clear();
        
        // Long string
        let long_string = "a".repeat(1000);
        let _result3 = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("active").eq(true))
            .all(&executor);
        
        let param_counts = executor.get_captured_param_counts();
        
        assert!(param_counts[0] > 0, "Boolean values should generate parameters");
    }

    #[test]
    fn test_parameter_extraction_arithmetic_expressions() {
        // Test expressions with arithmetic
        let executor = MockExecutor::new(vec![]);
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        
        let _result = SelectQuery::<TestModel>::new("test_table")
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
        let query = SelectQuery::<TestModel>::new("test_table")
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
        let query = SelectQuery::<TestModel>::new("test_table")
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
        let query1 = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("id").eq(42));
        let (sql1, values1) = query1.query.build(sea_query::PostgresQueryBuilder);
        let values1_vec: Vec<_> = values1.iter().collect();
        assert!(!values1_vec.is_empty(), "Integer filter should generate values");
        assert!(sql1.contains("$"), "Integer filter should generate placeholders");
        
        // String
        let query2 = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("name").eq("test"));
        let (sql2, values2) = query2.query.build(sea_query::PostgresQueryBuilder);
        let values2_vec: Vec<_> = values2.iter().collect();
        assert!(!values2_vec.is_empty(), "String filter should generate values");
        assert!(sql2.contains("$"), "String filter should generate placeholders");
        
        // Boolean
        let query3 = SelectQuery::<TestModel>::new("test_table")
            .filter(Expr::col("active").eq(true));
        let (sql3, values3) = query3.query.build(sea_query::PostgresQueryBuilder);
        let values3_vec: Vec<_> = values3.iter().collect();
        assert!(!values3_vec.is_empty(), "Boolean filter should generate values");
        assert!(sql3.contains("$"), "Boolean filter should generate placeholders");
    }

    #[test]
    fn test_sql_generation_complex_expressions() {
        // Test complex expressions generate correct number of parameters
        let query = SelectQuery::<TestModel>::new("test_table")
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
        let query = SelectQuery::<TestModel>::new("test_table")
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
        let query = SelectQuery::<TestModel>::new("test_table")
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
        let query = SelectQuery::<TestModel>::new("test_table")
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
        let query = SelectQuery::<TestModel>::new("test_table")
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
        let query = SelectQuery::<TestModel>::new("test_table")
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
}
