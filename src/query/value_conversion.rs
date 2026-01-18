//! Value conversion utilities for SeaQuery to may_postgres.
//!
//! This module provides functions to convert SeaQuery `Value` enums into
//! `ToSql` trait objects that can be used with `may_postgres` queries.
//!
//! The conversion follows a two-pass pattern:
//! 1. First pass: collect all values into typed vectors
//! 2. Second pass: create references to the stored values
//!
//! This pattern ensures that references remain valid within the closure scope.

use crate::executor::LifeError;
use may_postgres::types::ToSql;
use sea_query::Value;

/// Convert SeaQuery values to may_postgres ToSql parameters.
///
///
/// This function converts a slice of SeaQuery `Value` enums into
/// `ToSql` trait objects that can be used with `may_postgres`, then executes
/// a closure with the converted parameters.
///
/// The conversion follows the same pattern as `SelectQuery::all()` and `SelectQuery::one()`:
/// 1. First pass: collect all values into typed vectors
/// 2. Second pass: create references to the stored values
/// 3. Execute closure with the parameters (references are valid within closure scope)
///
/// # Arguments
///
/// * `values` - Slice of SeaQuery `Value` enums to convert
/// * `f` - Closure that receives the converted parameters and executes the database operation
///
/// # Returns
///
/// Returns the result of the closure, or an error if conversion fails.
///
/// # Errors
///
/// Returns `LifeError::Other` if an unsupported value type is encountered.
pub fn with_converted_params<F, R>(values: &sea_query::Values, f: F) -> Result<R, LifeError>
where
    F: FnOnce(&[&dyn ToSql]) -> Result<R, LifeError>,
{
    // Collect all values first - values are wrapped in Option in this version
    let mut bools: Vec<bool> = Vec::new();
    let mut ints: Vec<i32> = Vec::new();
    let mut big_ints: Vec<i64> = Vec::new();
    let mut strings: Vec<String> = Vec::new();
    let mut bytes: Vec<Vec<u8>> = Vec::new();
    let mut nulls: Vec<Option<i32>> = Vec::new();
    let mut floats: Vec<f32> = Vec::new();
    let mut doubles: Vec<f64> = Vec::new();

    // First pass: collect all values into typed vectors
    for value in values.iter() {
        match value {
            Value::Bool(Some(b)) => bools.push(*b),
            Value::Int(Some(i)) => ints.push(*i),
            Value::BigInt(Some(i)) => big_ints.push(*i),
            Value::String(Some(s)) => strings.push(s.clone()),
            Value::Bytes(Some(b)) => bytes.push(b.clone()),
            Value::Bool(None)
            | Value::Int(None)
            | Value::BigInt(None)
            | Value::String(None)
            | Value::Bytes(None) => nulls.push(None),
            Value::TinyInt(Some(i)) => ints.push(*i as i32),
            Value::SmallInt(Some(i)) => ints.push(*i as i32),
            Value::TinyUnsigned(Some(u)) => ints.push(*u as i32),
            Value::SmallUnsigned(Some(u)) => ints.push(*u as i32),
            Value::Unsigned(Some(u)) => big_ints.push(*u as i64),
            Value::BigUnsigned(Some(u)) => {
                if *u > i64::MAX as u64 {
                    return Err(LifeError::Other(format!(
                        "BigUnsigned value {} exceeds i64::MAX ({}), cannot be safely cast to i64",
                        u, i64::MAX
                    )));
                }
                big_ints.push(*u as i64);
            }
            Value::Float(Some(f)) => floats.push(*f),
            Value::Double(Some(d)) => doubles.push(*d),
            Value::TinyInt(None)
            | Value::SmallInt(None)
            | Value::TinyUnsigned(None)
            | Value::SmallUnsigned(None)
            | Value::Unsigned(None)
            | Value::BigUnsigned(None)
            | Value::Float(None)
            | Value::Double(None) => nulls.push(None),
            Value::Json(Some(j)) => {
                strings.push(serde_json::to_string(&**j).map_err(|e| {
                    LifeError::Other(format!("Failed to serialize JSON: {}", e))
                })?);
            }
            Value::Json(None) => nulls.push(None),
            _ => {
                return Err(LifeError::Other(format!(
                    "Unsupported value type in query: {:?}",
                    value
                )));
            }
        }
    }

    // Second pass: create references to the stored values
    let mut bool_idx = 0;
    let mut int_idx = 0;
    let mut big_int_idx = 0;
    let mut string_idx = 0;
    let mut byte_idx = 0;
    let mut null_idx = 0;
    let mut float_idx = 0;
    let mut double_idx = 0;

    let mut params: Vec<&dyn ToSql> = Vec::new();

    for value in values.iter() {
        match value {
            Value::Bool(Some(_)) => {
                params.push(&bools[bool_idx] as &dyn ToSql);
                bool_idx += 1;
            }
            Value::Int(Some(_)) => {
                params.push(&ints[int_idx] as &dyn ToSql);
                int_idx += 1;
            }
            Value::BigInt(Some(_)) => {
                params.push(&big_ints[big_int_idx] as &dyn ToSql);
                big_int_idx += 1;
            }
            Value::String(Some(_)) => {
                params.push(&strings[string_idx] as &dyn ToSql);
                string_idx += 1;
            }
            Value::Bytes(Some(_)) => {
                params.push(&bytes[byte_idx] as &dyn ToSql);
                byte_idx += 1;
            }
            Value::Bool(None)
            | Value::Int(None)
            | Value::BigInt(None)
            | Value::String(None)
            | Value::Bytes(None) => {
                params.push(&nulls[null_idx] as &dyn ToSql);
                null_idx += 1;
            }
            Value::TinyInt(Some(_))
            | Value::SmallInt(Some(_))
            | Value::TinyUnsigned(Some(_))
            | Value::SmallUnsigned(Some(_)) => {
                params.push(&ints[int_idx] as &dyn ToSql);
                int_idx += 1;
            }
            Value::Unsigned(Some(_)) | Value::BigUnsigned(Some(_)) => {
                params.push(&big_ints[big_int_idx] as &dyn ToSql);
                big_int_idx += 1;
            }
            Value::Float(Some(_)) => {
                params.push(&floats[float_idx] as &dyn ToSql);
                float_idx += 1;
            }
            Value::Double(Some(_)) => {
                params.push(&doubles[double_idx] as &dyn ToSql);
                double_idx += 1;
            }
            Value::TinyInt(None)
            | Value::SmallInt(None)
            | Value::TinyUnsigned(None)
            | Value::SmallUnsigned(None)
            | Value::Unsigned(None)
            | Value::BigUnsigned(None)
            | Value::Float(None)
            | Value::Double(None) => {
                params.push(&nulls[null_idx] as &dyn ToSql);
                null_idx += 1;
            }
            Value::Json(Some(_)) => {
                params.push(&strings[string_idx] as &dyn ToSql);
                string_idx += 1;
            }
            Value::Json(None) => {
                params.push(&nulls[null_idx] as &dyn ToSql);
                null_idx += 1;
            }
            _ => {
                return Err(LifeError::Other(format!(
                    "Unsupported value type in query: {:?}",
                    value
                )));
            }
        }
    }

    // Execute closure with the parameters (references are valid within closure scope)
    f(&params)
}
