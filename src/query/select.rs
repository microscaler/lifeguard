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