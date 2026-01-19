//! Column definition metadata and type inference.
//!
//! This module provides `ColumnDefinition` which stores metadata about database columns
//! including type, nullability, default values, and constraints. It also provides
//! utilities for inferring column definitions from Rust types.

use super::type_mapping;
use sea_query::{ColumnDef, Iden};

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
            // Create a static string by leaking the string (for migration use only)
            // This is safe because migrations are typically run once at startup
            let static_str: &'static str = Box::leak(expr_str.clone().into_boxed_str());
            use sea_query::Expr;
            let expr = Expr::cust(static_str);
            def.default(expr);
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
}
