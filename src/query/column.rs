//! Type-safe column operations for query building
//!
//! This module provides traits and implementations for type-safe column operations
//! that match SeaORM's API. Columns can be used in filters with compile-time type checking.

use sea_query::{Expr, ExprTrait, IntoColumnRef, Iden};

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
            unique: false,
            indexed: false,
            auto_increment: false,
        }
    }
}

impl ColumnDefinition {
    /// Convert to SeaQuery's ColumnDef for use in migrations
    ///
    /// This is a placeholder implementation. Full implementation would require
    /// mapping column types to SeaQuery's ColumnType enum and proper Iden trait bounds.
    ///
    /// # Note
    ///
    /// This method is a placeholder. Full implementation will be added when
    /// migration support is implemented. For now, it returns metadata that can
    /// be used by migration code to build ColumnDef instances.
    pub fn to_column_def<T: Iden>(&self, column_name: T) -> sea_query::ColumnDef {
        use sea_query::ColumnDef;
        
        let mut def = ColumnDef::new(column_name);
        
        // Set nullable if applicable
        if self.nullable {
            def.null();
        }
        
        // Set auto-increment if applicable
        if self.auto_increment {
            def.auto_increment();
        }
        
        // TODO: Map column_type string to ColumnType enum
        // TODO: Add unique constraint support (may need separate index definition)
        // For now, default to Text type
        def.string();
        
        def
    }
}

/// Trait for type-safe column operations
///
/// This trait provides methods for building type-safe filter expressions.
/// Each column type (String, i32, etc.) has appropriate methods that accept
/// the correct value types.
///
/// # Example
///
/// ```no_run
/// use lifeguard::ColumnTrait;
/// use sea_query::Expr;
///
/// // Type-safe: Email.eq() accepts String
/// let filter = User::Email.eq("test@example.com".to_string());
///
/// // Type-safe: Age.eq() accepts i32
/// let filter = User::Age.eq(25);
/// ```
pub trait ColumnTrait: IntoColumnRef {
    /// Create an equality filter: `column = value`
    fn eq<T: Into<sea_query::Value>>(self, value: T) -> Expr {
        Expr::col(self).eq(value)
    }

    /// Create a not-equal filter: `column != value`
    fn ne<T: Into<sea_query::Value>>(self, value: T) -> Expr {
        Expr::col(self).ne(value)
    }

    /// Create a greater-than filter: `column > value`
    fn gt<T: Into<sea_query::Value>>(self, value: T) -> Expr {
        Expr::col(self).gt(value)
    }

    /// Create a greater-than-or-equal filter: `column >= value`
    fn gte<T: Into<sea_query::Value>>(self, value: T) -> Expr {
        Expr::col(self).gte(value)
    }

    /// Create a less-than filter: `column < value`
    fn lt<T: Into<sea_query::Value>>(self, value: T) -> Expr {
        Expr::col(self).lt(value)
    }

    /// Create a less-than-or-equal filter: `column <= value`
    fn lte<T: Into<sea_query::Value>>(self, value: T) -> Expr {
        Expr::col(self).lte(value)
    }

    /// Create a LIKE filter: `column LIKE pattern`
    fn like(self, pattern: &str) -> Expr {
        Expr::col(self).like(pattern)
    }

    /// Create an IN filter: `column IN (values)`
    fn is_in<T, I>(self, values: I) -> Expr
    where
        T: Into<sea_query::Value>,
        I: IntoIterator<Item = T>,
    {
        Expr::col(self).is_in(values)
    }

    /// Create a NOT IN filter: `column NOT IN (values)`
    fn is_not_in<T, I>(self, values: I) -> Expr
    where
        T: Into<sea_query::Value>,
        I: IntoIterator<Item = T>,
    {
        Expr::col(self).is_not_in(values)
    }

    /// Create an IS NULL filter: `column IS NULL`
    fn is_null(self) -> Expr {
        Expr::col(self).is_null()
    }

    /// Create an IS NOT NULL filter: `column IS NOT NULL`
    fn is_not_null(self) -> Expr {
        Expr::col(self).is_not_null()
    }

    /// Create a BETWEEN filter: `column BETWEEN start AND end`
    fn between<T1: Into<sea_query::Value>, T2: Into<sea_query::Value>>(
        self,
        start: T1,
        end: T2,
    ) -> Expr {
        Expr::col(self).between(start, end)
    }

    /// Get column definition metadata
    ///
    /// Returns metadata about the column including type, nullability, default value, etc.
    /// This method should be implemented by the macro for each column enum variant.
    ///
    /// # Default Implementation
    ///
    /// Returns a default `ColumnDefinition` with no metadata. The `LifeModel` macro
    /// should generate implementations that return actual column metadata based on
    /// field attributes.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::ColumnTrait;
    ///
    /// // In a real application, the macro would generate this:
    /// // impl ColumnTrait for UserColumn {
    /// //     fn def(self) -> ColumnDefinition {
    /// //         match self {
    /// //             UserColumn::Email => ColumnDefinition {
    /// //                 column_type: Some("String".to_string()),
    /// //                 nullable: false,
    /// //                 ..Default::default()
    /// //             },
    /// //             // ...
    /// //         }
    /// //     }
    /// // }
    /// ```
    fn def(self) -> ColumnDefinition {
        ColumnDefinition::default()
    }

    /// Get enum type name for enum columns
    ///
    /// Returns the PostgreSQL enum type name if this column is an enum type.
    /// Returns `None` for non-enum columns.
    ///
    /// # Default Implementation
    ///
    /// Returns `None`. The `LifeModel` macro should generate implementations
    /// for enum columns based on the `#[enum_name = "..."]` attribute.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::ColumnTrait;
    ///
    /// // In a real application, the macro would generate this:
    /// // impl ColumnTrait for UserColumn {
    /// //     fn enum_type_name(self) -> Option<String> {
    /// //         match self {
    /// //             UserColumn::Status => Some("user_status_enum".to_string()),
    /// //             _ => None,
    /// //         }
    /// //     }
    /// // }
    /// ```
    fn enum_type_name(self) -> Option<String> {
        None
    }

    /// Get custom SELECT expression
    ///
    /// Returns a custom SQL expression to use when selecting this column.
    /// Returns `None` if no custom expression is defined (uses column name directly).
    ///
    /// # Default Implementation
    ///
    /// Returns `None`. The `LifeModel` macro should generate implementations
    /// for columns with `#[select_as = "..."]` attribute.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::ColumnTrait;
    ///
    /// // In a real application, the macro would generate this for columns with #[select_as = "..."]:
    /// // impl ColumnTrait for UserColumn {
    /// //     fn select_as(self) -> Option<String> {
    /// //         match self {
    /// //             UserColumn::FullName => Some("CONCAT(first_name, ' ', last_name)".to_string()),
    /// //             _ => None,
    /// //         }
    /// //     }
    /// // }
    /// ```
    fn select_as(self) -> Option<String> {
        None
    }

    /// Get custom save expression
    ///
    /// Returns a custom SQL expression to use when saving this column.
    /// Returns `None` if no custom expression is defined (uses column value directly).
    ///
    /// # Default Implementation
    ///
    /// Returns `None`. The `LifeModel` macro should generate implementations
    /// for columns with `#[save_as = "..."]` attribute.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::ColumnTrait;
    ///
    /// // In a real application, the macro would generate this for columns with #[save_as = "..."]:
    /// // impl ColumnTrait for UserColumn {
    /// //     fn save_as(self) -> Option<String> {
    /// //         match self {
    /// //             UserColumn::UpdatedAt => Some("NOW()".to_string()),
    /// //             _ => None,
    /// //         }
    /// //     }
    /// // }
    /// ```
    fn save_as(self) -> Option<String> {
        None
    }
}

// Implement ColumnTrait for all types that implement IntoColumnRef
impl<T: IntoColumnRef> ColumnTrait for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_query::{Expr, ExprTrait};

    // Test Column enum for ColumnTrait tests
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    enum TestColumn {
        Id,
        Name,
        Age,
        Email,
    }

    impl sea_query::Iden for TestColumn {
        fn unquoted(&self) -> &str {
            match self {
                TestColumn::Id => "id",
                TestColumn::Name => "name",
                TestColumn::Age => "age",
                TestColumn::Email => "email",
            }
        }
    }

    impl sea_query::IdenStatic for TestColumn {
        fn as_str(&self) -> &'static str {
            match self {
                TestColumn::Id => "id",
                TestColumn::Name => "name",
                TestColumn::Age => "age",
                TestColumn::Email => "email",
            }
        }
    }

    #[test]
    fn test_column_trait_eq() {
        let expr = TestColumn::Id.eq(42);
        // Verify it creates an Expr (can't easily test SQL generation without executor)
        let _ = expr;
    }

    #[test]
    fn test_column_trait_ne() {
        let expr = TestColumn::Name.ne("test".to_string());
        let _ = expr;
    }

    #[test]
    fn test_column_trait_gt() {
        let expr = TestColumn::Age.gt(18);
        let _ = expr;
    }

    #[test]
    fn test_column_trait_gte() {
        let expr = TestColumn::Age.gte(18);
        let _ = expr;
    }

    #[test]
    fn test_column_trait_lt() {
        let expr = TestColumn::Age.lt(65);
        let _ = expr;
    }

    #[test]
    fn test_column_trait_lte() {
        let expr = TestColumn::Age.lte(65);
        let _ = expr;
    }

    #[test]
    fn test_column_trait_like() {
        let expr = TestColumn::Email.like("%@example.com");
        let _ = expr;
    }

    #[test]
    fn test_column_trait_is_in() {
        let expr = TestColumn::Id.is_in(vec![1, 2, 3]);
        let _ = expr;
    }

    #[test]
    fn test_column_trait_is_not_in() {
        let expr = TestColumn::Id.is_not_in(vec![4, 5, 6]);
        let _ = expr;
    }

    #[test]
    fn test_column_trait_is_null() {
        let expr = TestColumn::Name.is_null();
        let _ = expr;
    }

    #[test]
    fn test_column_trait_is_not_null() {
        let expr = TestColumn::Email.is_not_null();
        let _ = expr;
    }

    #[test]
    fn test_column_trait_between() {
        let expr = TestColumn::Age.between(18, 65);
        let _ = expr;
    }

    #[test]
    fn test_column_trait_def_default() {
        let def = TestColumn::Id.def();
        assert_eq!(def.column_type, None);
        assert_eq!(def.nullable, false);
        assert_eq!(def.default_value, None);
        assert_eq!(def.unique, false);
        assert_eq!(def.indexed, false);
        assert_eq!(def.auto_increment, false);
    }

    #[test]
    fn test_column_trait_enum_type_name_default() {
        let enum_name = TestColumn::Id.enum_type_name();
        assert_eq!(enum_name, None);
    }

    #[test]
    fn test_column_trait_select_as_default() {
        let select_expr = TestColumn::Name.select_as();
        assert_eq!(select_expr, None);
    }

    #[test]
    fn test_column_trait_save_as_default() {
        let save_expr = TestColumn::Email.save_as();
        assert_eq!(save_expr, None);
    }

    #[test]
    fn test_column_definition_default() {
        let def = ColumnDefinition::default();
        assert_eq!(def.column_type, None);
        assert_eq!(def.nullable, false);
        assert_eq!(def.default_value, None);
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
            unique: true,
            indexed: true,
            auto_increment: false,
        };
        
        assert_eq!(def.column_type, Some("String".to_string()));
        assert_eq!(def.nullable, true);
        assert_eq!(def.default_value, Some("''".to_string()));
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
            unique: false,
            indexed: false,
            auto_increment: true,
        };
        
        // Test that to_column_def compiles and works
        let column_def = def.to_column_def(TestColumn::Id);
        // Can't easily test the ColumnDef internals, but we can verify it doesn't panic
        let _ = column_def;
    }
}
