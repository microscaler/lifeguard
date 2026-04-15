//! Shared `sea_query::Value` → `may_postgres` `ToSql` parameter building.
//!
//! Used by [`super::value_conversion::with_converted_params`] (`LifeError`) and
//! [`crate::active_model::conversion::with_converted_params`] (`ActiveModelError`) so new
//! `Value` variants are handled in one place.
//!
//! ## Two-pass invariant
//!
//! 1. **Pass 1** walks `values` **in order** and appends into typed storage (`strings`, `ints`,
//!    `null_chrono_datetimes_utc`, …).
//! 2. **Pass 2** walks the **same `values` slice in the same order** and, for each element, takes
//!    the next `&dyn ToSql` from the **matching** bucket (`string_idx`, `chrono_datetime_utc_null_idx`, …).
//!
//! So the *i*-th input `Value` always becomes the *i*-th bind parameter. When adding a variant,
//! both passes must stay in lockstep.
//!
//! ## Typed NULLs vs generic `nulls`
//!
//! `Value::{String,Json}(None)` and several other variants share the **`nulls: Vec<Option<i32>>`**
//! bucket because `postgres-types` accepts that representation for those OIDs. **Do not** route
//! `ChronoDateTimeUtc(None)`, `Uuid(None)`, or other typed NULLs through that generic bucket —
//! they use `Option<T>` vectors with the correct `T` so `ToSql::accepts` matches the column (see
//! `docs/CHRONO_AND_POSTGRES_TYPES.md` and PRD §5.2).

use may_postgres::types::ToSql;
use sea_query::Value;

/// Two-pass conversion: collect typed storage, then `&dyn ToSql` refs for one statement.
pub(crate) fn with_converted_value_slice<F, R, E, IE>(
    values: &[Value],
    into_err: IE,
    f: F,
) -> Result<R, E>
where
    F: FnOnce(&[&dyn ToSql]) -> Result<R, E>,
    IE: Fn(String) -> E,
{
    let mut bools: Vec<bool> = Vec::new();
    let mut ints: Vec<i32> = Vec::new();
    let mut big_ints: Vec<i64> = Vec::new();
    let mut strings: Vec<String> = Vec::new();
    let mut bytes: Vec<Vec<u8>> = Vec::new();
    let mut nulls: Vec<Option<i32>> = Vec::new();
    let mut floats: Vec<f32> = Vec::new();
    let mut doubles: Vec<f64> = Vec::new();

    let mut decimals: Vec<rust_decimal::Decimal> = Vec::new();

    let mut chrono_dates: Vec<chrono::NaiveDate> = Vec::new();
    let mut chrono_times: Vec<chrono::NaiveTime> = Vec::new();
    let mut chrono_date_times: Vec<chrono::NaiveDateTime> = Vec::new();
    let mut chrono_date_times_utc: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();
    let mut chrono_date_times_local: Vec<chrono::DateTime<chrono::Local>> = Vec::new();

    let mut uuids: Vec<uuid::Uuid> = Vec::new();

    // Typed SQL NULLs: `Option<i32>::None` must not be used for UUID/timestamp params —
    // `ToSql::accepts` for `Option<T>` delegates to `T::accepts` (postgres-types).
    let mut null_uuids: Vec<Option<uuid::Uuid>> = Vec::new();
    let mut null_chrono_dates: Vec<Option<chrono::NaiveDate>> = Vec::new();
    let mut null_chrono_times: Vec<Option<chrono::NaiveTime>> = Vec::new();
    let mut null_chrono_datetimes: Vec<Option<chrono::NaiveDateTime>> = Vec::new();
    let mut null_chrono_datetimes_utc: Vec<Option<chrono::DateTime<chrono::Utc>>> = Vec::new();
    let mut null_chrono_datetimes_local: Vec<Option<chrono::DateTime<chrono::Local>>> = Vec::new();

    for value in values {
        match value {
            Value::Bool(Some(b)) => bools.push(*b),
            Value::Int(Some(i)) => ints.push(*i),
            Value::BigInt(Some(i)) => big_ints.push(*i),
            Value::String(Some(s)) => strings.push(s.clone()),
            Value::Bytes(Some(b)) => bytes.push(b.clone()),
            Value::TinyInt(Some(i)) => ints.push(i32::from(*i)),
            Value::SmallInt(Some(i)) => ints.push(i32::from(*i)),
            Value::TinyUnsigned(Some(u)) => ints.push(i32::from(*u)),
            Value::SmallUnsigned(Some(u)) => ints.push(i32::from(*u)),
            Value::Unsigned(Some(u)) => big_ints.push(i64::from(*u)),
            Value::BigUnsigned(Some(u)) => {
                #[allow(clippy::cast_sign_loss)]
                if *u > i64::MAX as u64 {
                    return Err(into_err(format!(
                        "BigUnsigned value {u} exceeds i64::MAX ({}), cannot be safely cast to i64",
                        i64::MAX
                    )));
                }
                #[allow(clippy::cast_possible_wrap)]
                big_ints.push(*u as i64);
            }
            Value::Float(Some(fl)) => floats.push(*fl),
            Value::Double(Some(d)) => doubles.push(*d),

            Value::Decimal(Some(d)) => decimals.push(*d),

            Value::ChronoDate(Some(d)) => chrono_dates.push(*d),
            Value::ChronoTime(Some(t)) => chrono_times.push(*t),
            Value::ChronoDateTime(Some(dt)) => chrono_date_times.push(*dt),
            Value::ChronoDateTimeUtc(Some(dt)) => chrono_date_times_utc.push(*dt),
            Value::ChronoDateTimeLocal(Some(dt)) => chrono_date_times_local.push(*dt),

            Value::Uuid(Some(u)) => uuids.push(*u),

            #[allow(clippy::match_same_arms)]
            Value::Bool(None)
            | Value::Int(None)
            | Value::BigInt(None)
            | Value::String(None)
            | Value::Bytes(None)
            | Value::TinyInt(None)
            | Value::SmallInt(None)
            | Value::TinyUnsigned(None)
            | Value::SmallUnsigned(None)
            | Value::Unsigned(None)
            | Value::BigUnsigned(None)
            | Value::Float(None)
            | Value::Double(None) => nulls.push(None),

            Value::Decimal(None) => nulls.push(None),

            Value::ChronoDate(None) => null_chrono_dates.push(None),
            Value::ChronoTime(None) => null_chrono_times.push(None),
            Value::ChronoDateTime(None) => null_chrono_datetimes.push(None),
            Value::ChronoDateTimeUtc(None) => null_chrono_datetimes_utc.push(None),
            Value::ChronoDateTimeLocal(None) => null_chrono_datetimes_local.push(None),
            Value::Uuid(None) => null_uuids.push(None),

            Value::Json(Some(j)) => {
                strings.push(
                    serde_json::to_string(&**j)
                        .map_err(|e| into_err(format!("Failed to serialize JSON: {e}")))?,
                );
            }
            Value::Json(None) => nulls.push(None),

            _ => {
                return Err(into_err(format!(
                    "Unsupported value type in query: {value:?}"
                )));
            }
        }
    }

    let mut bool_idx = 0;
    let mut int_idx = 0;
    let mut big_int_idx = 0;
    let mut string_idx = 0;
    let mut byte_idx = 0;
    let mut null_idx = 0;
    let mut float_idx = 0;
    let mut double_idx = 0;

    let mut chrono_date_idx = 0;
    let mut chrono_time_idx = 0;
    let mut chrono_datetime_idx = 0;
    let mut chrono_datetime_utc_idx = 0;
    let mut chrono_datetime_local_idx = 0;

    let mut uuid_idx = 0;

    let mut uuid_null_idx = 0;
    let mut chrono_date_null_idx = 0;
    let mut chrono_time_null_idx = 0;
    let mut chrono_datetime_null_idx = 0;
    let mut chrono_datetime_utc_null_idx = 0;
    let mut chrono_datetime_local_null_idx = 0;

    let mut decimal_idx = 0;

    let mut params: Vec<&dyn ToSql> = Vec::new();

    // Second pass: same iteration order as above; index *_idx mirrors consumption from each bucket.
    for value in values {
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

            Value::ChronoDate(Some(_)) => {
                params.push(&chrono_dates[chrono_date_idx] as &dyn ToSql);
                chrono_date_idx += 1;
            }
            Value::ChronoTime(Some(_)) => {
                params.push(&chrono_times[chrono_time_idx] as &dyn ToSql);
                chrono_time_idx += 1;
            }
            Value::ChronoDateTime(Some(_)) => {
                params.push(&chrono_date_times[chrono_datetime_idx] as &dyn ToSql);
                chrono_datetime_idx += 1;
            }
            Value::ChronoDateTimeUtc(Some(_)) => {
                params.push(&chrono_date_times_utc[chrono_datetime_utc_idx] as &dyn ToSql);
                chrono_datetime_utc_idx += 1;
            }
            Value::ChronoDateTimeLocal(Some(_)) => {
                params.push(&chrono_date_times_local[chrono_datetime_local_idx] as &dyn ToSql);
                chrono_datetime_local_idx += 1;
            }

            Value::Uuid(Some(_)) => {
                params.push(&uuids[uuid_idx] as &dyn ToSql);
                uuid_idx += 1;
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
            Value::Decimal(Some(_)) => {
                params.push(&decimals[decimal_idx] as &dyn ToSql);
                decimal_idx += 1;
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
            Value::Decimal(None) => {
                params.push(&nulls[null_idx] as &dyn ToSql);
                null_idx += 1;
            }

            Value::ChronoDate(None) => {
                params.push(&null_chrono_dates[chrono_date_null_idx] as &dyn ToSql);
                chrono_date_null_idx += 1;
            }
            Value::ChronoTime(None) => {
                params.push(&null_chrono_times[chrono_time_null_idx] as &dyn ToSql);
                chrono_time_null_idx += 1;
            }
            Value::ChronoDateTime(None) => {
                params.push(&null_chrono_datetimes[chrono_datetime_null_idx] as &dyn ToSql);
                chrono_datetime_null_idx += 1;
            }
            Value::ChronoDateTimeUtc(None) => {
                params.push(&null_chrono_datetimes_utc[chrono_datetime_utc_null_idx] as &dyn ToSql);
                chrono_datetime_utc_null_idx += 1;
            }
            Value::ChronoDateTimeLocal(None) => {
                params.push(
                    &null_chrono_datetimes_local[chrono_datetime_local_null_idx] as &dyn ToSql,
                );
                chrono_datetime_local_null_idx += 1;
            }

            Value::Json(Some(_)) => {
                params.push(&strings[string_idx] as &dyn ToSql);
                string_idx += 1;
            }
            Value::Json(None) => {
                params.push(&nulls[null_idx] as &dyn ToSql);
                null_idx += 1;
            }
            Value::Uuid(None) => {
                params.push(&null_uuids[uuid_null_idx] as &dyn ToSql);
                uuid_null_idx += 1;
            }
            _ => {
                return Err(into_err(format!(
                    "Unsupported value type in query: {value:?}"
                )));
            }
        }
    }

    f(&params)
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use sea_query::Value;
    use std::str::FromStr;

    #[test]
    fn mixed_string_json_and_chrono_utc_nulls_one_slice() {
        let values = vec![
            Value::Int(Some(1)),
            Value::String(None),
            Value::Json(None),
            Value::ChronoDateTimeUtc(None),
            Value::String(Some("x".into())),
            Value::ChronoDateTimeUtc(Some(Utc::now())),
        ];
        let result = with_converted_value_slice(&values, |e| e, |params| {
            assert_eq!(
                params.len(),
                values.len(),
                "each Value must yield exactly one bind parameter"
            );
            Ok::<(), String>(())
        });
        assert!(result.is_ok(), "{:?}", result.err());
    }

    #[test]
    fn typed_uuid_null_separate_from_generic_nulls() {
        let values = vec![
            Value::Uuid(None),
            Value::Int(None),
            Value::ChronoDateTime(None),
        ];
        let result = with_converted_value_slice(&values, |e| e, |params| {
            assert_eq!(params.len(), 3);
            Ok::<(), String>(())
        });
        assert!(result.is_ok(), "{:?}", result.err());
    }

    #[test]
    #[allow(clippy::unwrap_used)] // literal decimal string; crate denies unwrap in non-test paths only
    fn test_converted_params_decimal() {
        let dec = Decimal::from_str("123.45").unwrap();

        let values = vec![Value::Decimal(Some(dec)), Value::Decimal(None)];

        let result = with_converted_value_slice(
            &values,
            |e| e,
            |params| {
                assert_eq!(params.len(), 2, "Should bind 2 parameters");
                Ok::<(), String>(())
            },
        );

        assert!(
            result.is_ok(),
            "Decimal conversion failed with error: {:?}",
            result.err()
        );
    }
}
