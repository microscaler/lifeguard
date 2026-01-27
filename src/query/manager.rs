//! Model Manager pattern for custom query methods.
//!
//! Model Managers provide a way to add custom query methods to entities, similar to
//! Django's Model Managers. This allows you to encapsulate common query patterns,
//! stored procedure wrappers, and business logic in reusable methods.
//!
//! # Usage
//!
//! ## Basic Pattern: Custom Methods on Entity
//!
//! ```no_run
//! use lifeguard::{LifeModelTrait, LifeExecutor, LifeError};
//! use sea_query::Expr;
//!
//! # struct User;
//! # struct UserModel { id: i32, email: String, is_active: bool };
//! # impl lifeguard::FromRow for UserModel {
//! #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
//! # }
//! # impl lifeguard::LifeModelTrait for User {
//! #     type Model = UserModel;
//! #     type Column = ();
//! # }
//!
//! // Add custom query methods to your Entity
//! impl User {
//!     /// Find active users
//!     pub fn find_active(executor: &dyn LifeExecutor) -> Result<Vec<UserModel>, LifeError> {
//!         User::find()
//!             .filter(Expr::col("is_active").eq(true))
//!             .all(executor)
//!     }
//!
//!     /// Find user by email
//!     pub fn find_by_email(executor: &dyn LifeExecutor, email: &str) -> Result<Option<UserModel>, LifeError> {
//!         User::find()
//!             .filter(Expr::col("email").eq(email))
//!             .find_one(executor)
//!     }
//!
//!     /// Count active users
//!     pub fn count_active(executor: &dyn LifeExecutor) -> Result<usize, LifeError> {
//!         User::find()
//!             .filter(Expr::col("is_active").eq(true))
//!             .count(executor)
//!     }
//! }
//! ```
//!
//! ## Stored Procedure Wrappers
//!
//! ```no_run
//! use lifeguard::{LifeModelTrait, LifeExecutor, LifeError, find_by_statement, FromRow};
//!
//! # struct User;
//! # struct UserModel { id: i32 };
//! # impl lifeguard::FromRow for UserModel {
//! #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
//! # }
//!
//! impl User {
//!     /// Call stored procedure to get user statistics
//!     pub fn get_stats(executor: &dyn LifeExecutor, user_id: i32) -> Result<UserStats, LifeError> {
//!         let row = find_by_statement(
//!             executor,
//!             "SELECT * FROM get_user_stats($1)",
//!             &[&(user_id as i64)]
//!         )?;
//!         UserStats::from_row(&row)
//!             .map_err(|e| LifeError::ExecutionError(e.to_string()))
//!     }
//!
//!     /// Call stored procedure to refresh cache
//!     pub fn refresh_cache(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
//!         lifeguard::execute_statement(executor, "CALL refresh_user_cache()", &[])?;
//!         Ok(())
//!     }
//! }
//! ```
//!
//! ## Using `ModelManager` Trait (Optional)
//!
//! The `ModelManager` trait provides common query patterns that you can implement
//! for your entities. This is optional - you can also just add custom methods directly.
//!
//! ```no_run
//! use lifeguard::query::manager::ModelManager;
//! use lifeguard::{LifeModelTrait, LifeExecutor, LifeError};
//!
//! # struct User;
//! # struct UserModel { id: i32, email: String };
//! # impl lifeguard::LifeModelTrait for User {
//! #     type Model = UserModel;
//! #     type Column = ();
//! # }
//!
//! impl ModelManager for User {
//!     type Model = UserModel;
//!
//!     fn find_by_id(executor: &dyn LifeExecutor, id: i64) -> Result<Option<Self::Model>, LifeError> {
//!         Self::find()
//!             .filter(sea_query::Expr::col("id").eq(id))
//!             .find_one(executor)
//!     }
//! }
//! ```

use crate::executor::{LifeExecutor, LifeError};
use crate::query::traits::{LifeModelTrait, FromRow};
use may_postgres::types::ToSql;

/// Trait for Model Manager patterns.
///
/// This trait provides common query patterns that entities can implement.
/// It's optional - you can also add custom methods directly to your Entity impl blocks.
///
/// # Example
///
/// ```no_run
/// use lifeguard::query::manager::ModelManager;
/// use lifeguard::{LifeModelTrait, LifeExecutor, LifeError};
///
/// # struct User;
/// # struct UserModel { id: i32, email: String };
/// # impl lifeguard::LifeModelTrait for User {
/// #     type Model = UserModel;
/// #     type Column = ();
/// # }
///
/// impl ModelManager for User {
///     type Model = UserModel;
///
///     fn find_by_id(executor: &dyn LifeExecutor, id: i64) -> Result<Option<Self::Model>, LifeError> {
///         Self::find()
///             .filter(sea_query::Expr::col("id").eq(id))
///             .find_one(executor)
///     }
/// }
/// ```
pub trait ModelManager: LifeModelTrait
where
    <Self as LifeModelTrait>::Model: FromRow,
{

    /// Find a model by its primary key ID.
    ///
    /// This is a convenience method that assumes the primary key column is named "id".
    /// Implement this method for your specific primary key type.
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor
    /// * `id` - The primary key value
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(model))` if found, `Ok(None)` if not found, or an error.
    ///
    /// # Example Implementation
    ///
    /// ```no_run
    /// # use lifeguard::query::manager::ModelManager;
    /// # use lifeguard::{LifeModelTrait, LifeExecutor, LifeError};
    /// # struct User;
    /// # struct UserModel { id: i32 };
    /// # impl lifeguard::LifeModelTrait for User {
    /// #     type Model = UserModel;
    /// #     type Column = ();
    /// # }
    /// impl ModelManager for User {
    ///     type Model = UserModel;
    ///     
    ///     fn find_by_id<Ex: LifeExecutor>(executor: &Ex, id: i64) -> Result<Option<Self::Model>, LifeError> {
    ///         Self::find()
    ///             .filter(sea_query::Expr::col("id").eq(id as i32))
    ///             .find_one(executor)
    ///     }
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if the query execution or row parsing fails.
    fn find_by_id<Ex: LifeExecutor>(executor: &Ex, id: i64) -> Result<Option<<Self as LifeModelTrait>::Model>, LifeError>;

    /// Find all models matching a condition.
    ///
    /// This is a convenience method that wraps `find().filter().all()`.
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor
    /// * `condition` - A condition expression (must implement `IntoCondition`)
    ///
    /// # Returns
    ///
    /// Returns a vector of models matching the condition.
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if the query execution or row parsing fails.
    fn find_where<Ex: LifeExecutor, C: sea_query::IntoCondition>(
        executor: &Ex,
        condition: C,
    ) -> Result<Vec<<Self as LifeModelTrait>::Model>, LifeError> {
        Self::find()
            .filter(condition)
            .all(executor)
    }

    /// Count models matching a condition.
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor
    /// * `condition` - A condition expression (must implement `IntoCondition`)
    ///
    /// # Returns
    ///
    /// Returns the count of models matching the condition.
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if the query execution fails.
    fn count_where<Ex: LifeExecutor, C: sea_query::IntoCondition>(
        executor: &Ex,
        condition: C,
    ) -> Result<usize, LifeError> {
        Self::find()
            .filter(condition)
            .count(executor)
    }

    /// Check if any models exist matching a condition.
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor
    /// * `condition` - A condition expression (must implement `IntoCondition`)
    ///
    /// # Returns
    ///
    /// Returns `true` if at least one model matches, `false` otherwise.
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if the query execution fails.
    fn exists_where<Ex: LifeExecutor, C: sea_query::IntoCondition>(
        executor: &Ex,
        condition: C,
    ) -> Result<bool, LifeError> {
        Self::count_where(executor, condition).map(|count| count > 0)
    }
}

/// Helper trait for stored procedure wrappers.
///
/// This trait provides convenience methods for calling stored procedures and
/// mapping their results to models. It's optional - you can also use raw SQL
/// methods directly.
///
/// # Example
///
/// ```no_run
/// use lifeguard::query::manager::StoredProcedure;
/// use lifeguard::{LifeExecutor, LifeError, FromRow};
///
/// # struct UserStats { user_id: i32, post_count: i64 };
/// # impl lifeguard::FromRow for UserStats {
/// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
/// # }
///
/// impl StoredProcedure for UserStats {
///     /// Call stored procedure to get user statistics
///     fn call_get_stats(executor: &dyn LifeExecutor, user_id: i32) -> Result<Self, LifeError> {
///         let row = lifeguard::find_by_statement(
///             executor,
///             "SELECT * FROM get_user_stats($1)",
///             &[&(user_id as i64)]
///         )?;
///         Self::from_row(&row)
///             .map_err(|e| LifeError::QueryError(e.to_string()))
///     }
/// }
/// ```
pub trait StoredProcedure: FromRow {
    /// Call a stored procedure that returns a single row.
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor
    /// * `sql` - The SQL statement to execute (typically a stored procedure call)
    /// * `params` - Parameters to bind to the statement
    ///
    /// # Returns
    ///
    /// Returns the model mapped from the procedure result.
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if:
    /// - The procedure execution fails
    /// - No rows are returned
    /// - Multiple rows are returned
    /// - Row parsing fails
    fn call_procedure<Ex: LifeExecutor>(
        executor: &Ex,
        sql: &str,
        params: &[&dyn ToSql],
    ) -> Result<Self, LifeError> {
        let row = crate::find_by_statement(executor, sql, params)?;
        Self::from_row(&row)
            .map_err(|e| LifeError::QueryError(e.to_string()))
    }

    /// Call a stored procedure that returns multiple rows.
    ///
    /// # Arguments
    ///
    /// * `executor` - The database executor
    /// * `sql` - The SQL statement to execute (typically a stored procedure call)
    /// * `params` - Parameters to bind to the statement
    ///
    /// # Returns
    ///
    /// Returns a vector of models mapped from the procedure results.
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if the procedure execution or row parsing fails.
    fn call_procedure_many<Ex: LifeExecutor>(
        executor: &Ex,
        sql: &str,
        params: &[&dyn ToSql],
    ) -> Result<Vec<Self>, LifeError> {
        let rows = crate::find_all_by_statement(executor, sql, params)?;
        rows.iter()
            .map(|row| Self::from_row(row))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| LifeError::QueryError(e.to_string()))
    }
}
