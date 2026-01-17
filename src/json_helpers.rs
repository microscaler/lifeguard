//! JSON serialization/deserialization helpers for Lifeguard
//!
//! This module provides custom deserializers for floating-point types
//! that can handle both numeric JSON values and special string representations
//! (NaN, Infinity, -Infinity) to support roundtrip serialization.

use serde::Deserializer;

/// Custom deserializer for f32 that accepts both numbers and special string representations
///
/// This allows deserializing JSON values that were serialized with our custom
/// serializer, which converts NaN/infinity to strings.
///
/// # Examples
///
/// ```rust,ignore
/// use serde_json::json;
/// use lifeguard::json_helpers::deserialize_f32;
///
/// // Normal number
/// let val: f32 = deserialize_f32(&json!(3.14)).unwrap();
/// assert_eq!(val, 3.14);
///
/// // NaN as string
/// let val: f32 = deserialize_f32(&json!("NaN")).unwrap();
/// assert!(val.is_nan());
///
/// // Infinity as string
/// let val: f32 = deserialize_f32(&json!("Infinity")).unwrap();
/// assert!(val.is_infinite() && val.is_sign_positive());
/// ```
pub fn deserialize_f32<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct F32Visitor;

    impl<'de> Visitor<'de> for F32Visitor {
        type Value = f32;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a number or a string representing a special floating-point value (NaN, Infinity, -Infinity)")
        }

        fn visit_f64<E>(self, value: f64) -> Result<f32, E>
        where
            E: de::Error,
        {
            Ok(value as f32)
        }

        fn visit_i64<E>(self, value: i64) -> Result<f32, E>
        where
            E: de::Error,
        {
            Ok(value as f32)
        }

        fn visit_u64<E>(self, value: u64) -> Result<f32, E>
        where
            E: de::Error,
        {
            Ok(value as f32)
        }

        fn visit_str<E>(self, value: &str) -> Result<f32, E>
        where
            E: de::Error,
        {
            match value {
                "NaN" => Ok(f32::NAN),
                "Infinity" => Ok(f32::INFINITY),
                "-Infinity" => Ok(f32::NEG_INFINITY),
                _ => Err(de::Error::invalid_value(
                    de::Unexpected::Str(value),
                    &"NaN, Infinity, or -Infinity",
                )),
            }
        }
    }

    deserializer.deserialize_any(F32Visitor)
}

/// Custom deserializer for f64 that accepts both numbers and special string representations
///
/// This allows deserializing JSON values that were serialized with our custom
/// serializer, which converts NaN/infinity to strings.
///
/// # Examples
///
/// ```rust,ignore
/// use serde_json::json;
/// use lifeguard::json_helpers::deserialize_f64;
///
/// // Normal number
/// let val: f64 = deserialize_f64(&json!(3.14)).unwrap();
/// assert_eq!(val, 3.14);
///
/// // NaN as string
/// let val: f64 = deserialize_f64(&json!("NaN")).unwrap();
/// assert!(val.is_nan());
///
/// // Infinity as string
/// let val: f64 = deserialize_f64(&json!("Infinity")).unwrap();
/// assert!(val.is_infinite() && val.is_sign_positive());
/// ```
pub fn deserialize_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct F64Visitor;

    impl<'de> Visitor<'de> for F64Visitor {
        type Value = f64;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a number or a string representing a special floating-point value (NaN, Infinity, -Infinity)")
        }

        fn visit_f64<E>(self, value: f64) -> Result<f64, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<f64, E>
        where
            E: de::Error,
        {
            Ok(value as f64)
        }

        fn visit_u64<E>(self, value: u64) -> Result<f64, E>
        where
            E: de::Error,
        {
            Ok(value as f64)
        }

        fn visit_str<E>(self, value: &str) -> Result<f64, E>
        where
            E: de::Error,
        {
            match value {
                "NaN" => Ok(f64::NAN),
                "Infinity" => Ok(f64::INFINITY),
                "-Infinity" => Ok(f64::NEG_INFINITY),
                _ => Err(de::Error::invalid_value(
                    de::Unexpected::Str(value),
                    &"NaN, Infinity, or -Infinity",
                )),
            }
        }
    }

    deserializer.deserialize_any(F64Visitor)
}

/// Custom deserializer for Option<f32> that accepts numbers, strings, and null
pub fn deserialize_option_f32<'de, D>(deserializer: D) -> Result<Option<f32>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct OptionF32Visitor;

    impl<'de> Visitor<'de> for OptionF32Visitor {
        type Value = Option<f32>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a number, a string representing a special floating-point value, or null")
        }

        fn visit_none<E>(self) -> Result<Option<f32>, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Option<f32>, D::Error>
        where
            D: Deserializer<'de>,
        {
            Ok(Some(deserialize_f32(deserializer)?))
        }

        fn visit_unit<E>(self) -> Result<Option<f32>, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_f64<E>(self, value: f64) -> Result<Option<f32>, E>
        where
            E: de::Error,
        {
            Ok(Some(value as f32))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Option<f32>, E>
        where
            E: de::Error,
        {
            Ok(Some(value as f32))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Option<f32>, E>
        where
            E: de::Error,
        {
            Ok(Some(value as f32))
        }

        fn visit_str<E>(self, value: &str) -> Result<Option<f32>, E>
        where
            E: de::Error,
        {
            match value {
                "NaN" => Ok(Some(f32::NAN)),
                "Infinity" => Ok(Some(f32::INFINITY)),
                "-Infinity" => Ok(Some(f32::NEG_INFINITY)),
                _ => Err(de::Error::invalid_value(
                    de::Unexpected::Str(value),
                    &"NaN, Infinity, or -Infinity",
                )),
            }
        }
    }

    deserializer.deserialize_option(OptionF32Visitor)
}

/// Custom deserializer for Option<f64> that accepts numbers, strings, and null
pub fn deserialize_option_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct OptionF64Visitor;

    impl<'de> Visitor<'de> for OptionF64Visitor {
        type Value = Option<f64>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a number, a string representing a special floating-point value, or null")
        }

        fn visit_none<E>(self) -> Result<Option<f64>, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Option<f64>, D::Error>
        where
            D: Deserializer<'de>,
        {
            Ok(Some(deserialize_f64(deserializer)?))
        }

        fn visit_unit<E>(self) -> Result<Option<f64>, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_f64<E>(self, value: f64) -> Result<Option<f64>, E>
        where
            E: de::Error,
        {
            Ok(Some(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Option<f64>, E>
        where
            E: de::Error,
        {
            Ok(Some(value as f64))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Option<f64>, E>
        where
            E: de::Error,
        {
            Ok(Some(value as f64))
        }

        fn visit_str<E>(self, value: &str) -> Result<Option<f64>, E>
        where
            E: de::Error,
        {
            match value {
                "NaN" => Ok(Some(f64::NAN)),
                "Infinity" => Ok(Some(f64::INFINITY)),
                "-Infinity" => Ok(Some(f64::NEG_INFINITY)),
                _ => Err(de::Error::invalid_value(
                    de::Unexpected::Str(value),
                    &"NaN, Infinity, or -Infinity",
                )),
            }
        }
    }

    deserializer.deserialize_option(OptionF64Visitor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deserialize_f32_normal() {
        let val: f32 = deserialize_f32(&json!(3.14)).unwrap();
        assert!((val - 3.14).abs() < 0.001);
    }

    #[test]
    fn test_deserialize_f32_nan() {
        let val: f32 = deserialize_f32(&json!("NaN")).unwrap();
        assert!(val.is_nan());
    }

    #[test]
    fn test_deserialize_f32_infinity() {
        let val: f32 = deserialize_f32(&json!("Infinity")).unwrap();
        assert!(val.is_infinite() && val.is_sign_positive());
    }

    #[test]
    fn test_deserialize_f32_neg_infinity() {
        let val: f32 = deserialize_f32(&json!("-Infinity")).unwrap();
        assert!(val.is_infinite() && val.is_sign_negative());
    }

    #[test]
    fn test_deserialize_f64_normal() {
        let val: f64 = deserialize_f64(&json!(2.71828)).unwrap();
        assert!((val - 2.71828).abs() < 0.00001);
    }

    #[test]
    fn test_deserialize_f64_nan() {
        let val: f64 = deserialize_f64(&json!("NaN")).unwrap();
        assert!(val.is_nan());
    }

    #[test]
    fn test_deserialize_option_f32() {
        let val: Option<f32> = deserialize_option_f32(&json!(null)).unwrap();
        assert_eq!(val, None);

        let val: Option<f32> = deserialize_option_f32(&json!(3.14)).unwrap();
        assert!((val.unwrap() - 3.14).abs() < 0.001);

        let val: Option<f32> = deserialize_option_f32(&json!("NaN")).unwrap();
        assert!(val.unwrap().is_nan());
    }

    #[test]
    fn test_deserialize_option_f64() {
        let val: Option<f64> = deserialize_option_f64(&json!(null)).unwrap();
        assert_eq!(val, None);

        let val: Option<f64> = deserialize_option_f64(&json!(2.71828)).unwrap();
        assert!((val.unwrap() - 2.71828).abs() < 0.00001);

        let val: Option<f64> = deserialize_option_f64(&json!("Infinity")).unwrap();
        assert!(val.unwrap().is_infinite());
    }
}
