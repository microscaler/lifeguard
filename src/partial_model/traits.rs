//! Core traits for partial model operations.
//!
//! This module provides `PartialModelTrait` and `PartialModelBuilder` for selecting
//! subsets of columns from database queries.

use crate::query::{LifeModelTrait, FromRow};
use super::query::SelectPartialQuery;

/// Trait for partial models that represent a subset of columns
///
/// Partial models allow you to select only specific columns from a query,
/// reducing data transfer and improving performance. This is especially useful
/// for large tables where you only need a few fields.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{PartialModelTrait, LifeModelTrait, SelectQuery, LifeExecutor};
///
/// // Define a partial model that only includes id and name
/// struct UserPartial {
///     id: i32,
///     name: String,
/// }
///
/// impl PartialModelTrait for UserPartial {
///     type Entity = User;
///     
    ///     fn selected_columns() -> Vec<&'static str> {
    ///         vec!["id", "name"]
    ///     }
/// }
///
/// impl lifeguard::FromRow for UserPartial {
///     fn from_row(row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
///         Ok(UserPartial {
///             id: row.get(0)?,
///             name: row.get(1)?,
///         })
///     }
/// }
///
/// // Use it in a query
/// # struct User;
/// # impl lifeguard::LifeModelTrait for User {
/// #     type Model = ();
/// # }
/// # let executor: &dyn LifeExecutor = todo!();
/// let users: Vec<UserPartial> = User::find()
///     .select_partial::<UserPartial>()
///     .all(executor)?;
/// ```
pub trait PartialModelTrait: FromRow {
    /// The Entity type that this partial model belongs to
    type Entity: LifeModelTrait;
    
    /// Get the list of columns that should be selected for this partial model
    ///
    /// This method returns a vector of column names that should be selected.
    /// The order of columns should match the order in which they are read in
    /// the `FromRow` implementation.
    ///
    /// # Returns
    ///
    /// A vector of column names as static string slices
    ///
    /// # Example
    ///
    /// ```no_run
    /// impl PartialModelTrait for UserPartial {
    ///     type Entity = User;
    ///     
    ///     fn selected_columns() -> Vec<&'static str> {
    ///         vec!["id", "name"]
    ///     }
    /// }
    /// ```
    fn selected_columns() -> Vec<&'static str>
    where
        Self: Sized;
}

/// Helper trait for building partial model queries
///
/// This trait provides methods to build queries that return partial models.
pub trait PartialModelBuilder<E: LifeModelTrait> {
    /// Select specific columns for a partial model
    ///
    /// This method configures the query to select only the columns required
    /// by the partial model type `P`.
    ///
    /// # Type Parameters
    ///
    /// * `P` - The partial model type that implements `PartialModelTrait`
    ///
    /// # Returns
    ///
    /// Returns a `SelectPartialQuery` that can be executed to get partial models
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{SelectQuery, PartialModelTrait, LifeModelTrait};
    ///
    /// # struct User;
    /// # impl lifeguard::LifeModelTrait for User {
    /// #     type Model = ();
    /// # }
    /// # struct UserPartial { id: i32 };
    /// # impl lifeguard::PartialModelTrait for UserPartial {
    /// #     type Entity = User;
    /// #     fn selected_columns() -> Vec<sea_query::Expr> { vec![] }
    /// # }
    /// # impl lifeguard::FromRow for UserPartial {
    /// #     fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> { todo!() }
    /// # }
    /// let query = User::find().select_partial::<UserPartial>();
    /// ```
    fn select_partial<P: PartialModelTrait<Entity = E>>(self) -> SelectPartialQuery<E, P>;
}
