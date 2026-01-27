//! `ValueType` trait for type-safe value conversions
//!
//! The `ValueType` trait maps Rust types to their corresponding `sea_query::Value` variant.
//! This enables type-safe conversions and better compile-time guarantees.
//!
//! ## Usage
//!
//! ```rust
//! use lifeguard::ValueType;
//! use sea_query::Value;
//!
//! // ValueType provides an associated type that maps to the Value variant
//! let value: Value = ValueType::value(42i32);
//! // value is Value::Int(Some(42))
//! ```
//!
//! ## Implementation
//!
//! All standard Rust types have `ValueType` implementations that map to the appropriate
//! `sea_query::Value` variant. The trait is automatically implemented for:
//!
//! - Integer types: `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`
//! - Floating point: `f32`, `f64`
//! - Boolean: `bool`
//! - String: `String`
//! - Binary: `Vec<u8>`
//! - JSON: `serde_json::Value`
//! - Option<T> for all above types

use sea_query::Value;

/// Trait for mapping Rust types to their corresponding `sea_query::Value` variant.
///
/// This trait provides type-safe conversions between Rust types and `Value` enums.
/// Each type implements `ValueType` to specify which `Value` variant it corresponds to.
///
/// ## Associated Type
///
/// The `ValueType` trait uses an associated type pattern, but since `Value` is an enum,
/// we use a marker trait approach where implementations provide conversion methods.
///
/// ## Example
///
/// ```rust
/// use lifeguard::ValueType;
/// use sea_query::Value;
///
/// // Convert i32 to Value
/// let value = ValueType::into_value(42i32);
/// assert!(matches!(value, Value::Int(Some(42))));
///
/// // Convert Option<i32> to Value
/// let value = ValueType::into_value(Some(42i32));
/// assert!(matches!(value, Value::Int(Some(42))));
///
/// let value = ValueType::into_value(None::<i32>);
/// assert!(matches!(value, Value::Int(None)));
/// ```
pub trait ValueType: Sized {
    /// Convert this value into a `sea_query::Value`.
    ///
    /// This method converts the implementing type into its corresponding `Value` variant.
    fn into_value(self) -> Value;
    
    /// Convert a `sea_query::Value` into this type, if possible.
    ///
    /// Returns `None` if the value doesn't match the expected variant or is null.
    fn from_value(value: Value) -> Option<Self>;
    
    /// Return the null variant for this type.
    ///
    /// This is used by `Option<T>` to create the appropriate null `Value` variant
    /// when converting `None`.
    fn null_value() -> Value;
}

// Implementations for primitive types

impl ValueType for i8 {
    fn into_value(self) -> Value {
        Value::TinyInt(Some(self))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::TinyInt(Some(v)) => Some(v),
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::TinyInt(None)
    }
}

impl ValueType for i16 {
    fn into_value(self) -> Value {
        Value::SmallInt(Some(self))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::SmallInt(Some(v)) => Some(v),
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::SmallInt(None)
    }
}

impl ValueType for i32 {
    fn into_value(self) -> Value {
        Value::Int(Some(self))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Int(Some(v)) => Some(v),
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::Int(None)
    }
}

impl ValueType for i64 {
    fn into_value(self) -> Value {
        Value::BigInt(Some(self))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::BigInt(Some(v)) => Some(v),
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::BigInt(None)
    }
}

impl ValueType for u8 {
    fn into_value(self) -> Value {
        Value::SmallInt(Some(i16::from(self)))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::TinyUnsigned(Some(v)) => Some(v),
            #[allow(clippy::checked_conversions)] // Manual range check is explicit and safe
            Value::SmallInt(Some(v)) if v >= 0 && v <= i16::from(u8::MAX) => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Checked for range above
                Some(v as u8)
            }
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::SmallInt(None)
    }
}

impl ValueType for u16 {
    fn into_value(self) -> Value {
        Value::Int(Some(i32::from(self)))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::SmallUnsigned(Some(v)) => Some(v),
            #[allow(clippy::checked_conversions)] // Manual range check is explicit and safe
            Value::Int(Some(v)) if v >= 0 && v <= i32::from(u16::MAX) => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Checked for range above
                Some(v as u16)
            }
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::Int(None)
    }
}

impl ValueType for u32 {
    fn into_value(self) -> Value {
        Value::BigInt(Some(i64::from(self)))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Unsigned(Some(v)) => Some(v),
            #[allow(clippy::checked_conversions)] // Manual range check is explicit and safe
            Value::BigInt(Some(v)) if v >= 0 && v <= i64::from(u32::MAX) => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Checked for range above
                Some(v as u32)
            }
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::BigInt(None)
    }
}

impl ValueType for u64 {
    fn into_value(self) -> Value {
        Value::BigUnsigned(Some(self))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::BigUnsigned(Some(v)) => Some(v),
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::BigUnsigned(None)
    }
}

impl ValueType for f32 {
    fn into_value(self) -> Value {
        Value::Float(Some(self))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Float(Some(v)) => Some(v),
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::Float(None)
    }
}

impl ValueType for f64 {
    fn into_value(self) -> Value {
        Value::Double(Some(self))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Double(Some(v)) => Some(v),
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::Double(None)
    }
}

impl ValueType for bool {
    fn into_value(self) -> Value {
        Value::Bool(Some(self))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Bool(Some(v)) => Some(v),
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::Bool(None)
    }
}

impl ValueType for String {
    fn into_value(self) -> Value {
        Value::String(Some(self))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::String(Some(v)) => Some(v),
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::String(None)
    }
}

impl ValueType for Vec<u8> {
    fn into_value(self) -> Value {
        Value::Bytes(Some(self))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Bytes(Some(v)) => Some(v),
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::Bytes(None)
    }
}

impl ValueType for serde_json::Value {
    fn into_value(self) -> Value {
        Value::Json(Some(Box::new(self)))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Json(Some(v)) => Some(*v),
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::Json(None)
    }
}

// Implementations for Option<T> where T: ValueType
// 
// Note: For None values, we need a way to create the appropriate null variant.
// Since we can't know the type from None alone, we provide a helper method
// that requires the type to be specified, or we use a pattern where None
// is handled by checking the Value variant directly in from_value.

impl<T: ValueType> ValueType for Option<T> {
    fn into_value(self) -> Value {
        match self {
            Some(v) => T::into_value(v),
            None => T::null_value(),
        }
    }
    
    fn from_value(value: Value) -> Option<Self> {
        // Try to extract the value using T's from_value
        match T::from_value(value.clone()) {
            Some(v) => Some(Some(v)),
            None => {
                // If T::from_value returned None, check if the value is the null variant for T
                // We can do this by comparing with T::null_value()
                if value == T::null_value() {
                    Some(None)
                } else {
                    // Value doesn't match T's type at all
                    None
                }
            }
        }
    }
    
    fn null_value() -> Value {
        T::null_value()
    }
}

#[cfg(test)]
#[allow(clippy::approx_constant)]
mod tests {
    use super::*;
    
    // Integer type tests
    
    #[test]
    fn test_i8_value_type() {
        let value = 42i8.into_value();
        assert!(matches!(value, Value::TinyInt(Some(42))));
        let extracted = <i8 as ValueType>::from_value(value);
        assert_eq!(extracted, Some(42i8));
        
        // Test null
        let null = <i8 as ValueType>::null_value();
        assert!(matches!(null, Value::TinyInt(None)));
        let extracted = <i8 as ValueType>::from_value(null);
        assert_eq!(extracted, None);
    }
    
    #[test]
    fn test_i16_value_type() {
        let value = 42i16.into_value();
        assert!(matches!(value, Value::SmallInt(Some(42))));
        let extracted = <i16 as ValueType>::from_value(value);
        assert_eq!(extracted, Some(42i16));
        
        // Test null
        let null = <i16 as ValueType>::null_value();
        assert!(matches!(null, Value::SmallInt(None)));
    }
    
    #[test]
    fn test_i32_value_type() {
        let value = 42i32.into_value();
        assert!(matches!(value, Value::Int(Some(42))));
        let extracted = <i32 as ValueType>::from_value(value);
        assert_eq!(extracted, Some(42i32));
        
        // Test null
        let null = <i32 as ValueType>::null_value();
        assert!(matches!(null, Value::Int(None)));
    }
    
    #[test]
    fn test_i64_value_type() {
        let value = 42i64.into_value();
        assert!(matches!(value, Value::BigInt(Some(42))));
        let extracted = <i64 as ValueType>::from_value(value);
        assert_eq!(extracted, Some(42i64));
        
        // Test null
        let null = <i64 as ValueType>::null_value();
        assert!(matches!(null, Value::BigInt(None)));
    }
    
    #[test]
    fn test_u8_value_type() {
        let value = 42u8.into_value();
        assert!(matches!(value, Value::SmallInt(Some(42))));
        let extracted = <u8 as ValueType>::from_value(value);
        assert_eq!(extracted, Some(42u8));
        
        // Test null - should match what into_value() produces (SmallInt)
        let null = <u8 as ValueType>::null_value();
        assert!(matches!(null, Value::SmallInt(None)));
        
        // Test TinyUnsigned variant (still supported for backward compatibility)
        let tiny_unsigned = Value::TinyUnsigned(Some(42u8));
        let extracted = <u8 as ValueType>::from_value(tiny_unsigned);
        assert_eq!(extracted, Some(42u8));
        
        // Test TinyUnsigned(None) is still recognized as null
        let tiny_unsigned_none = Value::TinyUnsigned(None);
        let extracted = <u8 as ValueType>::from_value(tiny_unsigned_none);
        assert_eq!(extracted, None);
    }
    
    #[test]
    fn test_u16_value_type() {
        let value = 42u16.into_value();
        assert!(matches!(value, Value::Int(Some(42))));
        let extracted = <u16 as ValueType>::from_value(value);
        assert_eq!(extracted, Some(42u16));
        
        // Test null - should match what into_value() produces (Int)
        let null = <u16 as ValueType>::null_value();
        assert!(matches!(null, Value::Int(None)));
        
        // Test SmallUnsigned variant (still supported for backward compatibility)
        let small_unsigned = Value::SmallUnsigned(Some(42u16));
        let extracted = <u16 as ValueType>::from_value(small_unsigned);
        assert_eq!(extracted, Some(42u16));
        
        // Test SmallUnsigned(None) is still recognized as null
        let small_unsigned_none = Value::SmallUnsigned(None);
        let extracted = <u16 as ValueType>::from_value(small_unsigned_none);
        assert_eq!(extracted, None);
    }
    
    #[test]
    fn test_u32_value_type() {
        let value = 42u32.into_value();
        assert!(matches!(value, Value::BigInt(Some(42))));
        let extracted = <u32 as ValueType>::from_value(value);
        assert_eq!(extracted, Some(42u32));
        
        // Test null - should match what into_value() produces (BigInt)
        let null = <u32 as ValueType>::null_value();
        assert!(matches!(null, Value::BigInt(None)));
        
        // Test Unsigned variant (still supported for backward compatibility)
        let unsigned = Value::Unsigned(Some(42u32));
        let extracted = <u32 as ValueType>::from_value(unsigned);
        assert_eq!(extracted, Some(42u32));
        
        // Test Unsigned(None) is still recognized as null
        let unsigned_none = Value::Unsigned(None);
        let extracted = <u32 as ValueType>::from_value(unsigned_none);
        assert_eq!(extracted, None);
    }
    
    #[test]
    fn test_u64_value_type() {
        let value = 42u64.into_value();
        assert!(matches!(value, Value::BigUnsigned(Some(42))));
        let extracted = <u64 as ValueType>::from_value(value);
        assert_eq!(extracted, Some(42u64));
        
        // Test null
        let null = <u64 as ValueType>::null_value();
        assert!(matches!(null, Value::BigUnsigned(None)));
    }
    
    // Floating point tests
    
    #[test]
    #[allow(clippy::map_unwrap_or)] // Test code - map().unwrap_or() pattern is acceptable
    fn test_f32_value_type() {
        let value = 3.14f32.into_value();
        assert!(matches!(value, Value::Float(Some(v)) if (v - 3.14).abs() < f32::EPSILON));
        let extracted = <f32 as ValueType>::from_value(value);
        assert!(extracted.map(|v: f32| (v - 3.14).abs() < f32::EPSILON).unwrap_or(false));
        
        // Test null
        let null = <f32 as ValueType>::null_value();
        assert!(matches!(null, Value::Float(None)));
    }
    
    #[test]
    #[allow(clippy::map_unwrap_or)] // Test code - map().unwrap_or() pattern is acceptable
    fn test_f64_value_type() {
        let value = 3.14f64.into_value();
        assert!(matches!(value, Value::Double(Some(v)) if (v - 3.14).abs() < f64::EPSILON));
        let extracted = <f64 as ValueType>::from_value(value);
        assert!(extracted.map(|v: f64| (v - 3.14).abs() < f64::EPSILON).unwrap_or(false));
        
        // Test null
        let null = <f64 as ValueType>::null_value();
        assert!(matches!(null, Value::Double(None)));
    }
    
    // Boolean tests
    
    #[test]
    fn test_bool_value_type() {
        let value = true.into_value();
        assert!(matches!(value, Value::Bool(Some(true))));
        let extracted = <bool as ValueType>::from_value(value);
        assert_eq!(extracted, Some(true));
        
        let value = false.into_value();
        assert!(matches!(value, Value::Bool(Some(false))));
        let extracted = <bool as ValueType>::from_value(value);
        assert_eq!(extracted, Some(false));
        
        // Test null
        let null = <bool as ValueType>::null_value();
        assert!(matches!(null, Value::Bool(None)));
    }
    
    // String tests
    
    #[test]
    fn test_string_value_type() {
        let value = "hello".to_string().into_value();
        assert!(matches!(value, Value::String(Some(ref s)) if s == "hello"));
        let extracted = <String as ValueType>::from_value(value);
        assert_eq!(extracted, Some("hello".to_string()));
        
        // Test empty string
        let empty = String::new().into_value();
        let extracted = <String as ValueType>::from_value(empty);
        assert_eq!(extracted, Some(String::new()));
        
        // Test null
        let null = <String as ValueType>::null_value();
        assert!(matches!(null, Value::String(None)));
    }
    
    // Binary tests
    
    #[test]
    fn test_vec_u8_value_type() {
        let value = vec![1u8, 2u8, 3u8].into_value();
        assert!(matches!(value, Value::Bytes(Some(ref v)) if v == &vec![1u8, 2u8, 3u8]));
        let extracted = <Vec<u8> as ValueType>::from_value(value);
        assert_eq!(extracted, Some(vec![1u8, 2u8, 3u8]));
        
        // Test empty vec
        let empty = Vec::<u8>::new().into_value();
        let extracted = <Vec<u8> as ValueType>::from_value(empty);
        assert_eq!(extracted, Some(Vec::<u8>::new()));
        
        // Test null
        let null = <Vec<u8> as ValueType>::null_value();
        assert!(matches!(null, Value::Bytes(None)));
    }
    
    // JSON tests
    
    #[test]
    fn test_json_value_type() {
        let json = serde_json::json!({"key": "value"});
        let value = json.clone().into_value();
        assert!(matches!(value, Value::Json(Some(_))));
        let extracted = <serde_json::Value as ValueType>::from_value(value);
        assert_eq!(extracted, Some(json));
        
        // Test null
        let null = <serde_json::Value as ValueType>::null_value();
        assert!(matches!(null, Value::Json(None)));
    }
    
    // Option<T> tests for all types
    
    #[test]
    fn test_option_i32_value_type() {
        let value = Some(42i32).into_value();
        assert!(matches!(value, Value::Int(Some(42))));
        let extracted = <Option<i32> as ValueType>::from_value(value);
        assert_eq!(extracted, Some(Some(42i32)));
        
        // Test None case
        let none_value = Value::Int(None);
        let extracted = <Option<i32> as ValueType>::from_value(none_value);
        assert_eq!(extracted, Some(None));
        
        // Test type mismatch
        let wrong_type = Value::String(Some("hello".to_string()));
        let extracted = <Option<i32> as ValueType>::from_value(wrong_type);
        assert_eq!(extracted, None);
    }
    
    #[test]
    fn test_option_string_value_type() {
        let value = Some("hello".to_string()).into_value();
        assert!(matches!(value, Value::String(Some(ref s)) if s == "hello"));
        let extracted = <Option<String> as ValueType>::from_value(value);
        assert_eq!(extracted, Some(Some("hello".to_string())));
        
        // Test None case
        let none_value = Value::String(None);
        let extracted = <Option<String> as ValueType>::from_value(none_value);
        assert_eq!(extracted, Some(None));
    }
    
    #[test]
    fn test_option_bool_value_type() {
        let value = Some(true).into_value();
        assert!(matches!(value, Value::Bool(Some(true))));
        let extracted = <Option<bool> as ValueType>::from_value(value);
        assert_eq!(extracted, Some(Some(true)));
        
        // Test None case
        let none_value = Value::Bool(None);
        let extracted = <Option<bool> as ValueType>::from_value(none_value);
        assert_eq!(extracted, Some(None));
    }
    
    #[test]
    fn test_option_vec_u8_value_type() {
        let value = Some(vec![1u8, 2u8, 3u8]).into_value();
        assert!(matches!(value, Value::Bytes(Some(_))));
        let extracted = <Option<Vec<u8>> as ValueType>::from_value(value);
        assert_eq!(extracted, Some(Some(vec![1u8, 2u8, 3u8])));
        
        // Test None case
        let none_value = Value::Bytes(None);
        let extracted = <Option<Vec<u8>> as ValueType>::from_value(none_value);
        assert_eq!(extracted, Some(None));
    }
    
    // Type mismatch tests
    
    #[test]
    fn test_type_mismatch_i32_from_string() {
        let wrong_type = Value::String(Some("hello".to_string()));
        let extracted = <i32 as ValueType>::from_value(wrong_type);
        assert_eq!(extracted, None);
    }
    
    #[test]
    fn test_type_mismatch_string_from_int() {
        let wrong_type = Value::Int(Some(42));
        let extracted = <String as ValueType>::from_value(wrong_type);
        assert_eq!(extracted, None);
    }
    
    #[test]
    fn test_type_mismatch_bool_from_int() {
        let wrong_type = Value::Int(Some(42));
        let extracted = <bool as ValueType>::from_value(wrong_type);
        assert_eq!(extracted, None);
    }
    
    // Boundary value tests
    
    #[test]
    fn test_i8_boundary_values() {
        let min = i8::MIN.into_value();
        let extracted = <i8 as ValueType>::from_value(min);
        assert_eq!(extracted, Some(i8::MIN));
        
        let max = i8::MAX.into_value();
        let extracted = <i8 as ValueType>::from_value(max);
        assert_eq!(extracted, Some(i8::MAX));
    }
    
    #[test]
    fn test_i64_boundary_values() {
        let min = i64::MIN.into_value();
        let extracted = <i64 as ValueType>::from_value(min);
        assert_eq!(extracted, Some(i64::MIN));
        
        let max = i64::MAX.into_value();
        let extracted = <i64 as ValueType>::from_value(max);
        assert_eq!(extracted, Some(i64::MAX));
    }
    
    #[test]
    fn test_u64_boundary_values() {
        let max = u64::MAX.into_value();
        let extracted = <u64 as ValueType>::from_value(max);
        assert_eq!(extracted, Some(u64::MAX));
    }
    
    #[test]
    #[allow(clippy::map_unwrap_or)] // Test code - map().unwrap_or() pattern is acceptable
    fn test_f32_special_values() {
        // Test NaN
        let nan = f32::NAN.into_value();
        let extracted = <f32 as ValueType>::from_value(nan);
        assert!(extracted.map(|v: f32| v.is_nan()).unwrap_or(false));
        
        // Test infinity
        let inf = f32::INFINITY.into_value();
        let extracted = <f32 as ValueType>::from_value(inf);
        assert!(extracted.map(|v: f32| v.is_infinite()).unwrap_or(false));
    }
    
    #[test]
    #[allow(clippy::map_unwrap_or)] // Test code - map().unwrap_or() pattern is acceptable
    fn test_f64_special_values() {
        // Test NaN
        let nan = f64::NAN.into_value();
        let extracted = <f64 as ValueType>::from_value(nan);
        assert!(extracted.map(|v: f64| v.is_nan()).unwrap_or(false));
        
        // Test infinity
        let inf = f64::INFINITY.into_value();
        let extracted = <f64 as ValueType>::from_value(inf);
        assert!(extracted.map(|v: f64| v.is_infinite()).unwrap_or(false));
    }
    
    // Tests for Option<u8>, Option<u16>, Option<u32> null handling
    // These verify the fix for inconsistent Value variants between into_value() and null_value()
    
    #[test]
    fn test_option_u8_null_handling() {
        // Test that Option<u8>::from_value() correctly handles Value::SmallInt(None)
        // which is what into_value() produces for None (via null_value())
        let none_value = <Option<u8> as ValueType>::into_value(None);
        let extracted = <Option<u8> as ValueType>::from_value(none_value);
        assert_eq!(extracted, Some(None), "Option<u8>::from_value() should handle null correctly");
        
        // Test that Value::SmallInt(None) is recognized as null
        let small_int_none = Value::SmallInt(None);
        let extracted = <Option<u8> as ValueType>::from_value(small_int_none);
        assert_eq!(extracted, Some(None), "Value::SmallInt(None) should be recognized as null for Option<u8>");
    }
    
    #[test]
    fn test_option_u16_null_handling() {
        // Test that Option<u16>::from_value() correctly handles Value::Int(None)
        let none_value = <Option<u16> as ValueType>::into_value(None);
        let extracted = <Option<u16> as ValueType>::from_value(none_value);
        assert_eq!(extracted, Some(None), "Option<u16>::from_value() should handle null correctly");
        
        // Test that Value::Int(None) is recognized as null
        let int_none = Value::Int(None);
        let extracted = <Option<u16> as ValueType>::from_value(int_none);
        assert_eq!(extracted, Some(None), "Value::Int(None) should be recognized as null for Option<u16>");
    }
    
    #[test]
    fn test_option_u32_null_handling() {
        // Test that Option<u32>::from_value() correctly handles Value::BigInt(None)
        let none_value = <Option<u32> as ValueType>::into_value(None);
        let extracted = <Option<u32> as ValueType>::from_value(none_value);
        assert_eq!(extracted, Some(None), "Option<u32>::from_value() should handle null correctly");
        
        // Test that Value::BigInt(None) is recognized as null
        let big_int_none = Value::BigInt(None);
        let extracted = <Option<u32> as ValueType>::from_value(big_int_none);
        assert_eq!(extracted, Some(None), "Value::BigInt(None) should be recognized as null for Option<u32>");
    }
}
