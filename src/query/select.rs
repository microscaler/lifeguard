//! Select query builder for LifeModel.
//!
//! This module provides `SelectQuery` and `SelectModel` for building and executing
//! type-safe database queries. Query building methods (filter, order_by, limit, etc.)
//! are defined here, while execution methods are in the execution module.

use crate::query::traits::{LifeModelTrait, FromRow};
use sea_query::{SelectStatement, Iden, Expr, Order, IntoColumnRef};
use std::marker::PhantomData;

/// Query builder for selecting records
///
/// This is returned by `LifeModelTrait::find()` and can be chained with filters,
/// ordering, pagination, and grouping.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{SelectQuery, LifeModelTrait, LifeExecutor};
/// use sea_query::{Expr, Order};
///
/// # struct UserModel { id: i32, name: String };
/// # impl lifeguard::FromRow for UserModel {
/// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
/// # }
/// # impl lifeguard::LifeModelTrait for UserModel {
/// #     fn find() -> lifeguard::SelectQuery<Self> { todo!() }
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
/// 
/// Following SeaORM's pattern: `SelectQuery<E>` where `E: LifeModelTrait`.
/// The Entity (not Model) is the type parameter, and Model is accessed via
/// the associated type `E::Model`.
pub struct SelectQuery<E>
where
    E: LifeModelTrait,
{
    pub(crate) query: SelectStatement,  // Made pub(crate) for testing
    pub(crate) _phantom: PhantomData<E>,
}

/// Typed select query that returns a specific Model type
///
/// This is similar to SeaORM's `SelectModel<E>` and provides type-safe
/// query results. It wraps a `SelectQuery<E>` and ensures results are
/// properly typed as `M` where `M: FromRow`.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{SelectModel, LifeModelTrait, LifeExecutor};
///
/// # struct User; // Entity
/// # struct UserModel { id: i32, name: String };
/// # impl lifeguard::FromRow for UserModel {
/// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
/// # }
/// # impl lifeguard::LifeModelTrait for User {
/// #     type Model = UserModel;
/// # }
/// # let executor: &dyn LifeExecutor = todo!();
///
/// // Get typed results
/// let users: Vec<UserModel> = User::find()
///     .into_model::<UserModel>()
///     .all(executor)?;
/// ```
pub struct SelectModel<E, M>
where
    E: LifeModelTrait,
    M: FromRow,
{
    pub(crate) query: SelectQuery<E>,  // Made pub(crate) for execution module
    _model: PhantomData<M>,
}

impl<E> SelectQuery<E>
where
    E: LifeModelTrait,
{
    /// Create a new select query
    ///
    /// Following SeaORM's pattern: uses `E::default().table_name()` to get
    /// the table name, avoiding the need to pass it as a parameter.
    pub fn new() -> Self {
        let entity = E::default();
        let table_name = entity.table_name();
        let schema_name = entity.schema_name();
        
        // Use schema-qualified table name if schema is present
        use sea_query::{TableName, IntoIden, SchemaName};
        let table_ref = if let Some(schema) = schema_name {
            TableName(Some(SchemaName::from(schema)), table_name.into_iden())
        } else {
            TableName(None, table_name.into_iden())
        };
        
        let mut query = SelectStatement::default();
        query.column(sea_query::Asterisk).from(table_ref);
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
    /// Add a filter condition
    ///
    /// Accepts any type that implements `IntoCondition`, including:
    /// - `SimpleExpr` (from `Expr::column()`, `Expr::col()`, etc.)
    /// - `Condition` (from `Condition::all()`, `Condition::any()`, etc.)
    /// - `Expr` (automatically converted to `SimpleExpr` via `IntoCondition`)
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
    pub fn filter<F>(mut self, condition: F) -> Self
    where
        F: sea_query::IntoCondition,
    {
        self.query.cond_where(condition.into_condition());
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
    
    /// Add a JOIN clause (INNER JOIN)
    ///
    /// # Arguments
    ///
    /// * `table` - The table to join (must implement `Iden`)
    /// * `on` - The join condition expression
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::SelectQuery;
    /// use sea_query::{Expr, Iden};
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # struct Post; // Related entity
    /// # impl sea_query::Iden for Post {
    /// #     fn unquoted(&self) -> &str { "posts" }
    /// # }
    /// # let query = UserModel::find();
    /// let joined = query.join(Post, Expr::col("users.id").equals("posts.user_id"));
    /// ```
    pub fn join<T: Iden>(mut self, table: T, on: Expr) -> Self {
        self.query.join(sea_query::JoinType::InnerJoin, table, on);
        self
    }
    
    /// Add a LEFT JOIN clause
    ///
    /// # Arguments
    ///
    /// * `table` - The table to join (must implement `Iden`)
    /// * `on` - The join condition expression
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::SelectQuery;
    /// use sea_query::{Expr, Iden};
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # struct Post; // Related entity
    /// # impl sea_query::Iden for Post {
    /// #     fn unquoted(&self) -> &str { "posts" }
    /// # }
    /// # let query = UserModel::find();
    /// let joined = query.left_join(Post, Expr::col("users.id").equals("posts.user_id"));
    /// ```
    pub fn left_join<T: Iden>(mut self, table: T, on: Expr) -> Self {
        self.query.join(sea_query::JoinType::LeftJoin, table, on);
        self
    }
    
    /// Add a RIGHT JOIN clause
    ///
    /// # Arguments
    ///
    /// * `table` - The table to join (must implement `Iden`)
    /// * `on` - The join condition expression
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::SelectQuery;
    /// use sea_query::{Expr, Iden};
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # struct Post; // Related entity
    /// # impl sea_query::Iden for Post {
    /// #     fn unquoted(&self) -> &str { "posts" }
    /// # }
    /// # let query = UserModel::find();
    /// let joined = query.right_join(Post, Expr::col("users.id").equals("posts.user_id"));
    /// ```
    pub fn right_join<T: Iden>(mut self, table: T, on: Expr) -> Self {
        self.query.join(sea_query::JoinType::RightJoin, table, on);
        self
    }
    
    /// Add an INNER JOIN clause (alias for `join()`)
    ///
    /// # Arguments
    ///
    /// * `table` - The table to join (must implement `Iden`)
    /// * `on` - The join condition expression
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::SelectQuery;
    /// use sea_query::{Expr, Iden};
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// # struct Post; // Related entity
    /// # impl sea_query::Iden for Post {
    /// #     fn unquoted(&self) -> &str { "posts" }
    /// # }
    /// # let query = UserModel::find();
    /// let joined = query.inner_join(Post, Expr::col("users.id").equals("posts.user_id"));
    /// ```
    pub fn inner_join<T: Iden>(mut self, table: T, on: Expr) -> Self {
        self.query.join(sea_query::JoinType::InnerJoin, table, on);
        self
    }
    
    /// Add a Common Table Expression (CTE) using WITH clause
    ///
    /// CTEs allow you to define temporary named result sets that exist only for the duration of a query.
    /// **Note:** This method returns a `WithQuery` which has a different API than `SelectQuery`.
    /// You can use `with_query.select()` to continue building the query.
    ///
    /// # Arguments
    ///
    /// * `with_clause` - The WITH clause containing one or more CTEs (created with `WithClause::new().cte(...)`)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::SelectQuery;
    /// use sea_query::{Expr, Iden, SelectStatement, WithClause, CommonTableExpression};
    ///
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// struct ActiveUsers;
    /// impl sea_query::Iden for ActiveUsers {
    ///     fn unquoted(&self) -> &str { "active_users" }
    /// }
    ///
    /// // Define a CTE for active users
    /// let mut cte_query = SelectStatement::default();
    /// cte_query
    ///     .column(sea_query::Asterisk)
    ///     .from("users")
    ///     .cond_where(Expr::col("status").eq("active"));
    ///
    /// let cte = CommonTableExpression::new(ActiveUsers, cte_query);
    /// let with_clause = WithClause::new().cte(cte).to_owned();
    ///
    /// // Use the CTE in the main query
    /// # let query = UserModel::find();
    /// let with_query = query.with(with_clause);
    /// // Continue with: with_query.select(...)
    /// ```
    pub fn with(self, with_clause: sea_query::WithClause) -> sea_query::WithQuery {
        self.query.with(with_clause)
    }
    
    /// Add a subquery as a column in the SELECT clause
    ///
    /// Subqueries can be used in SELECT, WHERE, and other clauses to create nested queries.
    /// This method converts a `SelectStatement` to an `Expr` and adds it as a column.
    ///
    /// # Arguments
    ///
    /// * `subquery` - The subquery to add as a column (a `SelectStatement`)
    /// * `alias` - Optional alias for the subquery column (must implement `Iden`)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::SelectQuery;
    /// use sea_query::{Expr, Iden, SelectStatement, SubQueryStatement};
    ///
    /// # struct UserModel { id: i32, post_count: i64 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// struct PostCount;
    /// impl sea_query::Iden for PostCount {
    ///     fn unquoted(&self) -> &str { "post_count" }
    /// }
    ///
    /// // Create a subquery to count posts per user
    /// let mut subquery = SelectStatement::default();
    /// subquery
    ///     .expr(Expr::col("COUNT(*)"))
    ///     .from("posts")
    ///     .cond_where(Expr::col("posts.user_id").equals("users.id"));
    ///
    /// // Add subquery as a column
    /// # let query = UserModel::find();
    /// let query_with_subquery = query.subquery_column(subquery, Some(PostCount));
    /// ```
    pub fn subquery_column<T: Iden>(mut self, subquery: SelectStatement, alias: Option<T>) -> Self {
        // Convert SelectStatement to SubQueryStatement, then to Expr
        use sea_query::SubQueryStatement;
        let subquery_stmt = SubQueryStatement::SelectStatement(subquery);
        let expr = Expr::SubQuery(None, Box::new(subquery_stmt));
        if let Some(alias) = alias {
            self.query.expr_as(expr, alias);
        } else {
            self.query.expr(expr);
        }
        self
    }
    
    /// Add a window function expression using custom SQL
    ///
    /// Window functions perform calculations across a set of rows related to the current row.
    /// **Note:** This is a convenience method that uses `Expr::cust()` for window functions.
    /// For more complex window functions, consider using `Expr::cust()` directly.
    ///
    /// # Arguments
    ///
    /// * `window_expr` - The complete window function expression as SQL string (e.g., `"ROW_NUMBER() OVER (PARTITION BY department_id ORDER BY salary DESC)"`)
    /// * `alias` - Optional alias for the window function column (must implement `Iden`)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::SelectQuery;
    /// use sea_query::Iden;
    ///
    /// # struct UserModel { id: i32, name: String, row_num: i64 };
    /// # impl lifeguard::FromRow for UserModel {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// struct RowNumber;
    /// impl sea_query::Iden for RowNumber {
    ///     fn unquoted(&self) -> &str { "row_number" }
    /// }
    ///
    /// // Add window function to query using custom SQL
    /// # let query = UserModel::find();
    /// let query_with_window = query.window_function_cust(
    ///     "ROW_NUMBER() OVER (PARTITION BY department_id ORDER BY salary DESC)",
    ///     Some(RowNumber)
    /// );
    /// ```
    pub fn window_function_cust<T: Iden>(
        mut self,
        window_expr: &'static str,
        alias: Option<T>,
    ) -> Self {
        let expr = Expr::cust(window_expr);
        if let Some(alias) = alias {
            self.query.expr_as(expr, alias);
        } else {
            self.query.expr(expr);
        }
        self
    }
}

// SelectModel implementation methods will be added in execution module
impl<E, M> SelectModel<E, M>
where
    E: LifeModelTrait,
    M: FromRow,
{
    /// Create a new SelectModel from a SelectQuery
    pub(crate) fn new(query: SelectQuery<E>) -> Self {
        Self {
            query,
            _model: PhantomData,
        }
    }
}