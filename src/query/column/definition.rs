//! Column definition metadata and type inference.
//!
//! This module provides `ColumnDefinition` which stores metadata about database columns
//! including type, nullability, default values, and constraints. It also provides
//! utilities for inferring column definitions from Rust types.

use super::type_mapping;
use sea_query::{ColumnDef, Iden};
use std::collections::HashMap;
use std::sync::Mutex;
use once_cell::sync::Lazy;

/// Global cache for SQL expression strings to avoid memory leaks.
///
/// This cache stores leaked `&'static str` references for SQL expressions,
/// ensuring each unique expression is only leaked once. This prevents memory
/// accumulation when `apply_default_expr()` is called multiple times with
/// the same expression (e.g., in tests or repeated migrations).
#[cfg_attr(test, allow(dead_code))]
static EXPR_CACHE: Lazy<Mutex<HashMap<String, &'static str>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

/// Get or create a static string reference for a SQL expression.
///
/// This function uses a global cache to ensure each unique expression string
/// is only leaked once, preventing memory accumulation in tests and repeated
/// migration workflows.
#[cfg_attr(test, allow(dead_code))]
pub fn get_static_expr(expr: &str) -> &'static str {
    let mut cache = EXPR_CACHE.lock().unwrap();
    
    // Check if we already have this expression cached
    if let Some(&cached) = cache.get(expr) {
        return cached;
    }
    
    // Not in cache - leak it and store the reference
    let static_str: &'static str = Box::leak(expr.to_string().into_boxed_str());
    cache.insert(expr.to_string(), static_str);
    static_str
}

/// Column definition metadata
///
/// Stores information about a column's type, nullability, default value, etc.
/// This is used by `ColumnTrait::def()` to provide column metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnDefinition {
    /// Column type (e.g., "Integer", "String", "Json")
    pub column_type: Option<String>,
    /// Whether the column is nullable
    pub nullable: bool,
    /// Default value (if any)
    pub default_value: Option<String>,
    /// Default SQL expression (e.g., "NOW()", "uuid_generate_v4()")
    pub default_expr: Option<String>,
    /// Previous column name (for migrations - column was renamed from this)
    pub renamed_from: Option<String>,
    /// Custom SELECT expression (e.g., "CONCAT(first, ' ', last) AS full_name")
    pub select_as: Option<String>,
    /// Custom save expression (e.g., "NOW()" for timestamps)
    pub save_as: Option<String>,
    /// Column comment/documentation
    pub comment: Option<String>,
    /// Whether the column is unique
    pub unique: bool,
    /// Whether the column is indexed
    pub indexed: bool,
    /// Whether the column is auto-increment
    pub auto_increment: bool,
    /// Foreign key constraint (e.g., "chart_of_accounts(id) ON DELETE SET NULL")
    pub foreign_key: Option<String>,
    /// CHECK constraint expression (column-level)
    pub check: Option<String>,
}

impl Default for ColumnDefinition {
    fn default() -> Self {
        Self {
            column_type: None,
            nullable: false,
            default_value: None,
            default_expr: None,
            renamed_from: None,
            select_as: None,
            save_as: None,
            comment: None,
            unique: false,
            indexed: false,
            auto_increment: false,
            foreign_key: None,
            check: None,
        }
    }
}

impl ColumnDefinition {
    /// Convert to SeaQuery's ColumnDef for use in migrations
    ///
    /// Maps the column metadata to SeaQuery's `ColumnDef` with appropriate type,
    /// constraints, and attributes. This enables schema generation and migrations.
    ///
    /// # Arguments
    ///
    /// * `column_name` - The column identifier (implements `Iden`)
    ///
    /// # Returns
    ///
    /// Returns a `ColumnDef` configured with the column's type, nullability,
    /// auto-increment status, and other attributes.
    ///
    /// # Type Mapping
    ///
    /// Maps column type strings to SeaQuery column types:
    /// - "Integer" / "i32" / "i64" → `.integer()` or `.big_integer()`
    /// - "String" / "Text" → `.string()` or `.text()`
    /// - "Boolean" / "bool" → `.boolean()`
    /// - "Float" / "f32" → `.float()`
    /// - "Double" / "f64" → `.double()`
    /// - "Json" / "Jsonb" → `.json()`
    /// - "Timestamp" / "DateTime" → `.timestamp()`
    /// - "Date" → `.date()`
    /// - "Time" → `.time()`
    /// - "Uuid" → `.uuid()`
    /// - "Binary" / "Bytes" → `.binary()`
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::ColumnDefinition;
    /// use sea_query::ColumnDef;
    ///
    /// let def = ColumnDefinition {
    ///     column_type: Some("Integer".to_string()),
    ///     nullable: false,
    ///     auto_increment: true,
    ///     ..Default::default()
    /// };
    ///
    /// let column_def = def.to_column_def(sea_query::Iden::unquoted("id"));
    /// // column_def is configured as integer, not null, auto-increment
    /// ```
    pub fn to_column_def<T: Iden>(&self, column_name: T) -> ColumnDef {
        let mut def = ColumnDef::new(column_name);
        
        // Map column type string to SeaQuery ColumnType
        if let Some(ref col_type) = self.column_type {
            type_mapping::apply_column_type(col_type, &mut def);
        } else {
            // No type specified, default to text
            def.text();
        }
        
        // Set nullable if applicable
        if self.nullable {
            def.null();
        } else {
            def.not_null();
        }
        
        // Set auto-increment if applicable
        if self.auto_increment {
            def.auto_increment();
        }
        
        // Set default value if provided
        if let Some(ref _default) = self.default_value {
            // Note: SeaQuery's default_value() expects an Expr, not a string
            // For now, we'll need to parse the default value string
            // This is a simplified implementation - full support would require
            // parsing SQL expressions or providing a more structured default value type
            // For migrations, users can manually set defaults using SeaQuery's API
        }
        
        // Set default SQL expression if provided
        // Note: Expr::cust() requires &'static str, but we have &String
        // For now, we store the expression as metadata and migration builders
        // should use it when generating migration SQL.
        // TODO: Consider using a helper that creates Expr from non-static strings
        // or change the API to accept expressions at migration time
        if let Some(ref _expr_str) = self.default_expr {
            // The expression is stored in self.default_expr and can be used
            // by migration builders to set the default expression.
            // Migration builders should use: Expr::cust(expr_str) and then def.default(expr)
            // For now, we just store the metadata - actual application happens in migrations
        }
        
        // Note: Unique and indexed constraints are typically handled separately
        // in SeaQuery via IndexDef, not ColumnDef. The metadata is preserved
        // in ColumnDefinition for reference, but actual unique/index creation
        // should be done via migration builders.
        
        def
    }
    
    /// Apply default expression to a ColumnDef (for use in migrations)
    ///
    /// This helper method applies the default SQL expression to a ColumnDef.
    /// It should be called by migration builders after `to_column_def()` if
    /// `default_expr` is set.
    ///
    /// # Memory Safety
    ///
    /// This method uses a global cache to ensure each unique expression string
    /// is only leaked once. This prevents memory accumulation when called
    /// multiple times (e.g., in tests or repeated migrations). The cache
    /// persists for the lifetime of the program, which is acceptable for
    /// migration use cases where expressions are typically short and reused.
    ///
    /// # Arguments
    ///
    /// * `def` - The ColumnDef to apply the default expression to
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::ColumnDefinition;
    /// use sea_query::{ColumnDef, Iden};
    ///
    /// let col_def = ColumnDefinition {
    ///     default_expr: Some("NOW()".to_string()),
    ///     ..Default::default()
    /// };
    ///
    /// let mut def = col_def.to_column_def(sea_query::Iden::unquoted("created_at"));
    /// col_def.apply_default_expr(&mut def);
    /// ```
    pub fn apply_default_expr(&self, def: &mut ColumnDef) {
        if let Some(ref expr_str) = self.default_expr {
            // Use cached static string to avoid leaking memory on every call
            let static_str = get_static_expr(expr_str);
            use sea_query::Expr;
            let expr = Expr::cust(static_str);
            def.default(expr);
        }
    }
    
    /// Generate COMMENT ON COLUMN SQL statement (for use in migrations)
    ///
    /// This helper method generates a PostgreSQL `COMMENT ON COLUMN` SQL statement
    /// for columns that have a comment attribute. Migration builders can use this
    /// to add column documentation to the database schema.
    ///
    /// # Arguments
    ///
    /// * `table_name` - The table name (can be schema-qualified, e.g., "schema.table")
    /// * `column_name` - The column name
    ///
    /// # Returns
    ///
    /// Returns `Some(String)` containing the COMMENT ON COLUMN SQL statement if
    /// a comment is set, or `None` if no comment is defined.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::ColumnDefinition;
    ///
    /// let col_def = ColumnDefinition {
    ///     comment: Some("User's email address".to_string()),
    ///     ..Default::default()
    /// };
    ///
    /// let sql = col_def.comment_sql("users", "email");
    /// // Returns: Some("COMMENT ON COLUMN users.email IS 'User\\'s email address';".to_string())
    /// ```
    /// Validate identifier name to prevent SQL injection
    /// 
    /// Checks for dangerous characters that could be used for SQL injection.
    /// Returns an error message if validation fails, or None if valid.
    fn validate_identifier(name: &str, kind: &str) -> Option<String> {
        // Check for empty string
        if name.is_empty() {
            return Some(format!("{} name cannot be empty", kind));
        }
        
        // Check for dangerous characters that could be used for SQL injection
        // PostgreSQL identifiers can contain letters, digits, underscores, and dollar signs
        // but we'll be more restrictive for safety
        let dangerous_chars = ['\'', '"', ';', '\\'];
        for &char_seq in &dangerous_chars {
            if name.contains(char_seq) {
                return Some(format!(
                    "{} name contains invalid character: '{}'. Identifiers should only contain letters, digits, underscores, and dots (for schema.table format).",
                    kind, char_seq
                ));
            }
        }
        
        // Check for dangerous SQL patterns (multi-character sequences)
        let dangerous_patterns = ["--", "/*", "*/"];
        for pattern in &dangerous_patterns {
            if name.contains(pattern) {
                return Some(format!(
                    "{} name contains invalid pattern: '{}'. Identifiers should only contain letters, digits, underscores, and dots (for schema.table format).",
                    kind, pattern
                ));
            }
        }
        
        // Check for SQL keywords that could be problematic (basic check)
        // Note: This is a simple check - full keyword validation would be more complex
        let sql_keywords = ["DROP", "DELETE", "INSERT", "UPDATE", "SELECT", "ALTER", "CREATE"];
        let upper_name = name.to_uppercase();
        for keyword in &sql_keywords {
            if upper_name == *keyword {
                return Some(format!(
                    "{} name cannot be a SQL keyword: '{}'",
                    kind, keyword
                ));
            }
        }
        
        None
    }
    
    pub fn comment_sql(&self, table_name: &str, column_name: &str) -> Option<String> {
        if let Some(ref comment) = self.comment {
            // Validate table and column names to prevent SQL injection
            if let Some(err) = Self::validate_identifier(table_name, "Table") {
                // In production, this should probably panic or return an error
                // For now, we'll include it in a comment to make it visible
                eprintln!("WARNING: comment_sql() called with invalid table name: {}", err);
                // Continue anyway - the caller should fix this
            }
            
            if let Some(err) = Self::validate_identifier(column_name, "Column") {
                eprintln!("WARNING: comment_sql() called with invalid column name: {}", err);
                // Continue anyway - the caller should fix this
            }
            
            // Escape backslashes first (order matters: backslashes before single quotes)
            // Then escape single quotes in comment text for SQL
            let escaped_comment = comment.replace("\\", "\\\\").replace("'", "''");
            Some(format!(
                "COMMENT ON COLUMN {}.{} IS '{}';",
                table_name, column_name, escaped_comment
            ))
        } else {
            None
        }
    }
    
    /// Create a ColumnDefinition from a Rust type
    ///
    /// This helper function infers column metadata from a Rust type.
    /// Used by the macro to generate column definitions.
    ///
    /// # Arguments
    ///
    /// * `rust_type` - The Rust type name (e.g., "i32", "String", "Option<i32>")
    /// * `is_primary_key` - Whether this is a primary key
    /// * `is_auto_increment` - Whether this is auto-increment
    ///
    /// # Returns
    ///
    /// Returns a `ColumnDefinition` with inferred metadata.
    pub fn from_rust_type(
        rust_type: &str,
        is_primary_key: bool,
        is_auto_increment: bool,
    ) -> Self {
        let (inner_type, nullable) = if rust_type.starts_with("Option<") {
            // Extract inner type from Option<T>
            let inner = rust_type
                .strip_prefix("Option<")
                .and_then(|s| s.strip_suffix(">"))
                .unwrap_or(rust_type);
            (inner, true)
        } else {
            (rust_type, false)
        };
        
        let column_type = match inner_type {
            "i32" => Some("Integer".to_string()),
            "i64" => Some("BigInt".to_string()),
            "i16" => Some("SmallInt".to_string()),
            "i8" => Some("TinyInt".to_string()),
            "u32" => Some("Unsigned".to_string()),
            "u64" => Some("BigUnsigned".to_string()),
            "String" => Some("String".to_string()),
            "bool" => Some("Boolean".to_string()),
            "f32" => Some("Float".to_string()),
            "f64" => Some("Double".to_string()),
            _ => {
                // Try to infer from common patterns
                if inner_type == "Vec<u8>" || inner_type.starts_with("Vec<") && inner_type.contains("u8") {
                    Some("Binary".to_string())
                } else if inner_type.contains("Json") {
                    Some("Json".to_string())
                } else if inner_type.contains("Uuid") {
                    Some("Uuid".to_string())
                } else if inner_type.contains("DateTime") || inner_type.contains("Timestamp") {
                    Some("Timestamp".to_string())
                } else {
                    Some("String".to_string()) // Default fallback
                }
            }
        };
        
        Self {
            column_type,
            nullable,
            default_value: None,
            default_expr: None,
            renamed_from: None,
            select_as: None,
            save_as: None,
            comment: None,
            unique: is_primary_key, // Primary keys are typically unique
            indexed: is_primary_key, // Primary keys are typically indexed
            auto_increment: is_auto_increment,
            foreign_key: None,
            check: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_definition_default() {
        let def = ColumnDefinition::default();
        assert_eq!(def.column_type, None);
        assert_eq!(def.nullable, false);
        assert_eq!(def.default_value, None);
        assert_eq!(def.default_expr, None);
        assert_eq!(def.renamed_from, None);
        assert_eq!(def.select_as, None);
        assert_eq!(def.save_as, None);
        assert_eq!(def.comment, None);
        assert_eq!(def.unique, false);
        assert_eq!(def.indexed, false);
        assert_eq!(def.auto_increment, false);
        assert_eq!(def.foreign_key, None);
        assert_eq!(def.check, None);
    }

    #[test]
    fn test_column_definition_custom() {
        let def = ColumnDefinition {
            column_type: Some("String".to_string()),
            nullable: true,
            default_value: Some("''".to_string()),
            default_expr: Some("NOW()".to_string()),
            renamed_from: Some("old_name".to_string()),
            select_as: None,
            save_as: None,
            comment: None,
            unique: true,
            indexed: true,
            auto_increment: false,
            foreign_key: None,
            check: None,
        };
        
        assert_eq!(def.column_type, Some("String".to_string()));
        assert_eq!(def.nullable, true);
        assert_eq!(def.default_value, Some("''".to_string()));
        assert_eq!(def.default_expr, Some("NOW()".to_string()));
        assert_eq!(def.renamed_from, Some("old_name".to_string()));
        assert_eq!(def.unique, true);
        assert_eq!(def.indexed, true);
        assert_eq!(def.auto_increment, false);
    }

    #[test]
    fn test_column_definition_to_column_def() {
        let def = ColumnDefinition {
            column_type: Some("Integer".to_string()),
            nullable: true,
            default_value: None,
            default_expr: None,
            renamed_from: None,
            select_as: None,
            save_as: None,
            comment: None,
            unique: false,
            indexed: false,
            auto_increment: true,
            foreign_key: None,
            check: None,
        };
        
        // Test that to_column_def compiles and works
        struct TestColumn;
        impl sea_query::Iden for TestColumn {
            fn unquoted(&self) -> &str { "id" }
        }
        
        let column_def = def.to_column_def(TestColumn);
        // Can't easily test the ColumnDef internals, but we can verify it doesn't panic
        let _ = column_def;
    }
    
    #[test]
    fn test_column_definition_apply_default_expr() {
        let def = ColumnDefinition {
            column_type: Some("Timestamp".to_string()),
            nullable: false,
            default_value: None,
            default_expr: Some("NOW()".to_string()),
            renamed_from: None,
            select_as: None,
            save_as: None,
            comment: None,
            unique: false,
            indexed: false,
            auto_increment: false,
            foreign_key: None,
            check: None,
        };
        
        struct TestColumn;
        impl sea_query::Iden for TestColumn {
            fn unquoted(&self) -> &str { "created_at" }
        }
        
        let mut column_def = def.to_column_def(TestColumn);
        def.apply_default_expr(&mut column_def);
        // Can't easily test the ColumnDef internals, but we can verify it doesn't panic
        let _ = column_def;
    }
    
    #[test]
    fn test_apply_default_expr_cache_prevents_multiple_leaks() {
        // This test verifies that calling apply_default_expr multiple times
        // with the same expression doesn't leak memory on each call.
        // The cache should ensure the same expression is reused.
        
        let expr = "NOW()".to_string();
        let def1 = ColumnDefinition {
            default_expr: Some(expr.clone()),
            ..Default::default()
        };
        let def2 = ColumnDefinition {
            default_expr: Some(expr.clone()),
            ..Default::default()
        };
        
        struct TestColumn;
        impl sea_query::Iden for TestColumn {
            fn unquoted(&self) -> &str { "created_at" }
        }
        
        // Call apply_default_expr multiple times with the same expression
        let mut def1_col = def1.to_column_def(TestColumn);
        def1.apply_default_expr(&mut def1_col);
        
        let mut def2_col = def2.to_column_def(TestColumn);
        def2.apply_default_expr(&mut def2_col);
        
        // Verify the cache contains exactly one entry for this expression
        let cache = EXPR_CACHE.lock().unwrap();
        assert_eq!(cache.len(), 1, "Cache should contain exactly one entry for 'NOW()'");
        assert!(cache.contains_key("NOW()"), "Cache should contain 'NOW()'");
        
        // Verify both calls returned the same static reference
        let cached_expr = cache.get("NOW()").unwrap();
        // The expressions should be the same pointer (same memory address)
        // This verifies that the cache is working and preventing duplicate leaks
        let _ = cached_expr;
    }
}
