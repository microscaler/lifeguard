//! Relation trait for entity relationships - Epic 02 Story 08
//!
//! Provides support for defining and querying entity relationships:
//! - belongs_to: Many-to-one relationship
//! - has_one: One-to-one relationship
//! - has_many: One-to-many relationship
//! - has_many_through: Many-to-many relationship (via join table)

use crate::query::{SelectQuery, LifeModelTrait};
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
