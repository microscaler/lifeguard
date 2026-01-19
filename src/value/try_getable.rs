//! TryGetable and TryGetableMany traits for safe value extraction
//!
//! These traits provide safe, error-aware extraction of values from `sea_query::Value`
//! with proper error handling and type checking.

use sea_query::Value;
use crate::value::ValueType;

/// Error type for value extraction failures
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueExtractionError {
    /// The value is null (None variant)
    NullValue,
    /// The value type doesn't match the expected type
    TypeMismatch {
        expected: String,
        actual: String,
    },
    /// Value conversion failed (e.g., overflow, invalid format)
    ConversionError(String),
}

impl std::fmt::Display for ValueExtractionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueExtractionError::NullValue => write!(f, "Value is null"),
            ValueExtractionError::TypeMismatch { expected, actual } => {
                write!(f, "Type mismatch: expected {}, got {}", expected, actual)
            }
            ValueExtractionError::ConversionError(msg) => {
                write!(f, "Conversion error: {}", msg)
            }
        }
    }
}

impl std::error::Error for ValueExtractionError {}

/// Trait for safe value extraction with error handling
///
/// This trait extends `ValueType` with error-aware extraction methods that return
/// `Result<T, ValueExtractionError>` instead of `Option<T>`. This provides better
/// error messages and distinguishes between null values and type mismatches.
///
/// ## Usage
///
/// ```rust
/// use lifeguard::{TryGetable, ValueExtractionError};
/// use sea_query::Value;
///
/// let value = Value::Int(Some(42));
/// let result: Result<i32, ValueExtractionError> = TryGetable::try_get(value);
/// assert_eq!(result, Ok(42));
///
/// let null_value = Value::Int(None);
/// let result: Result<i32, ValueExtractionError> = TryGetable::try_get(null_value);
/// assert!(matches!(result, Err(ValueExtractionError::NullValue)));
/// ```
pub trait TryGetable: ValueType {
    /// Try to extract a value from `sea_query::Value`, returning an error if extraction fails.
    ///
    /// Returns:
    /// - `Ok(T)` if the value matches the expected type and is not null
    /// - `Err(ValueExtractionError::NullValue)` if the value is null
    /// - `Err(ValueExtractionError::TypeMismatch)` if the value type doesn't match
    /// - `Err(ValueExtractionError::ConversionError)` if conversion fails (e.g., overflow)
    fn try_get(value: Value) -> Result<Self, ValueExtractionError>;
    
    /// Try to extract a value, allowing null values to return `None`.
    ///
    /// Returns:
    /// - `Ok(Some(T))` if the value matches and is not null
    /// - `Ok(None)` if the value is null
    /// - `Err(...)` if the value type doesn't match or conversion fails
    fn try_get_opt(value: Value) -> Result<Option<Self>, ValueExtractionError> {
        match Self::try_get(value) {
            Ok(v) => Ok(Some(v)),
            Err(ValueExtractionError::NullValue) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// Implement TryGetable for all types that implement ValueType

macro_rules! impl_try_getable {
    ($type:ty, $variant:ident, $expected:expr) => {
        impl TryGetable for $type {
            fn try_get(value: Value) -> Result<Self, ValueExtractionError> {
                match value {
                    Value::$variant(Some(v)) => Ok(v),
                    Value::$variant(None) => Err(ValueExtractionError::NullValue),
                    _ => Err(ValueExtractionError::TypeMismatch {
                        expected: $expected.to_string(),
                        actual: format!("{:?}", value),
                    }),
                }
            }
        }
    };
}

impl_try_getable!(i8, TinyInt, "TinyInt");
impl_try_getable!(i16, SmallInt, "SmallInt");
impl_try_getable!(i32, Int, "Int");
impl_try_getable!(i64, BigInt, "BigInt");
impl_try_getable!(f32, Float, "Float");
impl_try_getable!(f64, Double, "Double");
impl_try_getable!(bool, Bool, "Bool");
impl_try_getable!(String, String, "String");
impl_try_getable!(Vec<u8>, Bytes, "Bytes");

// Special handling for unsigned types (may need conversion)
impl TryGetable for u8 {
    fn try_get(value: Value) -> Result<Self, ValueExtractionError> {
        match value {
            Value::TinyUnsigned(Some(v)) => Ok(v),
            Value::TinyUnsigned(None) => Err(ValueExtractionError::NullValue),
            Value::SmallInt(Some(v)) if v >= 0 && v <= u8::MAX as i16 => {
                Ok(v as u8)
            }
            Value::SmallInt(None) => Err(ValueExtractionError::NullValue),
            _ => Err(ValueExtractionError::TypeMismatch {
                expected: "TinyUnsigned or SmallInt".to_string(),
                actual: format!("{:?}", value),
            }),
        }
    }
}

impl TryGetable for u16 {
    fn try_get(value: Value) -> Result<Self, ValueExtractionError> {
        match value {
            Value::SmallUnsigned(Some(v)) => Ok(v),
            Value::SmallUnsigned(None) => Err(ValueExtractionError::NullValue),
            Value::Int(Some(v)) if v >= 0 && v <= u16::MAX as i32 => {
                Ok(v as u16)
            }
            Value::Int(None) => Err(ValueExtractionError::NullValue),
            _ => Err(ValueExtractionError::TypeMismatch {
                expected: "SmallUnsigned or Int".to_string(),
                actual: format!("{:?}", value),
            }),
        }
    }
}

impl TryGetable for u32 {
    fn try_get(value: Value) -> Result<Self, ValueExtractionError> {
        match value {
            Value::Unsigned(Some(v)) => Ok(v),
            Value::Unsigned(None) => Err(ValueExtractionError::NullValue),
            Value::BigInt(Some(v)) if v >= 0 && v <= u32::MAX as i64 => {
                Ok(v as u32)
            }
            Value::BigInt(None) => Err(ValueExtractionError::NullValue),
            _ => Err(ValueExtractionError::TypeMismatch {
                expected: "Unsigned or BigInt".to_string(),
                actual: format!("{:?}", value),
            }),
        }
    }
}

impl TryGetable for u64 {
    fn try_get(value: Value) -> Result<Self, ValueExtractionError> {
        match value {
            Value::BigUnsigned(Some(v)) => Ok(v),
            Value::BigUnsigned(None) => Err(ValueExtractionError::NullValue),
            _ => Err(ValueExtractionError::TypeMismatch {
                expected: "BigUnsigned".to_string(),
                actual: format!("{:?}", value),
            }),
        }
    }
}

impl TryGetable for serde_json::Value {
    fn try_get(value: Value) -> Result<Self, ValueExtractionError> {
        match value {
            Value::Json(Some(v)) => Ok(*v),
            Value::Json(None) => Err(ValueExtractionError::NullValue),
            _ => Err(ValueExtractionError::TypeMismatch {
                expected: "Json".to_string(),
                actual: format!("{:?}", value),
            }),
        }
    }
}

// Implement TryGetable for Option<T> where T: TryGetable
impl<T: TryGetable> TryGetable for Option<T> {
    fn try_get(value: Value) -> Result<Self, ValueExtractionError> {
        match T::try_get(value.clone()) {
            Ok(v) => Ok(Some(v)),
            Err(ValueExtractionError::NullValue) => Ok(None),
            Err(e) => Err(e),
        }
    }
    
    fn try_get_opt(value: Value) -> Result<Option<Self>, ValueExtractionError> {
        // For Option<Option<T>>, we flatten
        match T::try_get(value) {
            Ok(v) => Ok(Some(Some(v))),
            Err(ValueExtractionError::NullValue) => Ok(Some(None)),
            Err(e) => Err(e),
        }
    }
}

/// Trait for extracting multiple values from collections
///
/// This trait extends `TryGetable` to support extracting multiple values from
/// collections of `Value` enums. Useful for batch operations and composite keys.
///
/// ## Usage
///
/// ```rust
/// use lifeguard::TryGetableMany;
/// use sea_query::Value;
///
/// let values = vec![Value::Int(Some(1)), Value::Int(Some(2)), Value::Int(Some(3))];
/// let result: Result<Vec<i32>, _> = TryGetableMany::try_get_many(values);
/// assert_eq!(result, Ok(vec![1, 2, 3]));
/// ```
pub trait TryGetableMany: TryGetable {
    /// Try to extract multiple values from a collection of `Value` enums.
    ///
    /// Returns:
    /// - `Ok(Vec<T>)` if all values can be extracted successfully
    /// - `Err(ValueExtractionError)` if any value fails to extract
    fn try_get_many<I>(values: I) -> Result<Vec<Self>, ValueExtractionError>
    where
        I: IntoIterator<Item = Value>,
    {
        let mut result = Vec::new();
        for value in values {
            result.push(Self::try_get(value)?);
        }
        Ok(result)
    }
    
    /// Try to extract multiple optional values from a collection.
    ///
    /// Returns `Ok(Vec<Option<T>>)` where `None` entries represent null values.
    fn try_get_many_opt<I>(values: I) -> Result<Vec<Option<Self>>, ValueExtractionError>
    where
        I: IntoIterator<Item = Value>,
    {
        let mut result = Vec::new();
        for value in values {
            result.push(Self::try_get_opt(value)?);
        }
        Ok(result)
    }
}

// Implement TryGetableMany for all types that implement TryGetable
impl<T: TryGetable> TryGetableMany for T {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_try_get_success() {
        let value = Value::Int(Some(42));
        let result: Result<i32, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_get_null() {
        let value = Value::Int(None);
        let result: Result<i32, _> = TryGetable::try_get(value);
        assert!(matches!(result, Err(ValueExtractionError::NullValue)));
    }
    
    #[test]
    fn test_try_get_type_mismatch() {
        let value = Value::String(Some("hello".to_string()));
        let result: Result<i32, _> = TryGetable::try_get(value);
        assert!(matches!(result, Err(ValueExtractionError::TypeMismatch { .. })));
    }
    
    #[test]
    fn test_try_get_opt() {
        let value = Value::Int(Some(42));
        let result: Result<Option<i32>, _> = TryGetable::try_get_opt(value);
        assert_eq!(result, Ok(Some(42)));
        
        let null_value = Value::Int(None);
        let result: Result<Option<i32>, _> = TryGetable::try_get_opt(null_value);
        assert_eq!(result, Ok(None));
    }
    
    #[test]
    fn test_try_get_many() {
        let values = vec![
            Value::Int(Some(1)),
            Value::Int(Some(2)),
            Value::Int(Some(3)),
        ];
        let result: Result<Vec<i32>, _> = TryGetableMany::try_get_many(values);
        assert_eq!(result, Ok(vec![1, 2, 3]));
    }
    
    #[test]
    fn test_try_get_many_with_null() {
        let values = vec![
            Value::Int(Some(1)),
            Value::Int(None),
            Value::Int(Some(3)),
        ];
        let result: Result<Vec<i32>, _> = TryGetableMany::try_get_many(values);
        assert!(matches!(result, Err(ValueExtractionError::NullValue)));
    }
    
    #[test]
    fn test_try_get_many_opt() {
        let values = vec![
            Value::Int(Some(1)),
            Value::Int(None),
            Value::Int(Some(3)),
        ];
        let result: Result<Vec<Option<i32>>, _> = TryGetableMany::try_get_many_opt(values);
        assert_eq!(result, Ok(vec![Some(1), None, Some(3)]));
    }
}
