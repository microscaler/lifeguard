//! Core traits for entity relationships.
//!
//! This module provides traits for defining and querying entity relationships,
//! including RelationTrait, Related, FindRelated, and helper traits.

use crate::query::{SelectQuery, LifeModelTrait, LifeEntityName};
use crate::model::ModelTrait;
use crate::relation::def::{RelationDef, build_where_condition};
use sea_query::{Expr, Iden};

/// Trait for defining entity relationships
///
/// This trait allows entities to define their relationships with other entities.
/// Each relationship type (belongs_to, has_one, has_many, has_many_through) has
/// specific methods that return query builders for related entities.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{RelationTrait, LifeModelTrait, LifeExecutor};
/// use sea_query::Expr;
///
/// // Define a User entity with a has_many relationship to Posts
/// struct User;
///
/// impl RelationTrait for User {
///     fn belongs_to<R>(&self, _rel: R) -> SelectQuery<R>
///     where
///         R: LifeModelTrait,
///     {
///         // Implementation for belongs_to relationship
///         todo!()
///     }
/// }
/// ```
pub trait RelationTrait: LifeModelTrait {
    /// Get a query builder for a belongs_to relationship
    ///
    /// This represents a many-to-one relationship where the current entity
    /// belongs to another entity (e.g., Post belongs_to User).
    ///
    /// # Arguments
    ///
    /// * `rel` - The related entity type
    /// * `foreign_key` - The foreign key column in the current entity (e.g., "user_id")
    /// * `on` - The join condition expression (e.g., `Expr::col(("posts", "user_id")).eq(Expr::col(("users", "id")))`)
    ///
    /// # Returns
    ///
    /// Returns a `SelectQuery` builder for the related entity with the join condition applied
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{RelationTrait, LifeModelTrait};
    /// use sea_query::Expr;
    ///
    /// struct Post;
    /// struct User;
    ///
    /// impl RelationTrait for Post {
    ///     fn belongs_to<R>(&self, rel: R, foreign_key: &str, on: Expr) -> SelectQuery<R>
    ///     where
    ///         R: LifeModelTrait,
    ///     {
    ///         SelectQuery::new().left_join(rel, on)
    ///     }
    /// }
    /// ```
    fn belongs_to<R>(&self, rel: R, _foreign_key: &str, on: Expr) -> SelectQuery<R>
    where
        R: LifeModelTrait + Iden,
    {
        // belongs_to: Join the related entity table using LEFT JOIN
        // The join condition should be: current_table.foreign_key = related_table.primary_key
        SelectQuery::new().left_join(rel, on)
    }

    /// Get a query builder for a has_one relationship
    ///
    /// This represents a one-to-one relationship where the current entity
    /// has one related entity (e.g., User has_one Profile).
    ///
    /// # Arguments
    ///
    /// * `rel` - The related entity type
    /// * `foreign_key` - The foreign key column in the related entity (e.g., "user_id")
    /// * `on` - The join condition expression
    ///
    /// # Returns
    ///
    /// Returns a `SelectQuery` builder for the related entity with the join condition applied
    fn has_one<R>(&self, rel: R, _foreign_key: &str, on: Expr) -> SelectQuery<R>
    where
        R: LifeModelTrait + Iden,
    {
        // has_one: Join the related entity table using LEFT JOIN
        // The join condition should be: current_table.primary_key = related_table.foreign_key
        SelectQuery::new().left_join(rel, on)
    }

    /// Get a query builder for a has_many relationship
    ///
    /// This represents a one-to-many relationship where the current entity
    /// has many related entities (e.g., User has_many Posts).
    ///
    /// # Arguments
    ///
    /// * `rel` - The related entity type
    /// * `foreign_key` - The foreign key column in the related entity (e.g., "user_id")
    /// * `on` - The join condition expression
    ///
    /// # Returns
    ///
    /// Returns a `SelectQuery` builder for the related entities with the join condition applied
    fn has_many<R>(&self, rel: R, _foreign_key: &str, on: Expr) -> SelectQuery<R>
    where
        R: LifeModelTrait + Iden,
    {
        // has_many: Join the related entity table using LEFT JOIN
        // The join condition should be: current_table.primary_key = related_table.foreign_key
        SelectQuery::new().left_join(rel, on)
    }

    /// Get a query builder for a has_many_through relationship
    ///
    /// This represents a many-to-many relationship through a join table
    /// (e.g., User has_many_through Posts to Tags via PostTags).
    ///
    /// # Arguments
    ///
    /// * `rel` - The related entity type
    /// * `through` - The intermediate entity type (join table)
    /// * `first_join` - The join condition for the first join (current -> through)
    /// * `second_join` - The join condition for the second join (through -> related)
    ///
    /// # Returns
    ///
    /// Returns a `SelectQuery` builder for the related entities with both joins applied
    fn has_many_through<R, T>(&self, rel: R, through: T, first_join: Expr, second_join: Expr) -> SelectQuery<R>
    where
        R: LifeModelTrait + Iden,
        T: LifeModelTrait + Iden,
    {
        // has_many_through: Join through the intermediate table, then to the related entity
        // First join: current_table -> through_table
        // Second join: through_table -> related_table
        SelectQuery::new()
            .left_join(through, first_join)
            .left_join(rel, second_join)
    }

    /// Get a query builder for a belongs_to relationship using RelationDef
    ///
    /// This is a convenience method that automatically generates join conditions
    /// from the provided `RelationDef`, eliminating the need to manually construct
    /// join expressions.
    ///
    /// # Arguments
    ///
    /// * `rel` - The related entity type
    /// * `rel_def` - The relationship definition containing join metadata
    ///
    /// # Returns
    ///
    /// Returns a `SelectQuery` builder for the related entity with automatically generated join condition
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{RelationTrait, Related, LifeModelTrait};
    ///
    /// struct Post;
    /// struct User;
    ///
    /// impl RelationTrait for Post {
    ///     // ... other methods ...
    /// }
    ///
    /// impl Related<User> for Post {
    ///     fn to() -> lifeguard::RelationDef {
    ///         // ... relationship definition ...
    ///         # lifeguard::RelationDef { /* ... */ }
    ///     }
    /// }
    ///
    /// // Use automatic join condition generation
    /// let query = Post::default().belongs_to_with_def(User, <Post as Related<User>>::to());
    /// ```
    fn belongs_to_with_def<R>(&self, rel: R, rel_def: crate::relation::def::RelationDef) -> SelectQuery<R>
    where
        R: LifeModelTrait + Iden,
    {
        let join_expr = rel_def.join_on_expr();
        self.belongs_to(rel, "", join_expr)
    }

    /// Get a query builder for a has_one relationship using RelationDef
    ///
    /// This is a convenience method that automatically generates join conditions
    /// from the provided `RelationDef`.
    ///
    /// # Arguments
    ///
    /// * `rel` - The related entity type
    /// * `rel_def` - The relationship definition containing join metadata
    ///
    /// # Returns
    ///
    /// Returns a `SelectQuery` builder for the related entity with automatically generated join condition
    fn has_one_with_def<R>(&self, rel: R, rel_def: crate::relation::def::RelationDef) -> SelectQuery<R>
    where
        R: LifeModelTrait + Iden,
    {
        let join_expr = rel_def.join_on_expr();
        self.has_one(rel, "", join_expr)
    }

    /// Get a query builder for a has_many relationship using RelationDef
    ///
    /// This is a convenience method that automatically generates join conditions
    /// from the provided `RelationDef`.
    ///
    /// # Arguments
    ///
    /// * `rel` - The related entity type
    /// * `rel_def` - The relationship definition containing join metadata
    ///
    /// # Returns
    ///
    /// Returns a `SelectQuery` builder for the related entities with automatically generated join condition
    fn has_many_with_def<R>(&self, rel: R, rel_def: crate::relation::def::RelationDef) -> SelectQuery<R>
    where
        R: LifeModelTrait + Iden,
    {
        let join_expr = rel_def.join_on_expr();
        self.has_many(rel, "", join_expr)
    }
}

/// Helper trait for building relationship queries with join conditions
///
/// This trait provides methods to build relationship queries with proper
/// join conditions based on foreign keys.
pub trait RelationBuilder {
    /// Set the foreign key column for the relationship
    ///
    /// # Arguments
    ///
    /// * `column` - The foreign key column name
    ///
    /// # Returns
    ///
    /// Returns self for method chaining
    fn foreign_key<C: sea_query::IntoColumnRef>(self, column: C) -> Self;

    /// Set the referenced key column for the relationship
    ///
    /// # Arguments
    ///
    /// * `column` - The referenced key column name (usually primary key)
    ///
    /// # Returns
    ///
    /// Returns self for method chaining
    fn references<C: sea_query::IntoColumnRef>(self, column: C) -> Self;

    /// Set the join condition expression
    ///
    /// # Arguments
    ///
    /// * `condition` - The join condition expression
    ///
    /// # Returns
    ///
    /// Returns self for method chaining
    fn on(self, condition: Expr) -> Self;
}

/// Trait for storing relationship metadata
///
/// This trait provides metadata about relationships, including foreign key columns.
/// It's used by `find_related()` to build proper WHERE clauses.
pub trait RelationMetadata<R>
where
    Self: LifeModelTrait,
    R: LifeModelTrait,
{
    /// Get the foreign key column name in the related entity's table
    ///
    /// For has_many relationships: returns the foreign key column in the related entity
    /// For belongs_to relationships: returns the foreign key column in the current entity
    ///
    /// Returns None if the foreign key should be inferred (default behavior)
    fn foreign_key_column() -> Option<&'static str> {
        None
    }
}

/// Trait for finding related entities from a model instance
///
/// This trait enables querying related entities through relationships defined
/// via `RelationTrait`. It provides a convenient API for finding related entities
/// without manually constructing join queries.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{Related, LifeModelTrait, SelectQuery, LifeExecutor};
/// use sea_query::Expr;
///
/// // Define entities
/// struct User;
/// struct Post;
///
/// impl LifeModelTrait for User {
///     type Model = UserModel;
///     type Column = UserColumn;
/// }
///
/// impl LifeModelTrait for Post {
///     type Model = PostModel;
///     type Column = PostColumn;
/// }
///
/// // Define relationship: Post belongs_to User
/// impl Related<User> for Post {
///     fn to() -> SelectQuery<User> {
///         // This would typically use RelationTrait to build the query
///         // For now, this is a placeholder that needs implementation
///         todo!()
///     }
/// }
///
/// // Use it to find related entities
/// # struct UserModel { id: i32 };
/// # struct PostModel { id: i32, user_id: i32 };
/// # let user: UserModel = UserModel { id: 1 };
/// # let executor: &dyn LifeExecutor = todo!();
/// // Find all posts for a user (if User has_many Posts relationship is defined)
/// // let posts: Vec<PostModel> = user.find_related::<Post>().all(executor)?;
/// ```
pub trait Related<R>
where
    Self: LifeModelTrait,
    R: LifeModelTrait,
{
    /// Returns RelationDef with all relationship metadata
    ///
    /// This method returns a `RelationDef` struct containing all metadata about
    /// the relationship between `Self` and `R`, including:
    /// - Relationship type (HasOne, HasMany, BelongsTo)
    /// - Source and target tables
    /// - Foreign key and primary key columns (supports composite keys)
    /// - Additional metadata (ownership, foreign key constraints, etc.)
    ///
    /// # Returns
    ///
    /// Returns a `RelationDef` containing all relationship metadata
    ///
    /// # Note
    ///
    /// This is a static method that returns relationship metadata. To filter by
    /// a specific instance's primary key, use `find_related()` on a model instance
    /// which will call this method and use `build_where_condition()` to apply the WHERE clause.
    ///
    /// # Breaking Change
    ///
    /// **⚠️ BREAKING CHANGE:** As of this version, `Related::to()` returns `RelationDef` instead of `SelectQuery<Self>`.
    /// This is a breaking change but provides better design and supports composite keys.
    fn to() -> RelationDef;
}

/// Extension trait for models to find related entities
///
/// This trait provides the `find_related()` method on model instances,
/// allowing you to find related entities based on the current model's
/// primary key value.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{FindRelated, Related, LifeModelTrait, ModelTrait, LifeExecutor};
///
/// // Assuming User has_many Posts relationship
/// # struct UserModel { id: i32 };
/// # struct PostModel { id: i32, user_id: i32 };
/// # impl lifeguard::ModelTrait for UserModel {
/// #     fn get_primary_key_value(&self) -> lifeguard::PrimaryKeyValue { todo!() }
/// # }
/// # let user: UserModel = UserModel { id: 1 };
/// # let executor: &dyn LifeExecutor = todo!();
/// // Find all posts for this user
/// // let posts: Vec<PostModel> = user.find_related::<Post>().all(executor)?;
/// ```
pub trait FindRelated: ModelTrait {
    /// Find related entities of type `R`
    ///
    /// This method uses the `Related<R>` trait implementation to build a query
    /// for related entities, then filters by the current model's primary key.
    ///
    /// # Type Parameters
    ///
    /// * `R` - The related entity type. `Self::Entity` must implement `Related<R>`.
    ///
    /// # Returns
    ///
    /// Returns a `SelectQuery<R>` filtered by the current model's primary key
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{FindRelated, Related, LifeModelTrait, ModelTrait, LifeExecutor};
    /// use sea_query::Expr;
    ///
    /// // Assuming User has_many Posts relationship
    /// # struct UserModel { id: i32 };
    /// # struct PostModel { id: i32, user_id: i32 };
    /// # impl lifeguard::ModelTrait for UserModel {
    /// #     type Entity = User;
    /// #     fn get_primary_key_value(&self) -> sea_query::Value { todo!() }
    /// #     fn get(&self, _col: <User as lifeguard::LifeModelTrait>::Column) -> sea_query::Value { todo!() }
    /// #     fn set(&mut self, _col: <User as lifeguard::LifeModelTrait>::Column, _val: sea_query::Value) -> Result<(), lifeguard::ModelError> { todo!() }
    /// #     fn get_primary_key_identity(&self) -> lifeguard::Identity { todo!() }
    /// # }
    /// # struct User;
    /// # impl lifeguard::LifeModelTrait for User {
    /// #     type Model = UserModel;
    /// #     type Column = ();
    /// # }
    /// # let user: UserModel = UserModel { id: 1 };
    /// # let executor: &dyn LifeExecutor = todo!();
    /// // Find all posts for this user
    /// // let posts: Vec<PostModel> = user.find_related::<Post>().all(executor)?;
    /// ```
    fn find_related<R>(&self) -> SelectQuery<R>
    where
        R: LifeModelTrait,
        Self::Entity: Related<R>;
}

// Implement FindRelated for all ModelTrait types
impl<M> FindRelated for M
where
    M: ModelTrait,
    M::Entity: LifeEntityName,
{
    fn find_related<R>(&self) -> SelectQuery<R>
    where
        R: LifeModelTrait,
        Self::Entity: Related<R>,
    {
        // Get the relationship definition from Related trait
        // Self::Entity: Related<R> means "Self::Entity is related to R"
        // So Self::Entity::to() returns RelationDef for the relationship from Self::Entity to R
        // This is the correct relationship direction for find_related()
        let rel_def = <Self::Entity as Related<R>>::to();
        
        // Create a new query for the related entity
        let mut query = SelectQuery::new();
        
        // Build WHERE condition from RelationDef and model primary key values
        // This uses build_where_condition() which handles both single and composite keys
        // Note: build_where_condition() currently has a placeholder implementation
        // that will be fully functional after Phase 4 adds get_primary_key_identity()
        let condition = build_where_condition(&rel_def, self);
        query = query.filter(condition);
        
        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation::def::{RelationDef, RelationType};
    use crate::relation::identity::Identity;
    use sea_query::{IdenStatic, TableRef, ConditionType};

    #[test]
    fn test_relation_trait_methods_exist() {
        // Test that RelationTrait methods exist and can be called
        // This is a compile-time check that the trait is properly defined
        use crate::{LifeEntityName, LifeModelTrait};
        
        #[derive(Default, Copy, Clone)]
        struct TestEntity;
        
        impl sea_query::Iden for TestEntity {
            fn unquoted(&self) -> &str {
                "test_entities"
            }
        }
        
        #[derive(Copy, Clone, Debug)]
        enum TestColumn {
            Id,
        }
        
        impl sea_query::Iden for TestColumn {
            fn unquoted(&self) -> &str {
                "id"
            }
        }
        
        impl IdenStatic for TestColumn {
            fn as_str(&self) -> &'static str {
                "id"
            }
        }
        
        impl LifeEntityName for TestEntity {
            fn table_name(&self) -> &'static str {
                "test_entities"
            }
        }
        
        impl LifeModelTrait for TestEntity {
            type Model = ();
            type Column = TestColumn;
        }
        
        impl RelationTrait for TestEntity {}
        
        let entity = TestEntity;
        use sea_query::Expr;
        // Create placeholder join conditions for testing
        let join_cond = Expr::cust("1 = 1");
        let _query1 = entity.belongs_to(TestEntity, "foreign_key", join_cond.clone());
        let _query2 = entity.has_one(TestEntity, "foreign_key", join_cond.clone());
        let _query3 = entity.has_many(TestEntity, "foreign_key", join_cond.clone());
        let _query4 = entity.has_many_through(TestEntity, TestEntity, join_cond.clone(), join_cond);
    }

    #[test]
    fn test_relation_belongs_to_with_empty_join_condition() {
        // EDGE CASE: belongs_to with empty/invalid join condition
        use crate::{LifeEntityName, LifeModelTrait};
        
        #[derive(Default, Copy, Clone)]
        struct TestEntity;
        
        impl sea_query::Iden for TestEntity {
            fn unquoted(&self) -> &str { "test_entities" }
        }
        
        #[derive(Copy, Clone, Debug)]
        enum TestColumn { Id }
        impl sea_query::Iden for TestColumn {
            fn unquoted(&self) -> &str { "id" }
        }
        impl IdenStatic for TestColumn {
            fn as_str(&self) -> &'static str { "id" }
        }
        impl LifeEntityName for TestEntity {
            fn table_name(&self) -> &'static str { "test_entities" }
        }
        impl LifeModelTrait for TestEntity {
            type Model = ();
            type Column = TestColumn;
        }
        impl RelationTrait for TestEntity {}
        
        let entity = TestEntity;
        let join_cond = Expr::cust("1 = 1");
        let _query = entity.belongs_to(TestEntity, "user_id", join_cond);
    }

    #[test]
    fn test_find_related_on_model_type() {
        // Test that FindRelated can be implemented for Model types (not just Entity types)
        // This verifies the fix for the bug where FindRelated required LifeModelTrait,
        // which Models don't implement (only Entities do).
        use crate::{LifeEntityName, LifeModelTrait};
        use sea_query::{IntoIden, TableName};
        
        // Define test entities
        #[derive(Default, Copy, Clone)]
        struct UserEntity;
        
        #[derive(Default, Copy, Clone)]
        struct PostEntity;
        
        impl sea_query::Iden for UserEntity {
            fn unquoted(&self) -> &str { "users" }
        }
        
        impl sea_query::Iden for PostEntity {
            fn unquoted(&self) -> &str { "posts" }
        }
        
        #[derive(Copy, Clone, Debug)]
        enum UserColumn {
            Id,
        }
        
        #[derive(Copy, Clone, Debug)]
        enum PostColumn {
            Id,
            UserId,
        }
        
        impl sea_query::Iden for UserColumn {
            fn unquoted(&self) -> &str {
                match self {
                    UserColumn::Id => "id",
                }
            }
        }
        
        impl sea_query::Iden for PostColumn {
            fn unquoted(&self) -> &str {
                match self {
                    PostColumn::Id => "id",
                    PostColumn::UserId => "user_id",
                }
            }
        }
        
        impl IdenStatic for UserColumn {
            fn as_str(&self) -> &'static str {
                match self {
                    UserColumn::Id => "id",
                }
            }
        }
        
        impl IdenStatic for PostColumn {
            fn as_str(&self) -> &'static str {
                match self {
                    PostColumn::Id => "id",
                    PostColumn::UserId => "user_id",
                }
            }
        }
        
        impl LifeEntityName for UserEntity {
            fn table_name(&self) -> &'static str { "users" }
        }
        
        impl LifeEntityName for PostEntity {
            fn table_name(&self) -> &'static str { "posts" }
        }
        
        impl LifeModelTrait for UserEntity {
            type Model = UserModel;
            type Column = UserColumn;
        }
        
        impl LifeModelTrait for PostEntity {
            type Model = PostModel;
            type Column = PostColumn;
        }
        
        // Define test models
        #[derive(Clone, Debug)]
        struct UserModel {
            id: i32,
        }
        
        #[derive(Clone, Debug)]
        struct PostModel {
            id: i32,
            user_id: i32,
        }
        
        impl ModelTrait for UserModel {
            type Entity = UserEntity;
            
            fn get(&self, column: UserColumn) -> sea_query::Value {
                match column {
                    UserColumn::Id => sea_query::Value::Int(Some(self.id)),
                }
            }
            
            fn set(&mut self, column: UserColumn, value: sea_query::Value) -> Result<(), crate::ModelError> {
                match column {
                    UserColumn::Id => {
                        if let sea_query::Value::Int(Some(v)) = value {
                            self.id = v;
                            Ok(())
                        } else {
                            Err(crate::ModelError::InvalidValueType {
                                column: "id".to_string(),
                                expected: "Int(Some(_))".to_string(),
                                actual: format!("{:?}", value),
                            })
                        }
                    }
                }
            }
            
            fn get_primary_key_value(&self) -> sea_query::Value {
                sea_query::Value::Int(Some(self.id))
            }
            
            fn get_primary_key_identity(&self) -> Identity {
                Identity::Unary(sea_query::DynIden::from(UserColumn::Id.as_str()))
            }
            
            fn get_primary_key_values(&self) -> Vec<sea_query::Value> {
                vec![sea_query::Value::Int(Some(self.id))]
            }
        }
        
        impl ModelTrait for PostModel {
            type Entity = PostEntity;
            
            fn get(&self, column: PostColumn) -> sea_query::Value {
                match column {
                    PostColumn::Id => sea_query::Value::Int(Some(self.id)),
                    PostColumn::UserId => sea_query::Value::Int(Some(self.user_id)),
                }
            }
            
            fn set(&mut self, column: PostColumn, value: sea_query::Value) -> Result<(), crate::ModelError> {
                match column {
                    PostColumn::Id => {
                        if let sea_query::Value::Int(Some(v)) = value {
                            self.id = v;
                            Ok(())
                        } else {
                            Err(crate::ModelError::InvalidValueType {
                                column: "id".to_string(),
                                expected: "Int(Some(_))".to_string(),
                                actual: format!("{:?}", value),
                            })
                        }
                    }
                    PostColumn::UserId => {
                        if let sea_query::Value::Int(Some(v)) = value {
                            self.user_id = v;
                            Ok(())
                        } else {
                            Err(crate::ModelError::InvalidValueType {
                                column: "user_id".to_string(),
                                expected: "Int(Some(_))".to_string(),
                                actual: format!("{:?}", value),
                            })
                        }
                    }
                }
            }
            
            fn get_primary_key_value(&self) -> sea_query::Value {
                sea_query::Value::Int(Some(self.id))
            }
            
            fn get_primary_key_identity(&self) -> Identity {
                Identity::Unary(sea_query::DynIden::from(PostColumn::Id.as_str()))
            }
            
            fn get_primary_key_values(&self) -> Vec<sea_query::Value> {
                vec![sea_query::Value::Int(Some(self.id))]
            }
        }
        
        // Define relationship: User has_many Posts
        impl Related<PostEntity> for UserEntity {
            fn to() -> RelationDef {
                RelationDef {
                    rel_type: RelationType::HasMany,
                    from_tbl: TableRef::Table(TableName(None, UserEntity::table_name(&UserEntity).into_iden()), None),
                    to_tbl: TableRef::Table(TableName(None, PostEntity::table_name(&PostEntity).into_iden()), None),
                    from_col: Identity::Unary(sea_query::DynIden::from(UserColumn::Id.as_str())),
                    to_col: Identity::Unary(sea_query::DynIden::from(PostColumn::UserId.as_str())),
                    through_tbl: None,
                    is_owner: true,
                    skip_fk: false,
                    on_condition: None,
                    condition_type: ConditionType::All,
                }
            }
        }
        
        // Test that find_related() can be called on a Model instance
        // This verifies that Models (which only implement ModelTrait, not LifeModelTrait)
        // can use FindRelated trait
        let user = UserModel { id: 1 };
        let _query = user.find_related::<PostEntity>();
        // Just verify it compiles - the actual query execution would require an executor
    }
}
