//! Select query builder for `LifeModel`.
//!
//! This module provides [`SelectQuery`] and [`SelectModel`] for building and executing
//! type-safe database queries. Builder methods (`filter`, `order_by`, `limit`, …) live here;
//! execution (`all`, `one`, …) is in [`crate::query::execution`].
//!
//! # Default path vs advanced SQL (nothing is “magic”)
//!
//! Most handlers only need **`Entity::find()` → `filter` → `order_by` → `limit` →
//! [`all`](crate::query::select::SelectQuery::all)**. That path is the default; the ORM does **not**
//! inject CTEs, subquery joins, or window functions unless **you** chain the APIs below.
//!
//! **When you need a richer `SELECT`**, use these **explicit** methods (all keep [`SelectQuery`] so
//! loaders, soft-delete, and [`all`](crate::query::select::SelectQuery::all) /
//! [`one`](crate::query::select::SelectQuery::one) still work), or compose [`sea_query::Expr`] for `WHERE`:
//!
//! - **`WITH` (CTE)** — [`SelectQuery::with_cte`] (preferred). Avoid [`SelectQuery::with`] unless you
//!   intentionally want a raw [`sea_query::WithQuery`] and will hand-build SQL.
//! - **`JOIN (SELECT …)`** — [`SelectQuery::join_subquery`].
//! - **Subquery as a SELECT column** — [`SelectQuery::subquery_column`].
//! - **Window functions (`OVER`, `WINDOW`)** — [`SelectQuery::window`], [`SelectQuery::expr_window_as`],
//!   [`SelectQuery::expr_window_name_as`], or raw SQL via [`SelectQuery::window_function_cust`].
//!
//! Bring in [`sea_query`] types as needed: [`CommonTableExpression`](sea_query::CommonTableExpression),
//! [`WithClause`](sea_query::WithClause), [`WindowStatement`](sea_query::WindowStatement),
//! [`JoinType`](sea_query::JoinType), and [`ExprTrait`](sea_query::ExprTrait) for `.equals` / `.eq` on
//! expressions.

use crate::query::column::column_trait::ColumnDefHelper;
use crate::query::column::definition::get_static_expr;
use crate::query::loader::LoaderExecutor;
use crate::query::traits::{FromRow, LifeModelTrait};
use sea_query::{Expr, Iden, IntoColumnRef, Order, SelectStatement};
use std::marker::PhantomData;
use std::rc::Rc;

/// Query builder for selecting records
///
/// Returned by [`LifeModelTrait::find`]. Chain filters, ordering,
/// pagination, scopes, and (optionally) advanced SQL helpers documented in the [module
/// prelude](crate::query::select#default-path-vs-advanced-sql-nothing-is-magic).
///
/// # Example (typical)
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
/// CTEs with the same executor path: [`SelectQuery::with_cte`].
///
/// Following SeaORM-style naming: `SelectQuery<E>` where `E: LifeModelTrait`. The **entity** is the
/// type parameter; the row type is `E::Model`.
pub struct SelectQuery<E>
where
    E: LifeModelTrait,
{
    pub(crate) query: SelectStatement, // Made pub(crate) for testing
    pub(crate) with_trashed: bool,
    pub(crate) loaders: Vec<Rc<dyn LoaderExecutor<E>>>,
    pub(crate) _phantom: PhantomData<E>,
}

impl<E> Clone for SelectQuery<E>
where
    E: LifeModelTrait,
{
    fn clone(&self) -> Self {
        Self {
            query: self.query.clone(),
            with_trashed: self.with_trashed,
            loaders: self.loaders.clone(),
            _phantom: PhantomData,
        }
    }
}

/// Typed select query that returns a specific Model type
///
/// This is similar to `SeaORM`'s `SelectModel<E>` and provides type-safe
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
    pub(crate) query: SelectQuery<E>, // Made pub(crate) for execution module
    _model: PhantomData<M>,
}

impl<E> Default for SelectQuery<E>
where
    E: LifeModelTrait,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<E> SelectQuery<E>
where
    E: LifeModelTrait,
{
    /// Create a new select query
    ///
    /// Following `SeaORM`'s pattern: uses `E::default().table_name()` to get
    /// the table name, avoiding the need to pass it as a parameter.
    #[must_use]
    pub fn new() -> Self {
        use sea_query::{IntoIden, SchemaName, TableName};

        let entity = E::default();
        let table_name = entity.table_name();
        let schema_name = entity.schema_name();

        // Use schema-qualified table name if schema is present
        let table_ref = if let Some(schema) = schema_name {
            TableName(Some(SchemaName::from(schema)), table_name.into_iden())
        } else {
            TableName(None, table_name.into_iden())
        };

        let mut query = SelectStatement::default();

        // Check if any column has a select_as expression
        // If so, we need to build individual column selections instead of using Asterisk
        let columns = E::all_columns();
        let has_select_as = columns.iter().any(|col| {
            // Use ColumnDefHelper trait to access column_def() method (generated by macro)
            // ColumnTrait::def() uses blanket impl that returns ColumnDefinition::default()
            // Use fully qualified syntax to call trait method
            <E::Column as ColumnDefHelper>::column_def(*col)
                .select_as
                .is_some()
        });

        if has_select_as {
            // Build individual column selections, using select_as expressions when available
            for col in columns {
                // Use ColumnDefHelper trait to access column_def() method (generated by macro)
                // Use fully qualified syntax to call trait method
                let col_def = <E::Column as ColumnDefHelper>::column_def(*col);
                if let Some(select_expr) = col_def.select_as {
                    // Use custom SELECT expression
                    // Convert to static string using the same cache mechanism as default_expr
                    let static_str = get_static_expr(&select_expr);
                    let expr = Expr::cust(static_str);
                    query.expr(expr);
                } else {
                    // Use regular column reference (need to convert to column ref)
                    use sea_query::IntoColumnRef;
                    query.column((*col).into_column_ref());
                }
            }
        } else {
            // No select_as expressions, use Asterisk for efficiency
            query.column(sea_query::Asterisk);
        }

        query.from(table_ref);
        Self {
            query,
            with_trashed: false,
            loaders: Vec::new(),
            _phantom: PhantomData,
        }
    }

    /// Register a relation loader to automatically intercept and resolve N+1 patterns
    pub fn load<R: LifeModelTrait + 'static>(mut self, _relation: R) -> Self
    where
        E: crate::Related<R> + 'static,
        E::Model: crate::query::loader::RelationInjector<R> + crate::model::ModelTrait,
        R::Model: crate::query::traits::FromRow + crate::model::ModelTrait + Clone,
    {
        self.loaders
            .push(Rc::new(crate::query::loader::RelationLoader::<E, R>::new())
                as Rc<dyn LoaderExecutor<E>>);
        self
    }

    /// Initialize a Cursor Pagination builder anchored against a specific indexed column.
    ///
    /// Extends `SeaQuery` filtering resolving indexing dynamically
    /// to avoid `OFFSET` degradation algorithms.
    pub fn cursor_by<C: sea_query::IntoColumnRef + Clone>(
        self,
        column: C,
    ) -> crate::query::cursor::CursorPaginator<E, C> {
        crate::query::cursor::CursorPaginator::new(self, column)
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn having(mut self, condition: Expr) -> Self {
        self.query.and_having(condition);
        self
    }

    /// Allow soft-deleted records to be included in the results
    #[must_use]
    pub fn with_trashed(mut self) -> Self {
        self.with_trashed = true;
        self
    }

    /// Append soft delete filter if present and not `with_trashed`
    pub(crate) fn apply_soft_delete(mut self) -> SelectStatement {
        if !self.with_trashed {
            if let Some(col) = E::soft_delete_column() {
                use crate::query::column::column_trait::ColumnTrait;
                self.query.and_where(col.is_null());
            }
        }
        self.query
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
    /// #     fn unquoted(&self) -> &'static str { "posts" }
    /// # }
    /// # let query = UserModel::find();
    /// let joined = query.join(Post, Expr::col("users.id").equals("posts.user_id"));
    /// ```
    #[must_use]
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
    /// #     fn unquoted(&self) -> &'static str { "posts" }
    /// # }
    /// # let query = UserModel::find();
    /// let joined = query.left_join(Post, Expr::col("users.id").equals("posts.user_id"));
    /// ```
    #[must_use]
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
    /// #     fn unquoted(&self) -> &'static str { "posts" }
    /// # }
    /// # let query = UserModel::find();
    /// let joined = query.right_join(Post, Expr::col("users.id").equals("posts.user_id"));
    /// ```
    #[must_use]
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
    /// #     fn unquoted(&self) -> &'static str { "posts" }
    /// # }
    /// # let query = UserModel::find();
    /// let joined = query.inner_join(Post, Expr::col("users.id").equals("posts.user_id"));
    /// ```
    #[must_use]
    pub fn inner_join<T: Iden>(mut self, table: T, on: Expr) -> Self {
        self.query.join(sea_query::JoinType::InnerJoin, table, on);
        self
    }

    /// Attach a **`WITH`** clause while keeping this [`SelectQuery`] so [`all`](crate::query::select::SelectQuery::all),
    /// [`one`](crate::query::select::SelectQuery::one), loaders, and soft-delete still apply.
    ///
    /// Wraps [`SelectStatement::with_cte`](sea_query::SelectStatement::with_cte). Prefer this over
    /// [`Self::with`], which returns a raw [`sea_query::WithQuery`] outside the lifeguard execution
    /// API.
    ///
    /// # Example
    ///
    /// Build a [`CommonTableExpression`](sea_query::CommonTableExpression) with the `new` / `table_name` / `query`
    /// builder, wrap it in [`WithClause`](sea_query::WithClause), then pass it here.
    ///
    /// ```no_run
    /// use sea_query::{CommonTableExpression, SelectStatement, WithClause};
    ///
    /// let mut inner = SelectStatement::default();
    /// inner.column(sea_query::Asterisk).from("other_table");
    /// let mut cte = CommonTableExpression::new();
    /// cte.table_name("picked").query(inner);
    /// let wc = WithClause::new().cte(cte.to_owned()).to_owned();
    /// // Then: `MyEntity::find().with_cte(wc)` — still a `SelectQuery`; chain `.all(executor)` etc.
    /// ```
    #[must_use]
    pub fn with_cte<C>(mut self, clause: C) -> Self
    where
        C: Into<sea_query::WithClause>,
    {
        self.query.with_cte(clause);
        self
    }

    /// Join the main query to a **subquery** (`JOIN (SELECT …) AS alias ON …`).
    ///
    /// Wraps [`SelectStatement::join_subquery`](sea_query::SelectStatement::join_subquery). Use
    /// [`sea_query::ExprTrait`] (e.g. `.equals`) for the join condition.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use sea_query::SelectStatement;
    ///
    /// let mut sq = SelectStatement::default();
    /// sq.column("id").from("inner_t");
    /// // Then: `MyEntity::find().join_subquery(
    /// //     JoinType::LeftJoin,
    /// //     sq,
    /// //     "sub_a",
    /// //     Expr::col("my_table.id").equals(("sub_a", "id")),
    /// // )`
    /// ```
    #[must_use]
    pub fn join_subquery<T, C>(
        mut self,
        join: sea_query::JoinType,
        subquery: SelectStatement,
        alias: T,
        on: C,
    ) -> Self
    where
        T: sea_query::IntoIden,
        C: sea_query::IntoCondition,
    {
        self.query.join_subquery(join, subquery, alias, on);
        self
    }

    /// Define a named **`WINDOW`** clause (`WINDOW name AS (PARTITION BY …)`).
    ///
    /// Pair with [`Self::expr_window_name`] / [`Self::expr_window_name_as`], or use [`Self::expr_window`]
    /// / [`Self::expr_window_as`] for inline `OVER (…)`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use sea_query::WindowStatement;
    ///
    /// let w = WindowStatement::partition_by("grp");
    /// // Then: `MyEntity::find().window("w", w).expr_window_name_as(Expr::col("my_table.id"), "w", "rn")`
    /// ```
    #[must_use]
    pub fn window<W>(mut self, name: W, def: sea_query::WindowStatement) -> Self
    where
        W: sea_query::IntoIden,
    {
        self.query.window(name, def);
        self
    }

    /// `SELECT … OVER (window)` with an inline [`WindowStatement`](sea_query::WindowStatement) (no
    /// named `WINDOW` clause). Prefer [`Self::window`] + [`Self::expr_window_name_as`] when the same
    /// window definition is reused across several columns.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use sea_query::{Order, WindowStatement};
    ///
    /// let w = WindowStatement::partition_by("grp").order_by("ts", Order::Asc);
    /// // Then: `MyEntity::find().expr_window(Expr::col("my_table.id"), w)`
    /// ```
    #[must_use]
    pub fn expr_window<T>(mut self, expr: T, window: sea_query::WindowStatement) -> Self
    where
        T: Into<sea_query::Expr>,
    {
        self.query.expr_window(expr, window);
        self
    }

    /// `SELECT … OVER (window) AS alias` with an inline [`WindowStatement`](sea_query::WindowStatement).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use sea_query::WindowStatement;
    ///
    /// let w = WindowStatement::partition_by("grp");
    /// // Then: `MyEntity::find().expr_window_as(Expr::col("my_table.id"), w, "rn")`
    /// ```
    #[must_use]
    pub fn expr_window_as<T, A>(
        mut self,
        expr: T,
        window: sea_query::WindowStatement,
        alias: A,
    ) -> Self
    where
        T: Into<sea_query::Expr>,
        A: sea_query::IntoIden,
    {
        self.query.expr_window_as(expr, window, alias);
        self
    }

    /// `SELECT … OVER window_name` (use after [`Self::window`]).
    #[must_use]
    pub fn expr_window_name<T, W>(mut self, expr: T, window_name: W) -> Self
    where
        T: Into<sea_query::Expr>,
        W: sea_query::IntoIden,
    {
        self.query.expr_window_name(expr, window_name);
        self
    }

    /// `SELECT … OVER window_name AS alias` (use after [`Self::window`]).
    #[must_use]
    pub fn expr_window_name_as<T, W, A>(mut self, expr: T, window_name: W, alias: A) -> Self
    where
        T: Into<sea_query::Expr>,
        W: sea_query::IntoIden,
        A: sea_query::IntoIden,
    {
        self.query.expr_window_name_as(expr, window_name, alias);
        self
    }

    /// Add a Common Table Expression (CTE) using WITH clause
    ///
    /// CTEs allow you to define temporary named result sets that exist only for the duration of a query.
    /// **Note:** This method returns a [`sea_query::WithQuery`], not [`SelectQuery`]. For lifeguard
    /// execution (`all` / `one`), use [`Self::with_cte`] instead.
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
    ///     fn unquoted(&self) -> &'static str { "active_users" }
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
    /// Add a WITH clause (Common Table Expression) to the query
    ///
    /// # Arguments
    ///
    /// * `with_clause` - The `WithClause` to add
    ///
    /// # Returns
    ///
    /// Returns a `WithQuery` that can be used to continue building the query.
    #[must_use]
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
    ///     fn unquoted(&self) -> &'static str { "post_count" }
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
    #[must_use]
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
    /// use sea_query::Iden;
    ///
    /// struct RowNumber;
    /// impl sea_query::Iden for RowNumber {
    ///     fn unquoted(&self) -> &'static str { "row_number" }
    /// }
    ///
    /// // Then: `MyEntity::find().window_function_cust(
    /// //     "ROW_NUMBER() OVER (PARTITION BY department_id ORDER BY salary DESC)",
    /// //     Some(RowNumber),
    /// // )`
    /// ```
    #[must_use]
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

    /// Create a COUNT aggregation query
    ///
    /// Clears any selected columns and ordering, replaces with COUNT(*),
    /// and returns an `AggregateQuery` that resolves to a single i64 value.
    #[must_use]
    pub fn count(mut self) -> crate::query::aggregate::AggregateQuery<E, i64> {
        // Clear existing selects and orders
        self.query.clear_selects();
        self.query.clear_order_by();

        // Add COUNT(*) using a custom expression to avoid quoting '*' as a literal column name
        self.query.expr(sea_query::Expr::cust("COUNT(*)"));

        crate::query::aggregate::AggregateQuery::new(self.apply_soft_delete())
    }

    /// Create a SUM aggregation query
    ///
    /// Clears any selected columns and ordering, replaces with SUM(column),
    /// and returns an `AggregateQuery` that resolves to a single f64 value.
    pub fn sum<C: sea_query::IntoColumnRef>(
        mut self,
        column: C,
    ) -> crate::query::aggregate::AggregateQuery<E, f64> {
        use sea_query::ExprTrait;
        self.query.clear_selects();
        self.query.clear_order_by();

        self.query
            .expr(sea_query::Expr::col(column.into_column_ref()).sum());

        crate::query::aggregate::AggregateQuery::new(self.apply_soft_delete())
    }
}

// SelectModel implementation methods will be added in execution module
impl<E, M> SelectModel<E, M>
where
    E: LifeModelTrait,
    M: FromRow,
{
    /// Create a new `SelectModel` from a `SelectQuery`
    #[allow(dead_code)]
    pub(crate) fn new(query: SelectQuery<E>) -> Self {
        Self {
            query,
            _model: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::column::definition::ColumnDefinition;
    use crate::query::traits::LifeEntityName;

    // Test entity with select_as support
    #[derive(Copy, Clone, Default, Debug)]
    struct TestSelectAsEntity;

    impl LifeEntityName for TestSelectAsEntity {
        fn table_name(&self) -> &'static str {
            "test_table"
        }
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    enum TestSelectAsColumn {
        Id,
        Name,
        FullName, // This column has select_as
    }

    impl sea_query::Iden for TestSelectAsColumn {
        fn unquoted(&self) -> &'static str {
            match self {
                TestSelectAsColumn::Id => "id",
                TestSelectAsColumn::Name => "name",
                TestSelectAsColumn::FullName => "full_name",
            }
        }
    }

    impl sea_query::IdenStatic for TestSelectAsColumn {
        fn as_str(&self) -> &'static str {
            match self {
                TestSelectAsColumn::Id => "id",
                TestSelectAsColumn::Name => "name",
                TestSelectAsColumn::FullName => "full_name",
            }
        }
    }

    impl TestSelectAsColumn {
        /// Get all column variants (mimics macro-generated `all_columns()`)
        pub fn all_columns() -> &'static [TestSelectAsColumn] {
            static COLUMNS: &[TestSelectAsColumn] = &[
                TestSelectAsColumn::Id,
                TestSelectAsColumn::Name,
                TestSelectAsColumn::FullName,
            ];
            COLUMNS
        }
    }

    impl ColumnDefHelper for TestSelectAsColumn {
        fn column_def(self) -> ColumnDefinition {
            match self {
                TestSelectAsColumn::Id => ColumnDefinition {
                    column_type: Some("Integer".to_string()),
                    nullable: false,
                    ..Default::default()
                },
                TestSelectAsColumn::Name => ColumnDefinition {
                    column_type: Some("String".to_string()),
                    nullable: false,
                    ..Default::default()
                },
                TestSelectAsColumn::FullName => ColumnDefinition {
                    column_type: Some("String".to_string()),
                    nullable: false,
                    select_as: Some("CONCAT(first_name, ' ', last_name) AS full_name".to_string()),
                    ..Default::default()
                },
            }
        }
    }

    struct TestSelectAsModel;

    impl LifeModelTrait for TestSelectAsEntity {
        type Model = TestSelectAsModel;
        type Column = TestSelectAsColumn;

        fn all_columns() -> &'static [Self::Column] {
            TestSelectAsColumn::all_columns()
        }
    }

    #[test]
    fn test_select_as_uses_custom_expression() {
        // Test that when a column has select_as, it's used in the query
        let query = SelectQuery::<TestSelectAsEntity>::new();

        // Build SQL to verify select_as expression is used
        let (sql, _values) = query.query.build(sea_query::PostgresQueryBuilder);

        // Verify SQL contains the custom expression instead of just column name
        let sql_upper = sql.to_uppercase();
        assert!(
            sql_upper.contains("CONCAT"),
            "SQL should contain CONCAT expression from select_as. SQL: {sql}"
        );
        assert!(
            sql_upper.contains("FULL_NAME"),
            "SQL should contain full_name alias from select_as. SQL: {sql}"
        );

        // Verify we're not using SELECT * (should have explicit columns)
        // The SQL should have individual column selections, not just SELECT *
        assert!(
            !sql_upper.contains("SELECT *"),
            "Should not use SELECT * when select_as is present. SQL: {sql}"
        );
    }

    #[test]
    fn test_select_as_detection_works() {
        // Test that has_select_as correctly detects columns with select_as
        let columns = TestSelectAsColumn::all_columns();
        let has_select_as = columns
            .iter()
            .any(|col| col.column_def().select_as.is_some());

        assert!(
            has_select_as,
            "Should detect that at least one column has select_as"
        );
    }

    #[test]
    fn test_select_as_without_custom_expression_uses_asterisk() {
        // Test entity without select_as - should use SELECT *
        #[derive(Copy, Clone, Default, Debug)]
        struct TestNoSelectAsEntity;

        impl LifeEntityName for TestNoSelectAsEntity {
            fn table_name(&self) -> &'static str {
                "test_table"
            }
        }

        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        enum TestNoSelectAsColumn {
            Id,
            Name,
        }

        impl sea_query::Iden for TestNoSelectAsColumn {
            fn unquoted(&self) -> &'static str {
                match self {
                    TestNoSelectAsColumn::Id => "id",
                    TestNoSelectAsColumn::Name => "name",
                }
            }
        }

        impl sea_query::IdenStatic for TestNoSelectAsColumn {
            fn as_str(&self) -> &'static str {
                match self {
                    TestNoSelectAsColumn::Id => "id",
                    TestNoSelectAsColumn::Name => "name",
                }
            }
        }

        impl TestNoSelectAsColumn {
            pub fn all_columns() -> &'static [TestNoSelectAsColumn] {
                static COLUMNS: &[TestNoSelectAsColumn] =
                    &[TestNoSelectAsColumn::Id, TestNoSelectAsColumn::Name];
                COLUMNS
            }
        }

        impl ColumnDefHelper for TestNoSelectAsColumn {
            fn column_def(self) -> ColumnDefinition {
                match self {
                    TestNoSelectAsColumn::Id => ColumnDefinition {
                        column_type: Some("Integer".to_string()),
                        nullable: false,
                        ..Default::default()
                    },
                    TestNoSelectAsColumn::Name => ColumnDefinition {
                        column_type: Some("String".to_string()),
                        nullable: false,
                        ..Default::default()
                    },
                }
            }
        }

        struct TestNoSelectAsModel;

        impl LifeModelTrait for TestNoSelectAsEntity {
            type Model = TestNoSelectAsModel;
            type Column = TestNoSelectAsColumn;

            fn all_columns() -> &'static [Self::Column] {
                TestNoSelectAsColumn::all_columns()
            }
        }

        let query = SelectQuery::<TestNoSelectAsEntity>::new();
        let (sql, _values) = query.query.build(sea_query::PostgresQueryBuilder);

        // When no select_as, should use SELECT * for efficiency
        let sql_upper = sql.to_uppercase();
        // SeaQuery might format SELECT * differently, but we should not see individual columns
        // when there's no select_as. Actually, let's check that it doesn't have CONCAT
        assert!(
            !sql_upper.contains("CONCAT"),
            "Should not have CONCAT when no select_as is present. SQL: {sql}"
        );
    }

    #[test]
    fn with_cte_prepends_with_clause() {
        use sea_query::{CommonTableExpression, PostgresQueryBuilder, SelectStatement, WithClause};

        let mut inner = SelectStatement::default();
        inner.column(sea_query::Asterisk).from("cte_source");
        let mut cte = CommonTableExpression::new();
        cte.table_name("my_cte").query(inner);
        let wc = WithClause::new().cte(cte.to_owned()).to_owned();

        let q = SelectQuery::<TestSelectAsEntity>::new().with_cte(wc);
        let (sql, _) = q.query.build(PostgresQueryBuilder);
        let upper = sql.to_uppercase();
        assert!(
            upper.starts_with("WITH"),
            "expected WITH prefix, got: {sql}"
        );
    }

    #[test]
    fn join_subquery_emits_subselect_join() {
        use sea_query::{ExprTrait, JoinType, PostgresQueryBuilder, SelectStatement};

        let mut sq = SelectStatement::default();
        sq.column("id").from("inner_t");

        let q = SelectQuery::<TestSelectAsEntity>::new().join_subquery(
            JoinType::LeftJoin,
            sq,
            "sub_a",
            sea_query::Expr::col("test_table.id").equals(("sub_a", "id")),
        );
        let (sql, _) = q.query.build(PostgresQueryBuilder);
        let upper = sql.to_uppercase();
        assert!(upper.contains("LEFT JOIN"), "SQL: {sql}");
        assert!(upper.contains("SUB_A"), "SQL: {sql}");
    }

    #[test]
    fn window_clause_and_expr_window_name_as() {
        use sea_query::{Expr, PostgresQueryBuilder, WindowStatement};

        let w = WindowStatement::partition_by("grp");
        let q = SelectQuery::<TestSelectAsEntity>::new()
            .window("w", w)
            .expr_window_name_as(Expr::col("test_table.id"), "w", "rn");
        let (sql, _) = q.query.build(PostgresQueryBuilder);
        let upper = sql.to_uppercase();
        assert!(upper.contains("WINDOW"), "SQL: {sql}");
        assert!(upper.contains("OVER"), "SQL: {sql}");
    }

    #[test]
    fn test_count_query_asterisk_syntax() {
        use sea_query::PostgresQueryBuilder;

        // Ensure that calling .count() on a query correctly builds the generic COUNT(*) block
        // Without literal `"table"."*"` quoting that throws `column does not exist`.
        let q = SelectQuery::<TestSelectAsEntity>::new();
        let count_agg = q.count();

        let (sql, _) = count_agg.query.build(PostgresQueryBuilder);

        // Assert it explicitly does not have individual columns anymore
        assert!(!sql.contains("id"), "SQL: {sql}");
        // Assert Asterisk literal formulation
        assert!(
            sql.contains("COUNT(*)"),
            "SQL should have raw unquoted asterisk. SQL: {sql}"
        );
    }
}
