//! Column trait for type-safe query building.
//!
//! This module provides `ColumnTrait` which enables type-safe column operations
//! for building filter expressions. Columns can be used in filters with compile-time
//! type checking.

use super::definition::ColumnDefinition;
use sea_query::{Expr, ExprTrait, IntoColumnRef};

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
    #[allow(clippy::wrong_self_convention)]
    fn is_in<T, I>(self, values: I) -> Expr
    where
        T: Into<sea_query::Value>,
        I: IntoIterator<Item = T>,
    {
        Expr::col(self).is_in(values)
    }

    /// Create a NOT IN filter: `column NOT IN (values)`
    #[allow(clippy::wrong_self_convention)]
    fn is_not_in<T, I>(self, values: I) -> Expr
    where
        T: Into<sea_query::Value>,
        I: IntoIterator<Item = T>,
    {
        Expr::col(self).is_not_in(values)
    }

    /// Create an IS NULL filter: `column IS NULL`
    #[allow(clippy::wrong_self_convention)]
    fn is_null(self) -> Expr {
        Expr::col(self).is_null()
    }

    /// Create an IS NOT NULL filter: `column IS NOT NULL`
    #[allow(clippy::wrong_self_convention)]
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
    /// Returns the `PostgreSQL` enum type name if this column is an enum type.
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

/// Helper trait for accessing `column_def()` method in generic code
///
/// This trait provides a way to call the inherent `column_def()` method
/// that is generated by the `LifeModel` macro. The macro should implement
/// this trait for each `Column` enum.
///
/// # Note
///
/// This trait exists to work around the limitation that inherent methods
/// cannot be called on associated types in generic code. The macro generates
/// `column_def()` as an inherent method, and this trait provides a way to
/// access it generically.
pub trait ColumnDefHelper: Copy {
    /// Get column definition metadata (generated by `LifeModel` macro)
    ///
    /// This method should be implemented by the macro for each `Column` enum.
    /// It returns the column definition including `select_as`, `save_as`, etc.
    fn column_def(self) -> ColumnDefinition;
}

/// Helper macro for implementing `ColumnDefHelper` for test columns
///
/// This macro provides a default implementation that returns `ColumnDefinition::default()`.
/// It's useful for test code that manually defines Column enums.
#[macro_export]
macro_rules! impl_column_def_helper_for_test {
    ($column_type:ty) => {
        impl $crate::query::column::column_trait::ColumnDefHelper for $column_type {
            fn column_def(self) -> $crate::query::column::definition::ColumnDefinition {
                $crate::query::column::definition::ColumnDefinition::default()
            }
        }
    };
}

// Implement ColumnTrait for all types that implement IntoColumnRef
// NOTE: This blanket impl conflicts with macro-generated impls for Column enums.
// The macro generates specific impls that override def() and enum_type_name().
// For now, the macro-generated impls will conflict. We need a better solution.
// TODO: Use specialization (when stable) or change the trait design to avoid conflicts.
impl<T: IntoColumnRef> ColumnTrait for T {}

#[cfg(test)]
mod tests {
    use super::*;

    // Test Column enum for ColumnTrait tests
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    enum TestColumn {
        Id,
        Name,
        Age,
        Email,
    }

    impl sea_query::Iden for TestColumn {
        fn unquoted(&self) -> &'static str {
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
    
    crate::impl_column_def_helper_for_test!(TestColumn);

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
        assert!(!def.nullable);
        assert_eq!(def.default_value, None);
        assert!(!def.unique);
        assert!(!def.indexed);
        assert!(!def.auto_increment);
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
}
