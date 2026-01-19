//! ValueType trait for type-safe value conversions
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
            Value::TinyInt(None) => None,
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
            Value::SmallInt(None) => None,
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
            Value::Int(None) => None,
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
            Value::BigInt(None) => None,
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::BigInt(None)
    }
}

impl ValueType for u8 {
    fn into_value(self) -> Value {
        Value::SmallInt(Some(self as i16))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::TinyUnsigned(Some(v)) => Some(v),
            Value::TinyUnsigned(None) => None,
            Value::SmallInt(Some(v)) if v >= 0 && v <= u8::MAX as i16 => Some(v as u8),
            Value::SmallInt(None) => None,
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::TinyUnsigned(None)
    }
}

impl ValueType for u16 {
    fn into_value(self) -> Value {
        Value::Int(Some(self as i32))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::SmallUnsigned(Some(v)) => Some(v),
            Value::SmallUnsigned(None) => None,
            Value::Int(Some(v)) if v >= 0 && v <= u16::MAX as i32 => Some(v as u16),
            Value::Int(None) => None,
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::SmallUnsigned(None)
    }
}

impl ValueType for u32 {
    fn into_value(self) -> Value {
        Value::BigInt(Some(self as i64))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Unsigned(Some(v)) => Some(v),
            Value::Unsigned(None) => None,
            Value::BigInt(Some(v)) if v >= 0 && v <= u32::MAX as i64 => Some(v as u32),
            Value::BigInt(None) => None,
            _ => None,
        }
    }
    
    fn null_value() -> Value {
        Value::Unsigned(None)
    }
}

impl ValueType for u64 {
    fn into_value(self) -> Value {
        Value::BigUnsigned(Some(self))
    }
    
    fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::BigUnsigned(Some(v)) => Some(v),
            Value::BigUnsigned(None) => None,
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
            Value::Float(None) => None,
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
            Value::Double(None) => None,
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
            Value::Bool(None) => None,
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
            Value::String(None) => None,
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
            Value::Bytes(None) => None,
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
            Value::Json(None) => None,
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
mod tests {
    use super::*;
    
    #[test]
    fn test_i32_value_type() {
        let value = 42i32.into_value();
        assert!(matches!(value, Value::Int(Some(42))));
        
        let extracted = ValueType::from_value(value);
        assert_eq!(extracted, Some(42i32));
    }
    
    #[test]
    fn test_string_value_type() {
        let value = "hello".to_string().into_value();
        assert!(matches!(value, Value::String(Some(ref s)) if s == "hello"));
        
        let extracted = ValueType::from_value(value);
        assert_eq!(extracted, Some("hello".to_string()));
    }
    
    #[test]
    fn test_option_value_type() {
        let value = Some(42i32).into_value();
        assert!(matches!(value, Value::Int(Some(42))));
        
        let extracted = <Option<i32> as ValueType>::from_value(value);
        assert_eq!(extracted, Some(Some(42i32)));
        
        // Test None case
        let none_value = Value::Int(None);
        let extracted = <Option<i32> as ValueType>::from_value(none_value);
        assert_eq!(extracted, Some(None));
    }
    
    #[test]
    fn test_bool_value_type() {
        let value = true.into_value();
        assert!(matches!(value, Value::Bool(Some(true))));
        
        let extracted = ValueType::from_value(value);
        assert_eq!(extracted, Some(true));
    }
    
    #[test]
    fn test_f64_value_type() {
        let value = 3.14f64.into_value();
        assert!(matches!(value, Value::Double(Some(v)) if (v - 3.14).abs() < f64::EPSILON));
        
        let extracted = <f64 as ValueType>::from_value(value);
        assert!(extracted.map(|v: f64| (v - 3.14).abs() < f64::EPSILON).unwrap_or(false));
    }
}
