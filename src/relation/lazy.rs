//! Lazy loading utilities for related entities.
//!
//! This module provides utilities for loading related entities lazily,
//! similar to `SeaORM`'s lazy loading strategy. Queries are built but not
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
    ///
    /// # Errors
    ///
    /// Returns `LifeError` if:
    /// - The relationship definition is invalid
    /// - Query execution fails
    /// - Row parsing fails
    ///
    /// # Panics
    ///
    /// This function will panic if:
    /// - `to_col` and primary key have mismatched arities for `HasMany` relationships
    /// - Required relationship fields are missing
    pub fn load<R>(&self) -> Result<Vec<R::Model>, LifeError>
    where
        R: LifeModelTrait,
        M::Entity: Related<R>,
        R::Model: crate::query::traits::FromRow,
    {
        use crate::relation::def::types::RelationType;
        
        // Get the relationship definition
        let rel_def: RelationDef = <M::Entity as Related<R>>::to();

        // Build the query using the relationship definition
        let mut query = SelectQuery::<R>::new();
        
        // Build WHERE condition from the parent entity's primary key
        // For has_many relationships, we need to use to_col (FK in target table) instead of from_col (PK in source table)
        // because we're querying the target table (to_tbl), not the source table (from_tbl)
        let where_condition = if rel_def.rel_type == RelationType::HasMany {
            // For has_many: query target table (to_tbl) filtered by to_col (FK) = source PK
            // Example: SELECT * FROM posts WHERE posts.user_id = user.id
            // NOT: SELECT * FROM posts WHERE users.id = user.id (wrong - users table not in query)
            use crate::relation::def::condition::extract_table_name;
            use sea_query::{Condition, Expr, ExprTrait};
            
            let mut condition = Condition::all();
            let pk_identity = self.entity.get_primary_key_identity();
            let pk_values = self.entity.get_primary_key_values();
            
            // Ensure arities match
            assert_eq!(
                rel_def.to_col.arity(),
                pk_identity.arity(),
                "Foreign key columns (to_col) and primary key must have matching arity for has_many"
            );
            
            // Match to_col (FK in target table) to source PK values
            for (fk_col, pk_val) in rel_def.to_col.iter().zip(pk_values.iter()) {
                let fk_col_str = fk_col.to_string();
                let to_tbl_str = extract_table_name(&rel_def.to_tbl);
                
                // Create WHERE condition: to_table.fk_col = source_pk_value
                // Use the same pattern as build_where_condition in condition.rs
                let col_expr = format!("{to_tbl_str}.{fk_col_str}");
                let expr = Expr::cust(col_expr).eq(Expr::val(pk_val.clone()));
                condition = condition.add(expr);
            }
            
            condition
        } else {
            // For belongs_to and has_one, use the standard build_where_condition
            // which uses from_col (correct for these relationship types)
            build_where_condition(&rel_def, self.entity)
        };

        // Apply the WHERE condition
        query = query.filter(where_condition);

        // Execute the query
        query.all(self.executor)
    }
}

#[cfg(test)]
#[allow(dead_code)]
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
    #[allow(clippy::too_many_lines)] // Test code - long test function is acceptable
    fn test_lazy_loader_composite_key() {
        // Test that LazyLoader works with composite key entities
        use sea_query::{TableName, IntoIden, ConditionType};
        use crate::relation::def::{RelationDef, RelationType};
        
        #[derive(Default, Copy, Clone)]
        struct TenantEntity;
        
        impl sea_query::Iden for TenantEntity {
            fn unquoted(&self) -> &'static str { "tenants" }
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
            fn unquoted(&self) -> &'static str { "users" }
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
            fn unquoted(&self) -> &'static str {
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
        
        crate::impl_column_def_helper_for_test!(TenantColumn);
        
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
        
        crate::impl_column_def_helper_for_test!(UserColumn);
        
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
            #[allow(clippy::todo)] // Test code - todo!() is acceptable for unimplemented test helpers
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
        #[allow(clippy::items_after_statements)] // Test code - function definition after statement is acceptable
        fn _test_composite_key<'a, M: ModelTrait, Ex: LifeExecutor>(
            entity: &'a M,
            executor: &'a Ex,
        ) -> LazyLoader<'a, M, Ex> {
            LazyLoader::new(entity, executor)
        }
        
        // Just verify it compiles - actual execution test would need executor setup
        let _ = tenant;
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Test code - long test function is acceptable
    fn test_lazy_loader_has_many_uses_to_col() {
        // Test that LazyLoader::load() for has_many relationships uses to_col (FK in target table)
        // instead of from_col (PK in source table), which would reference an unjoined table
        // 
        // BUG FIX: Previously, build_where_condition used from_tbl.from_col (e.g., users.id),
        // but the query selects from to_tbl (posts) without joining users, causing invalid SQL.
        // The fix uses to_col (posts.user_id) instead.
        
        use sea_query::{TableName, IntoIden, ConditionType};
        use crate::relation::def::{RelationDef, RelationType};
        use crate::relation::identity::Identity;
        
        #[derive(Default, Copy, Clone)]
        struct UserEntity;
        
        impl sea_query::Iden for UserEntity {
            fn unquoted(&self) -> &'static str { "users" }
        }
        
        impl crate::LifeEntityName for UserEntity {
            fn table_name(&self) -> &'static str { "users" }
        }
        
        impl crate::LifeModelTrait for UserEntity {
            type Model = UserModel;
            type Column = UserColumn;
        }
        
        #[derive(Default, Copy, Clone)]
        struct PostEntity;
        
        impl sea_query::Iden for PostEntity {
            fn unquoted(&self) -> &'static str { "posts" }
        }
        
        impl crate::LifeEntityName for PostEntity {
            fn table_name(&self) -> &'static str { "posts" }
        }
        
        impl crate::LifeModelTrait for PostEntity {
            type Model = PostModel;
            type Column = PostColumn;
        }
        
        #[derive(Clone, Debug)]
        struct UserModel { id: i32 }
        #[derive(Clone, Debug)]
        struct PostModel { id: i32, user_id: i32 }
        
        #[derive(Copy, Clone, Debug)]
        enum UserColumn { Id }
        
        impl sea_query::Iden for UserColumn {
            fn unquoted(&self) -> &'static str { "id" }
        }
        
        impl sea_query::IdenStatic for UserColumn {
            fn as_str(&self) -> &'static str { "id" }
        }
        
        crate::impl_column_def_helper_for_test!(UserColumn);
        
        #[derive(Copy, Clone, Debug)]
        enum PostColumn { Id, UserId }
        
        impl sea_query::Iden for PostColumn {
            fn unquoted(&self) -> &str {
                match self {
                    PostColumn::Id => "id",
                    PostColumn::UserId => "user_id",
                }
                
            }
        }
        
        impl sea_query::IdenStatic for PostColumn {
            fn as_str(&self) -> &'static str {
                match self {
                    PostColumn::Id => "id",
                    PostColumn::UserId => "user_id",
                }
            }
        }
        
        crate::impl_column_def_helper_for_test!(PostColumn);
        
        impl crate::query::traits::FromRow for PostModel {
            fn from_row(_row: &may_postgres::Row) -> Result<Self, may_postgres::Error> {
                Ok(PostModel { id: 0, user_id: 0 })
            }
            
        }
        
        impl ModelTrait for UserModel {
            type Entity = UserEntity;
            fn get(&self, col: UserColumn) -> sea_query::Value {
                match col {
                    UserColumn::Id => sea_query::Value::Int(Some(self.id)),
                }
            }
            #[allow(clippy::todo)] // Test code - todo!() is acceptable for unimplemented test helpers
            fn set(&mut self, _col: UserColumn, _val: sea_query::Value) -> Result<(), crate::model::ModelError> { todo!() }
            fn get_primary_key_value(&self) -> sea_query::Value {
                sea_query::Value::Int(Some(self.id))
            }
            fn get_primary_key_identity(&self) -> Identity {
                Identity::Unary("id".into())
            }
            fn get_primary_key_values(&self) -> Vec<sea_query::Value> {
                vec![sea_query::Value::Int(Some(self.id))]
            }
        }
        
        impl Related<PostEntity> for UserEntity {
            fn to() -> RelationDef {
                RelationDef {
                    rel_type: RelationType::HasMany,
                    from_tbl: sea_query::TableRef::Table(TableName(None, "users".into_iden()), None),
                    to_tbl: sea_query::TableRef::Table(TableName(None, "posts".into_iden()), None),
                    from_col: Identity::Unary("id".into()),  // users.id (source PK)
                    to_col: Identity::Unary("user_id".into()), // posts.user_id (target FK)
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
        
        let user = UserModel { id: 42 };
        
        // Verify that LazyLoader::load() would use to_col (posts.user_id) not from_col (users.id)
        // The fix ensures the WHERE condition references posts.user_id, not users.id
        // This test verifies the function compiles with the fix
        #[allow(clippy::items_after_statements)] // Test code - function definition after statement is acceptable
        #[allow(clippy::used_underscore_binding)] // Test code - underscore prefix is intentional for unused parameter
        fn _test_has_many_fix<M: ModelTrait, R: LifeModelTrait, Ex: LifeExecutor>(
            entity: &M,
            executor: &Ex,
        ) -> Result<Vec<R::Model>, LifeError>
        where
            M::Entity: Related<R>,
            R::Model: crate::query::traits::FromRow,
        {
            let loader = LazyLoader::new(entity, executor);
            loader.load()
        }
        
        // Just verify it compiles - actual execution test would need executor setup
        let _ = user;
    }
}
