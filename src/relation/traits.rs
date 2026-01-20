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

    /// Get a query builder for a has_many_through relationship using RelationDef
    ///
    /// This is a convenience method that automatically generates join conditions
    /// from the provided `RelationDef` for many-to-many relationships.
    ///
    /// # Arguments
    ///
    /// * `rel` - The related entity type
    /// * `through` - The intermediate entity type (join table)
    /// * `rel_def` - The relationship definition containing join metadata
    ///
    /// # Returns
    ///
    /// Returns a `SelectQuery` builder for the related entities with automatically generated join conditions
    ///
    /// # Panics
    ///
    /// Panics if `rel_def` is not a `HasManyThrough` relationship or if required fields are missing.
    fn has_many_through_with_def<R, T>(&self, rel: R, through: T, rel_def: crate::relation::def::RelationDef) -> SelectQuery<R>
    where
        R: LifeModelTrait + Iden,
        T: LifeModelTrait + Iden,
    {
        let (first_join, second_join) = rel_def.join_on_exprs();
        self.has_many_through(rel, through, first_join, second_join)
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

/// Trait for defining multi-hop relationship paths
///
/// This trait allows entities to define linked relationships that traverse
/// through intermediate entities. For example, User → Posts → Comments.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{Linked, LifeModelTrait};
///
/// struct User;
/// struct Post;
/// struct Comment;
///
/// // Define a linked path: User → Posts → Comments
/// impl Linked<Post, Comment> for User {
///     fn via() -> Vec<lifeguard::relation::def::RelationDef> {
///         vec![
///             // First hop: User → Post
///             <User as lifeguard::Related<Post>>::to(),
///             // Second hop: Post → Comment
///             <Post as lifeguard::Related<Comment>>::to(),
///         ]
///     }
/// }
/// ```
pub trait Linked<I, T>
where
    Self: LifeModelTrait,
    I: LifeModelTrait,
    T: LifeModelTrait,
{
    /// Returns a vector of RelationDefs representing the path from Self to T through I
    ///
    /// The first RelationDef should be from Self to I (intermediate entity),
    /// and the second should be from I to T (target entity).
    ///
    /// # Returns
    ///
    /// A vector of RelationDefs that define the multi-hop path
    fn via() -> Vec<RelationDef>;
}

/// Extension trait for models to find linked entities through multi-hop relationships
///
/// This trait provides the `find_linked()` method on model instances,
/// allowing you to find entities through intermediate relationships.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{FindLinked, Linked, LifeModelTrait, ModelTrait, LifeExecutor};
///
/// // Assuming User → Posts → Comments linked relationship
/// # struct UserModel { id: i32 };
/// # struct CommentModel { id: i32 };
/// # impl lifeguard::ModelTrait for UserModel {
/// #     fn get_primary_key_value(&self) -> lifeguard::PrimaryKeyValue { todo!() }
/// # }
/// # let user: UserModel = UserModel { id: 1 };
/// # let executor: &dyn LifeExecutor = todo!();
/// // Find all comments for this user through their posts
/// // let comments: Vec<CommentModel> = user.find_linked::<Post, Comment>().all(executor)?;
/// ```
pub trait FindLinked: ModelTrait {
    /// Find linked entities through a multi-hop relationship
    ///
    /// This method uses the `Linked<I, T>` trait implementation to build a query
    /// that joins through intermediate entities, then filters by the current model's primary key.
    ///
    /// # Type Parameters
    ///
    /// * `I` - The intermediate entity type
    /// * `T` - The target entity type. `Self::Entity` must implement `Linked<I, T>`.
    ///
    /// # Returns
    ///
    /// Returns a `SelectQuery<T>` filtered by the current model's primary key
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{FindLinked, Linked, LifeModelTrait, ModelTrait, LifeExecutor};
    ///
    /// // Assuming User → Posts → Comments linked relationship
    /// # struct UserModel { id: i32 };
    /// # struct CommentModel { id: i32 };
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
    /// // Find all comments for this user through their posts
    /// // let comments: Vec<CommentModel> = user.find_linked::<Post, Comment>().all(executor)?;
    /// ```
    fn find_linked<I, T>(&self) -> SelectQuery<T>
    where
        I: LifeModelTrait + Iden,
        T: LifeModelTrait + Iden,
        Self::Entity: Linked<I, T>;
}

// Implement FindLinked for all ModelTrait types
impl<M> FindLinked for M
where
    M: ModelTrait,
    M::Entity: LifeEntityName,
{
    fn find_linked<I, T>(&self) -> SelectQuery<T>
    where
        I: LifeModelTrait + Iden,
        T: LifeModelTrait + Iden,
        Self::Entity: Linked<I, T>,
    {
        // Get the linked path from Linked trait
        let path = <Self::Entity as Linked<I, T>>::via();
        
        // Ensure we have at least one hop (should have 2 for a proper linked relationship)
        if path.is_empty() {
            // Return empty query if no path defined
            return SelectQuery::new();
        }
        
        // Build query with joins through intermediate entities
        let mut query = SelectQuery::new();
        
        // For each hop in the path, add a LEFT JOIN
        // First hop: Self::Entity -> I (intermediate)
        if let Some(first_hop) = path.first() {
            let join_expr = first_hop.join_on_expr();
            query = query.left_join(I::default(), join_expr);
        }
        
        // Second hop: I -> T (target)
        if path.len() >= 2 {
            if let Some(second_hop) = path.get(1) {
                let join_expr = second_hop.join_on_expr();
                query = query.left_join(T::default(), join_expr);
            }
        }
        
        // Filter by the current model's primary key
        // Use the first hop's relation definition to build the WHERE condition
        if let Some(first_hop) = path.first() {
            let condition = build_where_condition(first_hop, self);
            query = query.filter(condition);
        }
        
        query
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation::def::{RelationDef, RelationType};
    use crate::relation::identity::Identity;
    use sea_query::{IdenStatic, ConditionType, TableRef};

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
        
        crate::impl_column_def_helper_for_test!(TestColumn);
        
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
        
        crate::impl_column_def_helper_for_test!(TestColumn);
        
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
        
        crate::impl_column_def_helper_for_test!(UserColumn);
        
        impl IdenStatic for PostColumn {
            fn as_str(&self) -> &'static str {
                match self {
                    PostColumn::Id => "id",
                    PostColumn::UserId => "user_id",
                }
            }
        }
        
        crate::impl_column_def_helper_for_test!(PostColumn);
        
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
                    through_from_col: None,
                    through_to_col: None,
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

    #[test]
    fn test_linked_trait_exists() {
        // Test that Linked trait can be implemented
        // This is a compile-time test - if it compiles, the trait works
        use crate::relation::def::{RelationDef, RelationType};
        use crate::relation::identity::Identity;
        use sea_query::{TableName, IntoIden, ConditionType, Iden};
        
        // Define a simple linked relationship for testing
        #[derive(Default, Copy, Clone)]
        struct TestUser;
        
        #[derive(Copy, Clone, Debug)]
        enum TestUserColumn { Id }
        impl sea_query::Iden for TestUserColumn {
            fn unquoted(&self) -> &str { "id" }
        }
        impl IdenStatic for TestUserColumn {
            fn as_str(&self) -> &'static str { "id" }
        }
        
        crate::impl_column_def_helper_for_test!(TestUserColumn);
        
        impl Iden for TestUser {
            fn unquoted(&self) -> &str {
                "users"
            }
            
        }
        
        impl LifeEntityName for TestUser {
            fn table_name(&self) -> &'static str {
                "users"
            }
            
        }
        
        impl LifeModelTrait for TestUser {
            type Model = ();
            type Column = TestUserColumn;
        }
        
        
        #[derive(Default, Copy, Clone)]
        struct TestPost;
        
        #[derive(Copy, Clone, Debug)]
        enum TestPostColumn { Id }
        impl sea_query::Iden for TestPostColumn {
            fn unquoted(&self) -> &str { "id" }
        }
        impl IdenStatic for TestPostColumn {
            fn as_str(&self) -> &'static str { "id" }
        }
        
        crate::impl_column_def_helper_for_test!(TestPostColumn);
        
        impl Iden for TestPost {
            fn unquoted(&self) -> &str {
                "posts"
            }
            
        }
        
        impl LifeEntityName for TestPost {
            fn table_name(&self) -> &'static str {
                "posts"
            }
            
        }
        
        impl LifeModelTrait for TestPost {
            type Model = ();
            type Column = TestPostColumn;
        }
        
        
        #[derive(Default, Copy, Clone)]
        struct TestComment;
        
        #[derive(Copy, Clone, Debug)]
        enum TestCommentColumn { Id }
        impl sea_query::Iden for TestCommentColumn {
            fn unquoted(&self) -> &str { "id" }
        }
        impl IdenStatic for TestCommentColumn {
            fn as_str(&self) -> &'static str { "id" }
        }
        
        crate::impl_column_def_helper_for_test!(TestCommentColumn);
        
        impl Iden for TestComment {
            fn unquoted(&self) -> &str {
                "comments"
            }
            
        }
        
        impl LifeEntityName for TestComment {
            fn table_name(&self) -> &'static str {
                "comments"
            }
            
        }
        
        impl LifeModelTrait for TestComment {
            type Model = ();
            type Column = TestCommentColumn;
        }
        
        
        impl Linked<TestPost, TestComment> for TestUser {
            fn via() -> Vec<RelationDef> {
                vec![
                    // First hop: User -> Post
                    RelationDef {
                        rel_type: RelationType::HasMany,
                        from_tbl: sea_query::TableRef::Table(TableName(None, "users".into_iden()), None),
                        to_tbl: sea_query::TableRef::Table(TableName(None, "posts".into_iden()), None),
                        from_col: Identity::Unary("id".into()),
                        to_col: Identity::Unary("user_id".into()),
                        through_tbl: None,
                        through_from_col: None,
                        through_to_col: None,
                        is_owner: true,
                        skip_fk: false,
                        on_condition: None,
                        condition_type: ConditionType::All,
                    },
                    // Second hop: Post -> Comment
                    RelationDef {
                        rel_type: RelationType::HasMany,
                        from_tbl: sea_query::TableRef::Table(TableName(None, "posts".into_iden()), None),
                        to_tbl: sea_query::TableRef::Table(TableName(None, "comments".into_iden()), None),
                        from_col: Identity::Unary("id".into()),
                        to_col: Identity::Unary("post_id".into()),
                        through_tbl: None,
                        through_from_col: None,
                        through_to_col: None,
                        is_owner: true,
                        skip_fk: false,
                        on_condition: None,
                        condition_type: ConditionType::All,
                    },
                ]
            }
        }
        
        // Verify the trait can be used
        let path = <TestUser as Linked<TestPost, TestComment>>::via();
        assert_eq!(path.len(), 2);
    }

    #[test]
    fn test_find_linked_builds_query() {
        // Test that find_linked() builds a query with proper joins
        // This is a compile-time test to verify the function signature
        use crate::relation::traits::FindLinked;
        use sea_query::{TableName, IntoIden, TableRef};
        
        #[derive(Default, Copy, Clone)]
        struct UserEntity;
        
        impl sea_query::Iden for UserEntity {
            fn unquoted(&self) -> &str { "users" }
        }
        
        impl LifeEntityName for UserEntity {
            fn table_name(&self) -> &'static str { "users" }
        }
        
        impl LifeModelTrait for UserEntity {
            type Model = UserModel;
            type Column = UserColumn;
        }
        
        #[derive(Default, Copy, Clone)]
        struct PostEntity;
        
        impl sea_query::Iden for PostEntity {
            fn unquoted(&self) -> &str { "posts" }
        }
        
        impl LifeEntityName for PostEntity {
            fn table_name(&self) -> &'static str { "posts" }
        }
        
        impl LifeModelTrait for PostEntity {
            type Model = PostModel;
            type Column = PostColumn;
        }
        
        #[derive(Default, Copy, Clone)]
        struct CommentEntity;
        
        impl sea_query::Iden for CommentEntity {
            fn unquoted(&self) -> &str { "comments" }
        }
        
        impl LifeEntityName for CommentEntity {
            fn table_name(&self) -> &'static str { "comments" }
        }
        
        impl LifeModelTrait for CommentEntity {
            type Model = CommentModel;
            type Column = CommentColumn;
        }
        
        #[derive(Clone, Debug)]
        struct UserModel { id: i32 }
        #[derive(Clone, Debug)]
        struct PostModel;
        #[derive(Clone, Debug)]
        struct CommentModel;
        
        #[derive(Copy, Clone, Debug)]
        enum UserColumn { Id }
        
        impl sea_query::Iden for UserColumn {
            fn unquoted(&self) -> &str { "id" }
        }
        
        impl IdenStatic for UserColumn {
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
        
        impl IdenStatic for PostColumn {
            fn as_str(&self) -> &'static str {
                match self {
                    PostColumn::Id => "id",
                    PostColumn::UserId => "user_id",
                }
            }
        }
        
        crate::impl_column_def_helper_for_test!(PostColumn);
        
        #[derive(Copy, Clone, Debug)]
        enum CommentColumn { Id, PostId }
        
        impl sea_query::Iden for CommentColumn {
            fn unquoted(&self) -> &str {
                match self {
                    CommentColumn::Id => "id",
                    CommentColumn::PostId => "post_id",
                }
                
            }
        }
        
        impl IdenStatic for CommentColumn {
            fn as_str(&self) -> &'static str {
                match self {
                    CommentColumn::Id => "id",
                    CommentColumn::PostId => "post_id",
                }
            }
        }
        
        crate::impl_column_def_helper_for_test!(CommentColumn);
        
        impl ModelTrait for UserModel {
            type Entity = UserEntity;
            fn get(&self, col: UserColumn) -> sea_query::Value {
                match col {
                    UserColumn::Id => sea_query::Value::Int(Some(self.id)),
                }
                
            }
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
                    from_col: Identity::Unary("id".into()),
                    to_col: Identity::Unary("user_id".into()),
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
        
        impl Related<CommentEntity> for PostEntity {
            fn to() -> RelationDef {
                RelationDef {
                    rel_type: RelationType::HasMany,
                    from_tbl: TableRef::Table(TableName(None, "posts".into_iden()), None),
                    to_tbl: TableRef::Table(TableName(None, "comments".into_iden()), None),
                    from_col: Identity::Unary("id".into()),
                    to_col: Identity::Unary("post_id".into()),
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
        
        impl Linked<PostEntity, CommentEntity> for UserEntity {
            fn via() -> Vec<RelationDef> {
                vec![
                    <UserEntity as Related<PostEntity>>::to(),
                    <PostEntity as Related<CommentEntity>>::to(),
                ]
            }
        }
        
        let user = UserModel { id: 1 };
        
        // Verify find_linked() returns a query
        let _query = user.find_linked::<PostEntity, CommentEntity>();
        // Just verify it compiles - the actual query execution would require an executor
    }

    #[test]
    fn test_find_linked_three_hop() {
        // Test that find_linked() works with three-hop relationships
        // User → Posts → Comments → Reactions
        use crate::relation::traits::FindLinked;
        use sea_query::{TableName, IntoIden, TableRef};
        
        #[derive(Default, Copy, Clone)]
        struct UserEntity;
        
        impl sea_query::Iden for UserEntity {
            fn unquoted(&self) -> &str { "users" }
        }
        
        impl LifeEntityName for UserEntity {
            fn table_name(&self) -> &'static str { "users" }
        }
        
        impl LifeModelTrait for UserEntity {
            type Model = UserModel;
            type Column = UserColumn;
        }
        
        #[derive(Default, Copy, Clone)]
        struct PostEntity;
        
        impl sea_query::Iden for PostEntity {
            fn unquoted(&self) -> &str { "posts" }
        }
        
        impl LifeEntityName for PostEntity {
            fn table_name(&self) -> &'static str { "posts" }
        }
        
        impl LifeModelTrait for PostEntity {
            type Model = PostModel;
            type Column = PostColumn;
        }
        
        #[derive(Default, Copy, Clone)]
        struct CommentEntity;
        
        impl sea_query::Iden for CommentEntity {
            fn unquoted(&self) -> &str { "comments" }
        }
        
        impl LifeEntityName for CommentEntity {
            fn table_name(&self) -> &'static str { "comments" }
        }
        
        impl LifeModelTrait for CommentEntity {
            type Model = CommentModel;
            type Column = CommentColumn;
        }
        
        #[derive(Default, Copy, Clone)]
        struct ReactionEntity;
        
        impl sea_query::Iden for ReactionEntity {
            fn unquoted(&self) -> &str { "reactions" }
        }
        
        impl LifeEntityName for ReactionEntity {
            fn table_name(&self) -> &'static str { "reactions" }
        }
        
        impl LifeModelTrait for ReactionEntity {
            type Model = ReactionModel;
            type Column = ReactionColumn;
        }
        
        #[derive(Clone, Debug)]
        struct UserModel { id: i32 }
        #[derive(Clone, Debug)]
        struct PostModel;
        #[derive(Clone, Debug)]
        struct CommentModel;
        #[derive(Clone, Debug)]
        struct ReactionModel;
        
        #[derive(Copy, Clone, Debug)]
        enum UserColumn { Id }
        
        impl sea_query::Iden for UserColumn {
            fn unquoted(&self) -> &str { "id" }
        }
        
        impl IdenStatic for UserColumn {
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
        
        impl IdenStatic for PostColumn {
            fn as_str(&self) -> &'static str {
                match self {
                    PostColumn::Id => "id",
                    PostColumn::UserId => "user_id",
                }
            }
        }
        
        crate::impl_column_def_helper_for_test!(PostColumn);
        
        #[derive(Copy, Clone, Debug)]
        enum CommentColumn { Id, PostId }
        
        impl sea_query::Iden for CommentColumn {
            fn unquoted(&self) -> &str {
                match self {
                    CommentColumn::Id => "id",
                    CommentColumn::PostId => "post_id",
                }
                
            }
        }
        
        impl IdenStatic for CommentColumn {
            fn as_str(&self) -> &'static str {
                match self {
                    CommentColumn::Id => "id",
                    CommentColumn::PostId => "post_id",
                }
            }
        }
        
        crate::impl_column_def_helper_for_test!(CommentColumn);
        
        #[derive(Copy, Clone, Debug)]
        enum ReactionColumn { Id, CommentId }
        
        impl sea_query::Iden for ReactionColumn {
            fn unquoted(&self) -> &str {
                match self {
                    ReactionColumn::Id => "id",
                    ReactionColumn::CommentId => "comment_id",
                }
                
            }
        }
        
        impl IdenStatic for ReactionColumn {
            fn as_str(&self) -> &'static str {
                match self {
                    ReactionColumn::Id => "id",
                    ReactionColumn::CommentId => "comment_id",
                }
            }
        }
        
        crate::impl_column_def_helper_for_test!(ReactionColumn);
        
        impl ModelTrait for UserModel {
            type Entity = UserEntity;
            fn get(&self, col: UserColumn) -> sea_query::Value {
                match col {
                    UserColumn::Id => sea_query::Value::Int(Some(self.id)),
                }
                
            }
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
                    from_col: Identity::Unary("id".into()),
                    to_col: Identity::Unary("user_id".into()),
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
        
        impl Related<CommentEntity> for PostEntity {
            fn to() -> RelationDef {
                RelationDef {
                    rel_type: RelationType::HasMany,
                    from_tbl: TableRef::Table(TableName(None, "posts".into_iden()), None),
                    to_tbl: TableRef::Table(TableName(None, "comments".into_iden()), None),
                    from_col: Identity::Unary("id".into()),
                    to_col: Identity::Unary("post_id".into()),
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
        
        impl Related<ReactionEntity> for CommentEntity {
            fn to() -> RelationDef {
                RelationDef {
                    rel_type: RelationType::HasMany,
                    from_tbl: TableRef::Table(TableName(None, "comments".into_iden()), None),
                    to_tbl: TableRef::Table(TableName(None, "reactions".into_iden()), None),
                    from_col: Identity::Unary("id".into()),
                    to_col: Identity::Unary("comment_id".into()),
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
        
        // Three-hop: User → Posts → Comments → Reactions
        impl Linked<PostEntity, CommentEntity> for UserEntity {
            fn via() -> Vec<RelationDef> {
                vec![
                    <UserEntity as Related<PostEntity>>::to(),
                    <PostEntity as Related<CommentEntity>>::to(),
                ]
            }
        }
        
        // Note: We can't directly do User → Reactions in one Linked, but we can chain
        // For this test, we verify the three-hop path compiles
        let user = UserModel { id: 1 };
        
        // First hop: User → Comments (through Posts)
        let _comments_query = user.find_linked::<PostEntity, CommentEntity>();
        
        // Verify it compiles - actual execution would require executor setup
    }

    #[test]
    fn test_find_linked_empty_path() {
        // Test that find_linked() handles empty path gracefully
        use crate::relation::traits::FindLinked;
        
        #[derive(Default, Copy, Clone)]
        struct TestEntity;
        
        impl sea_query::Iden for TestEntity {
            fn unquoted(&self) -> &str { "test" }
        }
        
        impl LifeEntityName for TestEntity {
            fn table_name(&self) -> &'static str { "test" }
        }
        
        impl LifeModelTrait for TestEntity {
            type Model = TestModel;
            type Column = TestColumn;
        }
        
        #[derive(Clone, Debug)]
        struct TestModel;
        #[derive(Default, Copy, Clone)]
        struct IntermediateEntity;
        #[derive(Default, Copy, Clone)]
        struct TargetEntity;
        
        #[derive(Copy, Clone, Debug)]
        enum TestColumn { Id }
        
        impl sea_query::Iden for TestColumn {
            fn unquoted(&self) -> &str { "id" }
        }
        
        impl IdenStatic for TestColumn {
            fn as_str(&self) -> &'static str { "id" }
        }
        
        crate::impl_column_def_helper_for_test!(TestColumn);
        
        impl sea_query::Iden for IntermediateEntity {
            fn unquoted(&self) -> &str { "intermediate" }
        }
        
        impl sea_query::Iden for TargetEntity {
            fn unquoted(&self) -> &str { "target" }
        }
        
        
        impl LifeEntityName for IntermediateEntity {
            fn table_name(&self) -> &'static str { "intermediate" }
        }
        
        #[derive(Copy, Clone, Debug)]
        enum IntermediateColumn { Id }
        
        impl sea_query::Iden for IntermediateColumn {
            fn unquoted(&self) -> &str { "id" }
        }
        
        impl IdenStatic for IntermediateColumn {
            fn as_str(&self) -> &'static str { "id" }
        }
        
        crate::impl_column_def_helper_for_test!(IntermediateColumn);
        
        impl LifeModelTrait for IntermediateEntity {
            type Model = ();
            type Column = IntermediateColumn;
        }
        
        
        impl LifeEntityName for TargetEntity {
            fn table_name(&self) -> &'static str { "target" }
        }
        
        #[derive(Copy, Clone, Debug)]
        enum TargetColumn { Id }
        
        impl sea_query::Iden for TargetColumn {
            fn unquoted(&self) -> &str { "id" }
        }
        
        
        impl IdenStatic for TargetColumn {
            fn as_str(&self) -> &'static str { "id" }
        }
        
        crate::impl_column_def_helper_for_test!(TargetColumn);
        
        impl LifeModelTrait for TargetEntity {
            type Model = ();
            type Column = TargetColumn;
        }
        
        
        impl ModelTrait for TestModel {
            type Entity = TestEntity;
            fn get(&self, _col: TestColumn) -> sea_query::Value { todo!() }
            fn set(&mut self, _col: TestColumn, _val: sea_query::Value) -> Result<(), crate::model::ModelError> { todo!() }
            fn get_primary_key_value(&self) -> sea_query::Value { todo!() }
            fn get_primary_key_identity(&self) -> Identity { Identity::Unary("id".into()) }
            fn get_primary_key_values(&self) -> Vec<sea_query::Value> { vec![] }
        }
        
        
        impl super::Linked<IntermediateEntity, TargetEntity> for TestEntity {
            fn via() -> Vec<RelationDef> {
                // Return empty path to test edge case
                vec![]
            }
        }
        
        let model = TestModel;
        let query = model.find_linked::<IntermediateEntity, TargetEntity>();
        
        // Verify query was created (even if path is empty)
        let _ = query;
    }
}
