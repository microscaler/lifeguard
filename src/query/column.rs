//! Type-safe column operations for query building
//!
//! This module provides traits and implementations for type-safe column operations
//! that match SeaORM's API. Columns can be used in filters with compile-time type checking.

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
}

// Implement ColumnTrait for all types that implement IntoColumnRef
impl<T: IntoColumnRef> ColumnTrait for T {}
