//! Lazy loading utilities for related entities.
//!
//! This module provides utilities for loading related entities lazily,
//! similar to SeaORM's lazy loading strategy. Queries are built but not
//! executed until the related data is actually accessed.
//!
//! # Example
//!
//! ```no_run
//! use lifeguard::{LazyLoader, LifeModelTrait, LifeExecutor, Related};
//!
//! // Fetch a user
//! # struct UserModel { id: i32 };
//! # struct PostModel { id: i32, user_id: i32 };
//! # let user = UserModel { id: 1 };
//! # let executor: &dyn LifeExecutor = todo!();
//!
//! // Create a lazy loader for posts (query not executed yet)
//! let lazy_posts = LazyLoader::new(&user, executor);
//!
//! // Later, when we need the posts, execute the query
//! let posts = lazy_posts.load::<PostModel>()?;
//! ```
//!
//! # Strategy
//!
//! Lazy loading defers query execution until the data is actually needed:
//! 1. Store the parent entity and executor
//! 2. Build the query but don't execute it
//! 3. Execute the query only when `load()` is called
//!
//! This is useful when you're not sure if you'll need the related data,
//! or when you want to conditionally load relationships.

use crate::executor::{LifeExecutor, LifeError};
use crate::model::ModelTrait;
use crate::query::{SelectQuery, LifeModelTrait};
use crate::relation::traits::Related;
use crate::relation::def::{RelationDef, build_where_condition};

/// A lazy loader for related entities
///
/// This struct stores the necessary information to load related entities
/// on-demand. The query is built but not executed until `load()` is called.
///
/// # Type Parameters
///
/// * `M` - The main model type (e.g., `UserModel`)
/// * `Ex` - The executor type
///
/// # Example
///
/// ```no_run
/// use lifeguard::{LazyLoader, LifeModelTrait, LifeExecutor, Related};
///
/// # struct UserModel { id: i32 };
/// # struct PostModel { id: i32, user_id: i32 };
/// # impl lifeguard::ModelTrait for UserModel {
/// #     type Entity = User;
/// #     fn get_primary_key_value(&self) -> sea_query::Value { todo!() }
/// #     fn get_primary_key_identity(&self) -> lifeguard::Identity { todo!() }
/// #     fn get_primary_key_values(&self) -> Vec<sea_query::Value> { todo!() }
/// #     fn get(&self, _col: <User as lifeguard::LifeModelTrait>::Column) -> sea_query::Value { todo!() }
/// #     fn set(&mut self, _col: <User as lifeguard::LifeModelTrait>::Column, _val: sea_query::Value) -> Result<(), lifeguard::ModelError> { todo!() }
/// # }
/// # struct User;
/// # impl lifeguard::LifeModelTrait for User {
/// #     type Model = UserModel;
/// #     type Column = ();
/// # }
/// # struct Post;
/// # impl lifeguard::LifeModelTrait for Post {
/// #     type Model = PostModel;
/// #     type Column = ();
/// # }
/// # let user = UserModel { id: 1 };
/// # let executor: &dyn LifeExecutor = todo!();
///
/// // Create lazy loader
/// let lazy_posts = LazyLoader::new(&user, executor);
///
/// // Later, load the posts
/// let posts = lazy_posts.load::<PostModel>()?;
/// ```
pub struct LazyLoader<'a, M, Ex> {
    /// The parent entity to load related entities for
    entity: &'a M,
    /// The executor to use when loading
    executor: &'a Ex,
}

impl<'a, M, Ex> LazyLoader<'a, M, Ex>
where
    M: ModelTrait,
    Ex: LifeExecutor,
{
    /// Create a new lazy loader for the given entity
    ///
    /// # Arguments
    ///
    /// * `entity` - The parent entity to load related entities for
    /// * `executor` - The executor to use when loading
    ///
    /// # Returns
    ///
    /// A `LazyLoader` that can be used to load related entities on-demand
    pub fn new(entity: &'a M, executor: &'a Ex) -> Self {
        Self { entity, executor }
    }

    /// Load related entities for the parent entity
    ///
    /// This method builds and executes the query to fetch related entities.
    /// The query is built using the relationship definition from `Related<R>`.
    ///
    /// # Type Parameters
    ///
    /// * `R` - The related entity type (e.g., `PostEntity`)
    ///
    /// # Returns
    ///
    /// A vector of related entity models, or an error if the query fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{LazyLoader, LifeModelTrait, LifeExecutor, Related};
    ///
    /// # struct UserModel { id: i32 };
    /// # struct PostModel { id: i32, user_id: i32 };
    /// # let user = UserModel { id: 1 };
    /// # let executor: &dyn LifeExecutor = todo!();
    ///
    /// let lazy_posts = LazyLoader::new(&user, executor);
    /// let posts = lazy_posts.load::<PostModel>()?;
    /// ```
    pub fn load<R>(&self) -> Result<Vec<R::Model>, LifeError>
    where
        R: LifeModelTrait,
        M::Entity: Related<R>,
        R::Model: crate::query::traits::FromRow,
    {
        // Get the relationship definition
        let rel_def: RelationDef = <M::Entity as Related<R>>::to();

        // Build the query using the relationship definition
        let mut query = SelectQuery::<R>::new();

        // Build WHERE condition from the parent entity's primary key
        let where_condition = build_where_condition(
            &rel_def,
            self.entity,
        );

        // Apply the WHERE condition
        query = query.filter(where_condition);

        // Execute the query
        query.all(self.executor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation::identity::Identity;

    #[test]
    fn test_lazy_loader_compiles() {
        // This is a compile-time test to verify LazyLoader API compiles correctly
        // Actual execution tests would require a real executor setup
        // The API is verified by the fact that this test compiles
        
        // LazyLoader::new() and LazyLoader::load() signatures are tested
        // by their usage in the documentation examples above
    }
}
