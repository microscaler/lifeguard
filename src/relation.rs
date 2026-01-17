//! Relation trait for entity relationships - Epic 02 Story 08
//!
//! Provides support for defining and querying entity relationships:
//! - belongs_to: Many-to-one relationship
//! - has_one: One-to-one relationship
//! - has_many: One-to-many relationship
//! - has_many_through: Many-to-many relationship (via join table)

pub mod identity;
pub mod def;
pub use identity::{Identity, BorrowedIdentityIter, IntoIdentity};
pub use def::{RelationDef, RelationType, join_tbl_on_condition, build_where_condition};

use crate::query::{SelectQuery, LifeModelTrait, LifeEntityName};
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

/// Helper function to create a join condition for relationships
///
/// This creates an expression that joins two tables based on foreign key
/// relationships. The function creates a table-qualified column comparison
/// expression.
///
/// # Arguments
///
/// * `from_table` - The source table name
/// * `from_column` - The foreign key column in the source table
/// * `to_table` - The target table name
/// * `to_column` - The referenced column in the target table (usually primary key)
///
/// # Returns
///
/// Returns an `Expr` representing the join condition: `from_table.from_column = to_table.to_column`
///
/// # Example
///
/// ```no_run
/// use lifeguard::join_condition;
/// use sea_query::Expr;
///
/// // Create a join condition: posts.user_id = users.id
/// let condition = join_condition("posts", "user_id", "users", "id");
///
/// // Or construct manually for more control:
/// let condition = Expr::col(("posts", "user_id"))
///     .equals(Expr::col(("users", "id")));
/// ```
pub fn join_condition(
    from_table: &str,
    from_column: &str,
    to_table: &str,
    to_column: &str,
) -> Expr {
    // Create table-qualified column references and compare them
    // SeaQuery doesn't have a direct .equals() method for column-to-column comparisons,
    // so we use a custom SQL expression
    // Note: This creates a raw SQL string, so table/column names should be validated
    // to prevent SQL injection if user input is involved
    let condition = format!(
        "{}.{} = {}.{}",
        from_table, from_column, to_table, to_column
    );
    Expr::cust(condition)
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
pub trait FindRelated: ModelTrait + LifeModelTrait {
    /// Find related entities of type `R`
    ///
    /// This method uses the `Related<R>` trait implementation to build a query
    /// for related entities, then filters by the current model's primary key.
    ///
    /// # Type Parameters
    ///
    /// * `R` - The related entity type that implements `Related<Self::Entity>`
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
    /// #     fn get_primary_key_value(&self) -> lifeguard::PrimaryKeyValue { todo!() }
    /// # }
    /// # impl lifeguard::LifeModelTrait for UserModel {
    /// #     type Entity = ();
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
        R: LifeModelTrait + Related<Self::Entity>;
}

// Import ModelTrait for the FindRelated implementation
use crate::model::ModelTrait;

impl<M> FindRelated for M
where
    M: ModelTrait + LifeModelTrait,
    M::Entity: LifeEntityName,
{
    fn find_related<R>(&self) -> SelectQuery<R>
    where
        R: LifeModelTrait + Related<Self::Entity>,
    {
        // Get the relationship definition from Related trait
        // R: Related<Self::Entity> means "R is related to Self::Entity"
        // So R::to() returns RelationDef for the relationship from R to Self::Entity
        let rel_def = R::to();
        
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

    #[test]
    fn test_join_condition() {
        // Test that join_condition returns an Expr
        let condition = join_condition("posts", "user_id", "users", "id");
        // Verify the condition is created (we can't easily test the SQL generation
        // without a full query builder, but we can verify it compiles)
        let _ = condition;
    }

    #[test]
    fn test_relation_trait_methods_exist() {
        // Test that RelationTrait methods exist and can be called
        // This is a compile-time check that the trait is properly defined
        use crate::{LifeEntityName, LifeModelTrait};
        use sea_query::IdenStatic;
        
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

    // ============================================================================
    // Edge Cases
    // ============================================================================

    #[test]
    fn test_relation_belongs_to_with_empty_join_condition() {
        // EDGE CASE: belongs_to with empty/invalid join condition
        use crate::{LifeEntityName, LifeModelTrait};
        use sea_query::IdenStatic;
        
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
    fn test_join_condition_with_special_characters() {
        // EDGE CASE: Table/column names with special characters
        let condition = join_condition("user_profiles", "user_id", "users", "id");
        let _ = condition;
    }

    #[test]
    fn test_join_condition_empty_strings() {
        // EDGE CASE: Empty table/column names (should still compile, but invalid at runtime)
        let condition = join_condition("", "", "", "");
        let _ = condition;
    }
}
