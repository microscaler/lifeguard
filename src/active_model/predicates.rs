//! Built-in validation helpers for use in [`ActiveModelBehavior::validate_fields`](crate::active_model::ActiveModelBehavior::validate_fields)
//! and `#[validate(custom = path)]` (PRD Phase B — `range`, `len`-style predicates).
//!
//! Each function takes a [`sea_query::Value`] and returns `Result<(), String>` — the same shape as
//! `#[validate(custom = path)]` — so you can map to [`ValidationError`](crate::active_model::ValidationError) in
//! [`ActiveModelBehavior::validate_fields`](crate::active_model::ActiveModelBehavior::validate_fields).
//!
//! **Unset fields:** `None` / null scalar variants are treated as “nothing to validate” and return `Ok(())`.

use sea_query::Value;

/// Maximum UTF-8 **character** length (`.chars().count()`, not `.len()` bytes).
///
/// Unset `String(None)` or `Null` → `Ok(())`. Non-string values → `Ok(())` (use only on string columns).
pub fn string_utf8_chars_max(value: &Value, max: usize) -> Result<(), String> {
    if let Value::String(Some(s)) = value {
        let count = s.chars().count();
        if count > max {
            return Err(format!("must be at most {max} characters (got {count})"));
        }
    }
    Ok(())
}

/// Minimum and maximum UTF-8 character length (inclusive).
pub fn string_utf8_chars_in_range(value: &Value, min: usize, max: usize) -> Result<(), String> {
    match value {
        Value::String(Some(s)) => {
            let n = s.chars().count();
            if n < min || n > max {
                Err(format!(
                    "must be between {min} and {max} characters (got {n})"
                ))
            } else {
                Ok(())
            }
        }
        Value::String(None) => Ok(()),
        _ => Ok(()),
    }
}

/// Maximum **byte** length for `String` or `Bytes` payloads.
pub fn blob_or_string_byte_len_max(value: &Value, max: usize) -> Result<(), String> {
    match value {
        Value::String(Some(s)) if s.len() > max => {
            Err(format!("must be at most {max} bytes (got {})", s.len()))
        }
        Value::Bytes(Some(b)) if b.len() > max => {
            Err(format!("must be at most {max} bytes (got {})", b.len()))
        }
        Value::String(Some(_))
        | Value::String(None)
        | Value::Bytes(Some(_))
        | Value::Bytes(None) => Ok(()),
        _ => Ok(()),
    }
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::TinyInt(Some(v)) => Some(i64::from(*v)),
        Value::SmallInt(Some(v)) => Some(i64::from(*v)),
        Value::Int(Some(v)) => Some(i64::from(*v)),
        Value::BigInt(Some(v)) => Some(*v),
        Value::TinyUnsigned(Some(v)) => Some(i64::from(*v)),
        Value::SmallUnsigned(Some(v)) => Some(i64::from(*v)),
        Value::Unsigned(Some(v)) => Some(i64::from(*v)),
        Value::BigUnsigned(Some(v)) => i64::try_from(*v).ok(),
        _ => None,
    }
}

/// Inclusive `i64` range for integer-like [`Value`] variants.
///
/// Unset / null integer → `Ok(())`. Values that do not map to `i64` (e.g. float, string) → `Ok(())`;
/// use only on columns you know are integral, or combine with type checks.
pub fn i64_in_range(value: &Value, min: i64, max: i64) -> Result<(), String> {
    let Some(n) = value_as_i64(value) else {
        return Ok(());
    };
    if n < min || n > max {
        return Err(format!("must be between {min} and {max} (got {n})"));
    }
    Ok(())
}

fn value_as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Float(Some(v)) => Some(f64::from(*v)),
        Value::Double(Some(v)) => Some(*v),
        _ => None,
    }
}

/// Inclusive `f64` range for `Float` / `Double` values.
///
/// Unset / null → `Ok(())`. Non-float values → `Ok(())`.
pub fn f64_in_range(value: &Value, min: f64, max: f64) -> Result<(), String> {
    let Some(x) = value_as_f64(value) else {
        return Ok(());
    };
    if !x.is_finite() {
        return Err("must be a finite number".to_string());
    }
    if x < min || x > max {
        return Err(format!("must be between {min} and {max} (got {x})"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_utf8_chars_max_ok_empty() {
        assert!(string_utf8_chars_max(&Value::String(Some(String::new())), 5).is_ok());
    }

    #[test]
    fn string_utf8_chars_max_err() {
        assert_eq!(
            string_utf8_chars_max(&Value::String(Some("abcdef".to_string())), 5),
            Err("must be at most 5 characters (got 6)".to_string())
        );
    }

    #[test]
    fn string_utf8_chars_max_unicode_counts_chars() {
        assert!(string_utf8_chars_max(&Value::String(Some("é".to_string())), 1).is_ok());
        assert!(string_utf8_chars_max(&Value::String(Some("éé".to_string())), 1).is_err());
    }

    #[test]
    fn string_utf8_chars_in_range_unset_ok() {
        assert!(string_utf8_chars_in_range(&Value::String(None), 1, 10).is_ok());
    }

    #[test]
    fn i64_in_range_unset_ok() {
        assert!(i64_in_range(&Value::Int(None), 0, 10).is_ok());
    }

    #[test]
    fn i64_in_range_bounds() {
        assert!(i64_in_range(&Value::Int(Some(5)), 0, 10).is_ok());
        assert_eq!(
            i64_in_range(&Value::Int(Some(11)), 0, 10),
            Err("must be between 0 and 10 (got 11)".to_string())
        );
    }

    #[test]
    fn f64_in_range_rejects_nan() {
        assert_eq!(
            f64_in_range(&Value::Double(Some(f64::NAN)), 0.0, 1.0),
            Err("must be a finite number".to_string())
        );
    }
}
