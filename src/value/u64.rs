//! `TryFromU64` trait for safe conversion from `u64`
//!
//! This trait provides safe conversion from `u64` values to other integer types,
//! with proper overflow handling. This is particularly useful for primary key
//! conversions where database IDs are often stored as `u64` but need to be
//! converted to smaller integer types.

use crate::value::ValueExtractionError;

/// Trait for safe conversion from `u64` to other integer types
///
/// This trait provides a safe way to convert `u64` values to other integer types
/// with proper overflow checking. This is essential for primary key operations where
/// database IDs might be stored as `u64` but need to be converted to `i32`, `i64`, etc.
///
/// ## Usage
///
/// ```rust
/// use lifeguard::TryFromU64;
///
/// let value: u64 = 42;
/// let result: Result<i32, _> = TryFromU64::try_from_u64(value);
/// assert_eq!(result, Ok(42));
///
/// let overflow: u64 = u64::MAX;
/// let result: Result<i32, _> = TryFromU64::try_from_u64(overflow);
/// assert!(result.is_err()); // Overflow error
/// ```
pub trait TryFromU64: Sized {
    /// Try to convert a `u64` value to this type
    ///
    /// # Returns
    ///
    /// - `Ok(Self)` if the conversion succeeds
    /// - `Err(ValueExtractionError::ConversionError)` if the value overflows
    ///
    /// # Errors
    ///
    /// Returns `ValueExtractionError::ConversionError` if the `u64` value overflows
    /// the target integer type's maximum value.
    fn try_from_u64(value: u64) -> Result<Self, ValueExtractionError>;
}

// Implementations for all integer types

impl TryFromU64 for i8 {
    fn try_from_u64(value: u64) -> Result<Self, ValueExtractionError> {
        #[allow(clippy::cast_sign_loss)] // i8::MAX is positive, safe to cast to u64
        if value > i8::MAX as u64 {
            Err(ValueExtractionError::ConversionError(format!(
                "u64 value {} overflows i8::MAX ({})",
                value, i8::MAX
            )))
        } else {
            #[allow(clippy::cast_possible_truncation)] // Checked for overflow above
            Ok(value as i8)
        }
    }
}

impl TryFromU64 for i16 {
    fn try_from_u64(value: u64) -> Result<Self, ValueExtractionError> {
        #[allow(clippy::cast_sign_loss)] // i16::MAX is positive, safe to cast to u64
        if value > i16::MAX as u64 {
            Err(ValueExtractionError::ConversionError(format!(
                "u64 value {} overflows i16::MAX ({})",
                value, i16::MAX
            )))
        } else {
            #[allow(clippy::cast_possible_truncation)] // Checked for overflow above
            Ok(value as i16)
        }
    }
}

impl TryFromU64 for i32 {
    fn try_from_u64(value: u64) -> Result<Self, ValueExtractionError> {
        #[allow(clippy::cast_sign_loss)] // i32::MAX is positive, safe to cast to u64
        if value > i32::MAX as u64 {
            Err(ValueExtractionError::ConversionError(format!(
                "u64 value {} overflows i32::MAX ({})",
                value, i32::MAX
            )))
        } else {
            #[allow(clippy::cast_possible_truncation)] // Checked for overflow above
            Ok(value as i32)
        }
    }
}

impl TryFromU64 for i64 {
    fn try_from_u64(value: u64) -> Result<Self, ValueExtractionError> {
        #[allow(clippy::cast_sign_loss)] // i64::MAX is positive, safe to cast to u64
        if value > i64::MAX as u64 {
            Err(ValueExtractionError::ConversionError(format!(
                "u64 value {} overflows i64::MAX ({})",
                value, i64::MAX
            )))
        } else {
            #[allow(clippy::cast_possible_wrap)] // Checked for overflow above
            Ok(value as i64)
        }
    }
}

impl TryFromU64 for u8 {
    fn try_from_u64(value: u64) -> Result<Self, ValueExtractionError> {
        if value > u64::from(u8::MAX) {
            Err(ValueExtractionError::ConversionError(format!(
                "u64 value {} overflows u8::MAX ({})",
                value, u8::MAX
            )))
        } else {
            #[allow(clippy::cast_possible_truncation)] // Checked for overflow above
            Ok(value as u8)
        }
    }
}

impl TryFromU64 for u16 {
    fn try_from_u64(value: u64) -> Result<Self, ValueExtractionError> {
        if value > u64::from(u16::MAX) {
            Err(ValueExtractionError::ConversionError(format!(
                "u64 value {} overflows u16::MAX ({})",
                value, u16::MAX
            )))
        } else {
            #[allow(clippy::cast_possible_truncation)] // Checked for overflow above
            Ok(value as u16)
        }
    }
}

impl TryFromU64 for u32 {
    fn try_from_u64(value: u64) -> Result<Self, ValueExtractionError> {
        if value > u64::from(u32::MAX) {
            Err(ValueExtractionError::ConversionError(format!(
                "u64 value {} overflows u32::MAX ({})",
                value, u32::MAX
            )))
        } else {
            #[allow(clippy::cast_possible_truncation)] // Checked for overflow above
            Ok(value as u32)
        }
    }
}

impl TryFromU64 for u64 {
    fn try_from_u64(value: u64) -> Result<Self, ValueExtractionError> {
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // i8 tests
    
    #[test]
    fn test_try_from_u64_i8_success() {
        let value: u64 = 42;
        let result: Result<i8, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_from_u64_i8_overflow() {
        let value: u64 = i8::MAX as u64 + 1;
        let result: Result<i8, _> = TryFromU64::try_from_u64(value);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
    }
    
    #[test]
    fn test_try_from_u64_i8_boundary() {
        let value: u64 = i8::MAX as u64;
        let result: Result<i8, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(i8::MAX));
    }
    
    // i16 tests
    
    #[test]
    fn test_try_from_u64_i16_success() {
        let value: u64 = 42;
        let result: Result<i16, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_from_u64_i16_overflow() {
        let value: u64 = i16::MAX as u64 + 1;
        let result: Result<i16, _> = TryFromU64::try_from_u64(value);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
    }
    
    #[test]
    fn test_try_from_u64_i16_boundary() {
        let value: u64 = i16::MAX as u64;
        let result: Result<i16, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(i16::MAX));
    }
    
    // i32 tests
    
    #[test]
    fn test_try_from_u64_i32_success() {
        let value: u64 = 42;
        let result: Result<i32, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_from_u64_i32_overflow() {
        let value: u64 = i32::MAX as u64 + 1;
        let result: Result<i32, _> = TryFromU64::try_from_u64(value);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
    }
    
    #[test]
    fn test_try_from_u64_i32_boundary() {
        let value: u64 = i32::MAX as u64;
        let result: Result<i32, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(i32::MAX));
    }
    
    // i64 tests
    
    #[test]
    fn test_try_from_u64_i64_success() {
        let value: u64 = 42;
        let result: Result<i64, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_from_u64_i64_overflow() {
        let value: u64 = i64::MAX as u64 + 1;
        let result: Result<i64, _> = TryFromU64::try_from_u64(value);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
    }
    
    #[test]
    fn test_try_from_u64_i64_boundary() {
        let value: u64 = i64::MAX as u64;
        let result: Result<i64, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(i64::MAX));
    }
    
    // u8 tests
    
    #[test]
    fn test_try_from_u64_u8_success() {
        let value: u64 = 255;
        let result: Result<u8, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(255));
    }
    
    #[test]
    fn test_try_from_u64_u8_overflow() {
        let value: u64 = 256;
        let result: Result<u8, _> = TryFromU64::try_from_u64(value);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
    }
    
    #[test]
    fn test_try_from_u64_u8_boundary() {
        let value: u64 = u64::from(u8::MAX);
        let result: Result<u8, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(u8::MAX));
    }
    
    // u16 tests
    
    #[test]
    fn test_try_from_u64_u16_success() {
        let value: u64 = 42;
        let result: Result<u16, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_from_u64_u16_overflow() {
        let value: u64 = u64::from(u16::MAX) + 1;
        let result: Result<u16, _> = TryFromU64::try_from_u64(value);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
    }
    
    #[test]
    fn test_try_from_u64_u16_boundary() {
        let value: u64 = u64::from(u16::MAX);
        let result: Result<u16, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(u16::MAX));
    }
    
    // u32 tests
    
    #[test]
    fn test_try_from_u64_u32_success() {
        let value: u64 = 42;
        let result: Result<u32, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(42));
    }
    
    #[test]
    fn test_try_from_u64_u32_overflow() {
        let value: u64 = u64::from(u32::MAX) + 1;
        let result: Result<u32, _> = TryFromU64::try_from_u64(value);
        assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
    }
    
    #[test]
    fn test_try_from_u64_u32_boundary() {
        let value: u64 = u64::from(u32::MAX);
        let result: Result<u32, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(u32::MAX));
    }
    
    // u64 tests
    
    #[test]
    fn test_try_from_u64_u64_identity() {
        let value: u64 = u64::MAX;
        let result: Result<u64, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(u64::MAX));
    }
    
    #[test]
    fn test_try_from_u64_u64_zero() {
        let value: u64 = 0;
        let result: Result<u64, _> = TryFromU64::try_from_u64(value);
        assert_eq!(result, Ok(0));
    }
    
    // Error message tests
    
    #[test]
    #[allow(clippy::panic)] // Test code - panic is acceptable
    fn test_try_from_u64_error_message() {
        let value: u64 = i32::MAX as u64 + 1;
        let result: Result<i32, _> = TryFromU64::try_from_u64(value);
        match result {
            Err(ValueExtractionError::ConversionError(msg)) => {
                assert!(msg.contains("overflows"));
                assert!(msg.contains("i32::MAX"));
            }
            _ => panic!("Expected ConversionError"),
        }
    }
}
