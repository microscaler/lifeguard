//! `TryGetable` and `TryGetableMany` traits for safe value extraction
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
                write!(f, "Type mismatch: expected {expected}, got {actual}")
            }
            ValueExtractionError::ConversionError(msg) => {
                write!(f, "Conversion error: {msg}")
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
    ///
    /// # Errors
    ///
    /// Returns `ValueExtractionError::NullValue` if the value is null.
    /// Returns `ValueExtractionError::TypeMismatch` if the value type doesn't match the expected type.
    /// Returns `ValueExtractionError::ConversionError` if conversion fails (e.g., overflow, negative value for unsigned type).
    fn try_get(value: Value) -> Result<Self, ValueExtractionError>;
    
    /// Try to extract a value, allowing null values to return `None`.
    ///
    /// Returns:
    /// - `Ok(Some(T))` if the value matches and is not null
    /// - `Ok(None)` if the value is null
    /// - `Err(...)` if the value type doesn't match or conversion fails
    ///
    /// # Errors
    ///
    /// Returns `ValueExtractionError::TypeMismatch` if the value type doesn't match the expected type.
    /// Returns `ValueExtractionError::ConversionError` if conversion fails (e.g., overflow, negative value for unsigned type).
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
            Value::TinyUnsigned(None) | Value::SmallInt(None) => Err(ValueExtractionError::NullValue),
            #[allow(clippy::checked_conversions)] // Manual range check is explicit and safe
            Value::SmallInt(Some(v)) if v >= 0 && v <= i16::from(u8::MAX) => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Checked for range above
                Ok(v as u8)
            }
            Value::SmallInt(Some(v)) if v < 0 => {
                Err(ValueExtractionError::ConversionError(format!(
                    "Cannot convert negative value {v} to u8"
                )))
            }
            Value::SmallInt(Some(v)) => {
                Err(ValueExtractionError::ConversionError(format!(
                    "Value {} overflows u8::MAX ({})",
                    v, u8::MAX
                )))
            }
            _ => Err(ValueExtractionError::TypeMismatch {
                expected: "TinyUnsigned or SmallInt".to_string(),
                actual: format!("{value:?}"),
            }),
        }
    }
}

impl TryGetable for u16 {
    fn try_get(value: Value) -> Result<Self, ValueExtractionError> {
        match value {
            Value::SmallUnsigned(Some(v)) => Ok(v),
            Value::SmallUnsigned(None) | Value::Int(None) => Err(ValueExtractionError::NullValue),
            #[allow(clippy::checked_conversions)] // Manual range check is explicit and safe
            Value::Int(Some(v)) if v >= 0 && v <= i32::from(u16::MAX) => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Checked for range above
                Ok(v as u16)
            }
            Value::Int(Some(v)) if v < 0 => {
                Err(ValueExtractionError::ConversionError(format!(
                    "Cannot convert negative value {v} to u16"
                )))
            }
            Value::Int(Some(v)) => {
                Err(ValueExtractionError::ConversionError(format!(
                    "Value {} overflows u16::MAX ({})",
                    v, u16::MAX
                )))
            }
            _ => Err(ValueExtractionError::TypeMismatch {
                expected: "SmallUnsigned or Int".to_string(),
                actual: format!("{value:?}"),
            }),
        }
    }
}

impl TryGetable for u32 {
    fn try_get(value: Value) -> Result<Self, ValueExtractionError> {
        match value {
            Value::Unsigned(Some(v)) => Ok(v),
            Value::Unsigned(None) | Value::BigInt(None) => Err(ValueExtractionError::NullValue),
            #[allow(clippy::checked_conversions)] // Manual range check is explicit and safe
            Value::BigInt(Some(v)) if v >= 0 && v <= i64::from(u32::MAX) => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Checked for range above
                Ok(v as u32)
            }
            Value::BigInt(Some(v)) if v < 0 => {
                Err(ValueExtractionError::ConversionError(format!(
                    "Cannot convert negative value {v} to u32"
                )))
            }
            Value::BigInt(Some(v)) => {
                Err(ValueExtractionError::ConversionError(format!(
                    "Value {} overflows u32::MAX ({})",
                    v, u32::MAX
                )))
            }
            _ => Err(ValueExtractionError::TypeMismatch {
                expected: "Unsigned or BigInt".to_string(),
                actual: format!("{value:?}"),
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
                actual: format!("{value:?}"),
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
                actual: format!("{value:?}"),
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
    ///
    /// # Errors
    ///
    /// Returns `ValueExtractionError` if any value in the collection fails to extract.
    /// The error is the same as returned by `try_get()` for the failing value.
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
    /// # Errors
    ///
    /// Returns `ValueExtractionError` if any non-null value in the collection fails to extract.
    /// Null values are converted to `None` and do not cause errors.
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
#[allow(clippy::approx_constant)]
mod tests {
    use super::*;
    
    // Basic TryGetable tests
    
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
    
    // Integer type tests
    
    #[test]
    fn test_try_get_i8() {
        let value = Value::TinyInt(Some(42));
        let result: Result<i8, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(42));
        
        let null = Value::TinyInt(None);
        let result: Result<i8, _> = TryGetable::try_get(null);
        assert!(matches!(result, Err(ValueExtractionError::NullValue)));
    }
    
    #[test]
    fn test_try_get_i16() {
        let value = Value::SmallInt(Some(42));
        let result: Result<i16, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_get_i64() {
        let value = Value::BigInt(Some(42));
        let result: Result<i64, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(42));
    }
    
    // Unsigned type tests with conversion logic
    
    #[test]
    fn test_try_get_u8_from_tiny_unsigned() {
        let value = Value::TinyUnsigned(Some(42u8));
        let result: Result<u8, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_get_u8_from_small_int() {
        let value = Value::SmallInt(Some(42i16));
        let result: Result<u8, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_get_u8_overflow() {
        let value = Value::SmallInt(Some(256i16)); // > u8::MAX
        let result: Result<u8, _> = TryGetable::try_get(value);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
    }
    
    #[test]
    #[allow(clippy::panic)] // Test code - panic is acceptable
    fn test_try_get_u8_negative() {
        let value = Value::SmallInt(Some(-1i16));
        let result: Result<u8, _> = TryGetable::try_get(value);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
        match result {
            Err(ValueExtractionError::ConversionError(msg)) => {
                assert!(msg.contains("negative"));
            }
            _ => panic!("Expected ConversionError for negative value"),
        }
    }
    
    #[test]
    fn test_try_get_u16_from_small_unsigned() {
        let value = Value::SmallUnsigned(Some(42u16));
        let result: Result<u16, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_get_u16_from_int() {
        let value = Value::Int(Some(42i32));
        let result: Result<u16, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_get_u16_overflow() {
        let value = Value::Int(Some(65536i32)); // > u16::MAX
        let result: Result<u16, _> = TryGetable::try_get(value);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
    }
    
    #[test]
    #[allow(clippy::panic)] // Test code - panic is acceptable
    fn test_try_get_u16_negative() {
        let value = Value::Int(Some(-1i32));
        let result: Result<u16, _> = TryGetable::try_get(value);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
        match result {
            Err(ValueExtractionError::ConversionError(msg)) => {
                assert!(msg.contains("negative"));
            }
            _ => panic!("Expected ConversionError for negative value"),
        }
    }
    
    #[test]
    fn test_try_get_u32_from_unsigned() {
        let value = Value::Unsigned(Some(42u32));
        let result: Result<u32, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_get_u32_from_big_int() {
        let value = Value::BigInt(Some(42i64));
        let result: Result<u32, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_get_u32_overflow() {
        let value = Value::BigInt(Some(4_294_967_296_i64)); // > u32::MAX
        let result: Result<u32, _> = TryGetable::try_get(value);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
    }
    
    #[test]
    #[allow(clippy::panic)] // Test code - panic is acceptable
    fn test_try_get_u32_negative() {
        let value = Value::BigInt(Some(-1i64));
        let result: Result<u32, _> = TryGetable::try_get(value);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
        match result {
            Err(ValueExtractionError::ConversionError(msg)) => {
                assert!(msg.contains("negative"));
            }
            _ => panic!("Expected ConversionError for negative value"),
        }
    }
    
    #[test]
    fn test_try_get_u64() {
        let value = Value::BigUnsigned(Some(42u64));
        let result: Result<u64, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(42));
    }
    
    // Floating point tests
    
    #[test]
    #[allow(clippy::unwrap_used)] // Test code - unwrap is acceptable
    fn test_try_get_f32() {
        let value = Value::Float(Some(3.14f32));
        let result: Result<f32, _> = TryGetable::try_get(value);
        assert!((result.unwrap() - 3.14).abs() < f32::EPSILON);
    }
    
    #[test]
    #[allow(clippy::unwrap_used)] // Test code - unwrap is acceptable
    fn test_try_get_f64() {
        let value = Value::Double(Some(3.14f64));
        let result: Result<f64, _> = TryGetable::try_get(value);
        assert!((result.unwrap() - 3.14).abs() < f64::EPSILON);
    }
    
    // Boolean tests
    
    #[test]
    fn test_try_get_bool() {
        let value = Value::Bool(Some(true));
        let result: Result<bool, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(true));
        
        let value = Value::Bool(Some(false));
        let result: Result<bool, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(false));
    }
    
    // String tests
    
    #[test]
    fn test_try_get_string() {
        let value = Value::String(Some("hello".to_string()));
        let result: Result<String, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok("hello".to_string()));
    }
    
    // Binary tests
    
    #[test]
    fn test_try_get_vec_u8() {
        let value = Value::Bytes(Some(vec![1u8, 2u8, 3u8]));
        let result: Result<Vec<u8>, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(vec![1u8, 2u8, 3u8]));
    }
    
    // JSON tests
    
    #[test]
    fn test_try_get_json() {
        let json = serde_json::json!({"key": "value"});
        let value = Value::Json(Some(Box::new(json.clone())));
        let result: Result<serde_json::Value, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(json));
    }
    
    // Option<T> tests
    
    #[test]
    fn test_try_get_option_i32() {
        let value = Value::Int(Some(42));
        let result: Result<Option<i32>, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(Some(42)));
        
        let null = Value::Int(None);
        let result: Result<Option<i32>, _> = TryGetable::try_get(null);
        assert_eq!(result, Ok(None));
    }
    
    #[test]
    fn test_try_get_option_string() {
        let value = Value::String(Some("hello".to_string()));
        let result: Result<Option<String>, _> = TryGetable::try_get(value);
        assert_eq!(result, Ok(Some("hello".to_string())));
        
        let null = Value::String(None);
        let result: Result<Option<String>, _> = TryGetable::try_get(null);
        assert_eq!(result, Ok(None));
    }
    
    // TryGetableMany tests
    
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
    fn test_try_get_many_with_type_mismatch() {
        let values = vec![
            Value::Int(Some(1)),
            Value::String(Some("hello".to_string())),
            Value::Int(Some(3)),
        ];
        let result: Result<Vec<i32>, _> = TryGetableMany::try_get_many(values);
        assert!(matches!(result, Err(ValueExtractionError::TypeMismatch { .. })));
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
    
    #[test]
    fn test_try_get_many_empty() {
        let values: Vec<Value> = vec![];
        let result: Result<Vec<i32>, _> = TryGetableMany::try_get_many(values);
        assert_eq!(result, Ok(vec![]));
    }
    
    #[test]
    fn test_try_get_many_mixed_types() {
        let values = vec![
            Value::String(Some("a".to_string())),
            Value::String(Some("b".to_string())),
            Value::String(Some("c".to_string())),
        ];
        let result: Result<Vec<String>, _> = TryGetableMany::try_get_many(values);
        assert_eq!(result, Ok(vec!["a".to_string(), "b".to_string(), "c".to_string()]));
    }
    
    // Error message tests
    
    #[test]
    #[allow(clippy::panic)] // Test code - panic is acceptable
    fn test_error_message_null_value() {
        let value = Value::Int(None);
        let result: Result<i32, _> = TryGetable::try_get(value);
        match result {
            Err(ValueExtractionError::NullValue) => {
                // Error message should be clear
                let msg = format!("{}", ValueExtractionError::NullValue);
                assert!(msg.contains("null") || msg.contains("Null"));
            }
            _ => panic!("Expected NullValue error"),
        }
    }
    
    #[test]
    #[allow(clippy::panic)] // Test code - panic is acceptable
    fn test_error_message_type_mismatch() {
        let value = Value::String(Some("hello".to_string()));
        let result: Result<i32, _> = TryGetable::try_get(value);
        match result {
            Err(ValueExtractionError::TypeMismatch { expected, actual }) => {
                assert!(expected.contains("Int"));
                assert!(actual.contains("String"));
            }
            _ => panic!("Expected TypeMismatch error"),
        }
    }
}
