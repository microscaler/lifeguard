//! Tuple conversion traits for composite keys
//!
//! These traits provide conversion between Rust tuples and `sea_query::Value` tuples
//! for handling composite primary keys and other multi-column operations.

use sea_query::Value;
use crate::value::{ValueType, ValueExtractionError, TryGetable};

/// Trait for converting tuples to `Value` tuples
///
/// This trait enables conversion of Rust tuples (e.g., `(i32, String)`) to tuples
/// of `Value` enums (e.g., `(Value, Value)`). This is essential for composite primary
/// keys and multi-column operations.
///
/// ## Usage
///
/// ```rust
/// use lifeguard::IntoValueTuple;
/// use sea_query::Value;
///
/// let tuple = (42i32, "hello".to_string());
/// let value_tuple: (Value, Value) = tuple.into_value_tuple();
/// ```
pub trait IntoValueTuple {
    /// The type of the `Value` tuple (e.g., `(Value, Value)` for a 2-tuple)
    type ValueTuple;
    
    /// Convert this tuple into a tuple of `Value` enums
    fn into_value_tuple(self) -> Self::ValueTuple;
}

/// Trait for converting `Value` tuples back to Rust tuples
///
/// This trait enables conversion of tuples of `Value` enums back to Rust tuples.
/// This is the inverse operation of `IntoValueTuple`.
///
/// ## Usage
///
/// ```rust
/// use lifeguard::FromValueTuple;
/// use sea_query::Value;
///
/// let value_tuple = (Value::Int(Some(42)), Value::String(Some("hello".to_string())));
/// let result: Result<(i32, String), _> = FromValueTuple::from_value_tuple(value_tuple);
/// assert_eq!(result, Ok((42, "hello".to_string())));
/// ```
pub trait FromValueTuple: Sized {
    /// The type of the `Value` tuple (e.g., `(Value, Value)` for a 2-tuple)
    type ValueTuple;
    
    /// Try to convert a tuple of `Value` enums into this tuple type
    ///
    /// Returns:
    /// - `Ok(Self)` if all values can be converted successfully
    /// - `Err(ValueExtractionError)` if any value fails to convert
    fn from_value_tuple(value_tuple: Self::ValueTuple) -> Result<Self, ValueExtractionError>;
}

// Implementations for tuples of size 2-6 (matching PrimaryKeyArity)

impl<A: ValueType, B: ValueType> IntoValueTuple for (A, B) {
    type ValueTuple = (Value, Value);
    
    fn into_value_tuple(self) -> Self::ValueTuple {
        (A::into_value(self.0), B::into_value(self.1))
    }
}

impl<A: TryGetable, B: TryGetable> FromValueTuple for (A, B) {
    type ValueTuple = (Value, Value);
    
    fn from_value_tuple(value_tuple: Self::ValueTuple) -> Result<Self, ValueExtractionError> {
        Ok((
            A::try_get(value_tuple.0)?,
            B::try_get(value_tuple.1)?,
        ))
    }
}

impl<A: ValueType, B: ValueType, C: ValueType> IntoValueTuple for (A, B, C) {
    type ValueTuple = (Value, Value, Value);
    
    fn into_value_tuple(self) -> Self::ValueTuple {
        (A::into_value(self.0), B::into_value(self.1), C::into_value(self.2))
    }
}

impl<A: TryGetable, B: TryGetable, C: TryGetable> FromValueTuple for (A, B, C) {
    type ValueTuple = (Value, Value, Value);
    
    fn from_value_tuple(value_tuple: Self::ValueTuple) -> Result<Self, ValueExtractionError> {
        Ok((
            A::try_get(value_tuple.0)?,
            B::try_get(value_tuple.1)?,
            C::try_get(value_tuple.2)?,
        ))
    }
}

impl<A: ValueType, B: ValueType, C: ValueType, D: ValueType> IntoValueTuple for (A, B, C, D) {
    type ValueTuple = (Value, Value, Value, Value);
    
    fn into_value_tuple(self) -> Self::ValueTuple {
        (
            A::into_value(self.0),
            B::into_value(self.1),
            C::into_value(self.2),
            D::into_value(self.3),
        )
    }
}

impl<A: TryGetable, B: TryGetable, C: TryGetable, D: TryGetable> FromValueTuple for (A, B, C, D) {
    type ValueTuple = (Value, Value, Value, Value);
    
    fn from_value_tuple(value_tuple: Self::ValueTuple) -> Result<Self, ValueExtractionError> {
        Ok((
            A::try_get(value_tuple.0)?,
            B::try_get(value_tuple.1)?,
            C::try_get(value_tuple.2)?,
            D::try_get(value_tuple.3)?,
        ))
    }
}

impl<A: ValueType, B: ValueType, C: ValueType, D: ValueType, E: ValueType> IntoValueTuple for (A, B, C, D, E) {
    type ValueTuple = (Value, Value, Value, Value, Value);
    
    fn into_value_tuple(self) -> Self::ValueTuple {
        (
            A::into_value(self.0),
            B::into_value(self.1),
            C::into_value(self.2),
            D::into_value(self.3),
            E::into_value(self.4),
        )
    }
}

impl<A: TryGetable, B: TryGetable, C: TryGetable, D: TryGetable, E: TryGetable> FromValueTuple for (A, B, C, D, E) {
    type ValueTuple = (Value, Value, Value, Value, Value);
    
    fn from_value_tuple(value_tuple: Self::ValueTuple) -> Result<Self, ValueExtractionError> {
        Ok((
            A::try_get(value_tuple.0)?,
            B::try_get(value_tuple.1)?,
            C::try_get(value_tuple.2)?,
            D::try_get(value_tuple.3)?,
            E::try_get(value_tuple.4)?,
        ))
    }
}

impl<A: ValueType, B: ValueType, C: ValueType, D: ValueType, E: ValueType, F: ValueType> IntoValueTuple for (A, B, C, D, E, F) {
    type ValueTuple = (Value, Value, Value, Value, Value, Value);
    
    fn into_value_tuple(self) -> Self::ValueTuple {
        (
            A::into_value(self.0),
            B::into_value(self.1),
            C::into_value(self.2),
            D::into_value(self.3),
            E::into_value(self.4),
            F::into_value(self.5),
        )
    }
}

impl<A: TryGetable, B: TryGetable, C: TryGetable, D: TryGetable, E: TryGetable, F: TryGetable> FromValueTuple for (A, B, C, D, E, F) {
    type ValueTuple = (Value, Value, Value, Value, Value, Value);
    
    fn from_value_tuple(value_tuple: Self::ValueTuple) -> Result<Self, ValueExtractionError> {
        Ok((
            A::try_get(value_tuple.0)?,
            B::try_get(value_tuple.1)?,
            C::try_get(value_tuple.2)?,
            D::try_get(value_tuple.3)?,
            E::try_get(value_tuple.4)?,
            F::try_get(value_tuple.5)?,
        ))
    }
}

// For tuples with 6+ elements, we use a Vec<Value> representation
// This matches PrimaryKeyArity::Tuple6Plus

impl IntoValueTuple for Vec<Value> {
    type ValueTuple = Vec<Value>;
    
    fn into_value_tuple(self) -> Self::ValueTuple {
        self
    }
}

impl FromValueTuple for Vec<Value> {
    type ValueTuple = Vec<Value>;
    
    fn from_value_tuple(value_tuple: Self::ValueTuple) -> Result<Self, ValueExtractionError> {
        Ok(value_tuple)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_into_value_tuple_2() {
        let tuple = (42i32, "hello".to_string());
        let value_tuple = tuple.into_value_tuple();
        
        assert!(matches!(value_tuple.0, Value::Int(Some(42))));
        assert!(matches!(value_tuple.1, Value::String(Some(ref s)) if s == "hello"));
    }
    
    #[test]
    fn test_from_value_tuple_2() {
        let value_tuple = (
            Value::Int(Some(42)),
            Value::String(Some("hello".to_string())),
        );
        let result: Result<(i32, String), _> = FromValueTuple::from_value_tuple(value_tuple);
        assert_eq!(result, Ok((42, "hello".to_string())));
    }
    
    #[test]
    fn test_into_value_tuple_3() {
        let tuple = (1i32, 2i32, 3i32);
        let value_tuple = tuple.into_value_tuple();
        
        assert!(matches!(value_tuple.0, Value::Int(Some(1))));
        assert!(matches!(value_tuple.1, Value::Int(Some(2))));
        assert!(matches!(value_tuple.2, Value::Int(Some(3))));
    }
    
    #[test]
    fn test_from_value_tuple_3() {
        let value_tuple = (
            Value::Int(Some(1)),
            Value::Int(Some(2)),
            Value::Int(Some(3)),
        );
        let result: Result<(i32, i32, i32), _> = FromValueTuple::from_value_tuple(value_tuple);
        assert_eq!(result, Ok((1, 2, 3)));
    }
    
    #[test]
    fn test_from_value_tuple_error() {
        let value_tuple = (
            Value::Int(Some(42)),
            Value::String(Some("hello".to_string())),
        );
        let result: Result<(i32, i32), _> = FromValueTuple::from_value_tuple(value_tuple);
        assert!(matches!(result, Err(ValueExtractionError::TypeMismatch { .. })));
    }
    
    #[test]
    fn test_mixed_type_tuple() {
        let tuple = (42i32, "hello".to_string(), true);
        let value_tuple = tuple.into_value_tuple();
        
        assert!(matches!(value_tuple.0, Value::Int(Some(42))));
        assert!(matches!(value_tuple.1, Value::String(Some(ref s)) if s == "hello"));
        assert!(matches!(value_tuple.2, Value::Bool(Some(true))));
        
        let result: Result<(i32, String, bool), _> = FromValueTuple::from_value_tuple(value_tuple);
        assert_eq!(result, Ok((42, "hello".to_string(), true)));
    }
}
