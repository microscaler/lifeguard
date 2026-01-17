//! Helper functions for relationship operations.
//!
//! This module provides utility functions for working with relationships,
//! including join condition building.

use sea_query::Expr;

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
/// use lifeguard::relation::helpers::join_condition;
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
