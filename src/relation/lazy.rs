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

    #[test]
    fn test_lazy_loader_composite_key() {
        // Test that LazyLoader works with composite key entities
        use sea_query::{TableName, IntoIden, TableRef, ConditionType};
        use crate::relation::def::{RelationDef, RelationType};
        
        #[derive(Default, Copy, Clone)]
        struct TenantEntity;
        
        impl sea_query::Iden for TenantEntity {
            fn unquoted(&self) -> &str { "tenants" }
        }
        
        impl crate::LifeEntityName for TenantEntity {
            fn table_name(&self) -> &'static str { "tenants" }
        }
        
        impl crate::LifeModelTrait for TenantEntity {
            type Model = TenantModel;
            type Column = TenantColumn;
        }
        
        #[derive(Default, Copy, Clone)]
        struct UserEntity;
        
        impl sea_query::Iden for UserEntity {
            fn unquoted(&self) -> &str { "users" }
        }
        
        impl crate::LifeEntityName for UserEntity {
            fn table_name(&self) -> &'static str { "users" }
        }
        
        impl crate::LifeModelTrait for UserEntity {
            type Model = UserModel;
            type Column = UserColumn;
        }
        
        #[derive(Clone, Debug)]
        struct TenantModel { id: i32, tenant_id: i32 }
        #[derive(Clone, Debug)]
        struct UserModel { id: i32, tenant_id: i32 }
        
        #[derive(Copy, Clone, Debug)]
        enum TenantColumn { Id, TenantId }
        
        impl sea_query::Iden for TenantColumn {
            fn unquoted(&self) -> &str {
                match self {
                    TenantColumn::Id => "id",
                    TenantColumn::TenantId => "tenant_id",
                }
            }
        }
        
        impl sea_query::IdenStatic for TenantColumn {
            fn as_str(&self) -> &'static str {
                match self {
                    TenantColumn::Id => "id",
                    TenantColumn::TenantId => "tenant_id",
                }
            }
        }
        
        #[derive(Copy, Clone, Debug)]
        enum UserColumn { Id, TenantId }
        
        impl sea_query::Iden for UserColumn {
            fn unquoted(&self) -> &str {
                match self {
                    UserColumn::Id => "id",
                    UserColumn::TenantId => "tenant_id",
                }
            }
        }
        
        impl sea_query::IdenStatic for UserColumn {
            fn as_str(&self) -> &'static str {
                match self {
                    UserColumn::Id => "id",
                    UserColumn::TenantId => "tenant_id",
                }
            }
        }
        
        impl crate::query::traits::FromRow for UserModel {
            fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(UserModel { id: 0, tenant_id: 0 })
            }
        }
        
        impl ModelTrait for TenantModel {
            type Entity = TenantEntity;
            fn get(&self, col: TenantColumn) -> sea_query::Value {
                match col {
                    TenantColumn::Id => sea_query::Value::Int(Some(self.id)),
                    TenantColumn::TenantId => sea_query::Value::Int(Some(self.tenant_id)),
                }
            }
            fn set(&mut self, _col: TenantColumn, _val: sea_query::Value) -> Result<(), crate::model::ModelError> { todo!() }
            fn get_primary_key_value(&self) -> sea_query::Value {
                sea_query::Value::Int(Some(self.id))
            }
            fn get_primary_key_identity(&self) -> Identity {
                Identity::Binary("id".into(), "tenant_id".into())
            }
            fn get_primary_key_values(&self) -> Vec<sea_query::Value> {
                vec![
                    sea_query::Value::Int(Some(self.id)),
                    sea_query::Value::Int(Some(self.tenant_id)),
                ]
            }
            fn get_by_column_name(&self, column_name: &str) -> Option<sea_query::Value> {
                match column_name {
                    "id" => Some(sea_query::Value::Int(Some(self.id))),
                    "tenant_id" => Some(sea_query::Value::Int(Some(self.tenant_id))),
                    _ => None,
                }
            }
        }
        
        impl Related<UserEntity> for TenantEntity {
            fn to() -> RelationDef {
                RelationDef {
                    rel_type: RelationType::HasMany,
                    from_tbl: sea_query::TableRef::Table(TableName(None, "tenants".into_iden()), None),
                    to_tbl: sea_query::TableRef::Table(TableName(None, "users".into_iden()), None),
                    from_col: Identity::Binary("id".into(), "tenant_id".into()),
                    to_col: Identity::Binary("id".into(), "tenant_id".into()),
                    through_tbl: None,
                    through_from_col: None,
                    through_to_col: None,
                    is_owner: true,
                    skip_fk: false,
                    on_condition: None,
                    condition_type: ConditionType::All,
                }
            }
        }
        
        let tenant = TenantModel { id: 1, tenant_id: 10 };
        
        // Verify LazyLoader can be created with composite key entity
        fn _test_composite_key<'a, M: ModelTrait, Ex: LifeExecutor>(
            entity: &'a M,
            executor: &'a Ex,
        ) -> LazyLoader<'a, M, Ex> {
            LazyLoader::new(entity, executor)
        }
        
        // Just verify it compiles - actual execution test would need executor setup
        let _ = tenant;
    }
}
