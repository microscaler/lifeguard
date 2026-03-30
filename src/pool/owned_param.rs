//! Owned query parameters for crossing pool job channels.
//!
//! [`crate::executor::LifeExecutor`] uses `&[&dyn ToSql]`, which cannot be sent across channels.
//! [`OwnedParam`] mirrors the `sea_query::Value` variants supported by
//! [`crate::query::converted_params::with_converted_value_slice`] so workers can rebuild `ToSql`
//! references on their stack.

use crate::executor::LifeError;
use may_postgres::types::ToSql;
use sea_query::Value;

/// Single bind parameter in owned form for pool dispatch.
#[derive(Clone, Debug)]
pub enum OwnedParam {
    Bool(bool),
    Int(Option<i32>),
    BigInt(Option<i64>),
    Float(Option<f32>),
    Double(Option<f64>),
    String(Option<String>),
    Bytes(Option<Vec<u8>>),
    ChronoDate(Option<chrono::NaiveDate>),
    ChronoTime(Option<chrono::NaiveTime>),
    ChronoDateTime(Option<chrono::NaiveDateTime>),
    ChronoDateTimeUtc(Option<chrono::DateTime<chrono::Utc>>),
    ChronoDateTimeLocal(Option<chrono::DateTime<chrono::Local>>),
    Uuid(Option<uuid::Uuid>),
    /// JSON text for JSON/JSONB parameters (from `Value::Json(Some(..))`).
    JsonText(Option<String>),
    /// `sea_query::Value` nulls that bind as `Option<i32>::None` (see `converted_params`).
    GenericNull,
}

impl OwnedParam {
    /// Borrow as `dyn ToSql` for the current statement bind.
    pub(crate) fn as_sql_ref(&self) -> &dyn ToSql {
        match self {
            OwnedParam::Bool(b) => b as &dyn ToSql,
            OwnedParam::Int(i) => i as &dyn ToSql,
            OwnedParam::BigInt(i) => i as &dyn ToSql,
            OwnedParam::Float(f) => f as &dyn ToSql,
            OwnedParam::Double(d) => d as &dyn ToSql,
            OwnedParam::String(s) => s as &dyn ToSql,
            OwnedParam::Bytes(b) => b as &dyn ToSql,
            OwnedParam::ChronoDate(d) => d as &dyn ToSql,
            OwnedParam::ChronoTime(t) => t as &dyn ToSql,
            OwnedParam::ChronoDateTime(dt) => dt as &dyn ToSql,
            OwnedParam::ChronoDateTimeUtc(dt) => dt as &dyn ToSql,
            OwnedParam::ChronoDateTimeLocal(dt) => dt as &dyn ToSql,
            OwnedParam::Uuid(u) => u as &dyn ToSql,
            OwnedParam::JsonText(s) => s as &dyn ToSql,
            OwnedParam::GenericNull => {
                static C: Option<i32> = None;
                &C as &dyn ToSql
            }
        }
    }
}

impl TryFrom<&Value> for OwnedParam {
    type Error = LifeError;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bool(Some(b)) => Ok(OwnedParam::Bool(*b)),
            Value::Bool(None) => Ok(OwnedParam::GenericNull),

            Value::Int(Some(i)) => Ok(OwnedParam::Int(Some(*i))),
            Value::Int(None) => Ok(OwnedParam::GenericNull),

            Value::BigInt(Some(i)) => Ok(OwnedParam::BigInt(Some(*i))),
            Value::BigInt(None) => Ok(OwnedParam::GenericNull),

            Value::String(Some(s)) => Ok(OwnedParam::String(Some(s.clone()))),
            Value::String(None) => Ok(OwnedParam::GenericNull),

            Value::Bytes(Some(b)) => Ok(OwnedParam::Bytes(Some(b.clone()))),
            Value::Bytes(None) => Ok(OwnedParam::GenericNull),

            Value::TinyInt(Some(i)) => Ok(OwnedParam::Int(Some(i32::from(*i)))),
            Value::SmallInt(Some(i)) => Ok(OwnedParam::Int(Some(i32::from(*i)))),
            Value::TinyUnsigned(Some(u)) => Ok(OwnedParam::Int(Some(i32::from(*u)))),
            Value::SmallUnsigned(Some(u)) => Ok(OwnedParam::Int(Some(i32::from(*u)))),
            Value::Unsigned(Some(u)) => Ok(OwnedParam::BigInt(Some(i64::from(*u)))),
            Value::BigUnsigned(Some(u)) => {
                if *u > i64::MAX as u64 {
                    return Err(LifeError::Other(format!(
                        "BigUnsigned value {u} exceeds i64::MAX ({}), cannot be safely cast to i64",
                        i64::MAX
                    )));
                }
                #[allow(clippy::cast_possible_wrap)]
                Ok(OwnedParam::BigInt(Some(*u as i64)))
            }
            Value::TinyInt(None)
            | Value::SmallInt(None)
            | Value::TinyUnsigned(None)
            | Value::SmallUnsigned(None) => Ok(OwnedParam::GenericNull),
            Value::Unsigned(None) | Value::BigUnsigned(None) => Ok(OwnedParam::GenericNull),

            Value::Float(Some(f)) => Ok(OwnedParam::Float(Some(*f))),
            Value::Float(None) => Ok(OwnedParam::GenericNull),
            Value::Double(Some(d)) => Ok(OwnedParam::Double(Some(*d))),
            Value::Double(None) => Ok(OwnedParam::GenericNull),

            Value::ChronoDate(Some(d)) => Ok(OwnedParam::ChronoDate(Some(*d))),
            Value::ChronoDate(None) => Ok(OwnedParam::ChronoDate(None)),
            Value::ChronoTime(Some(t)) => Ok(OwnedParam::ChronoTime(Some(*t))),
            Value::ChronoTime(None) => Ok(OwnedParam::ChronoTime(None)),
            Value::ChronoDateTime(Some(dt)) => Ok(OwnedParam::ChronoDateTime(Some(*dt))),
            Value::ChronoDateTime(None) => Ok(OwnedParam::ChronoDateTime(None)),
            Value::ChronoDateTimeUtc(Some(dt)) => Ok(OwnedParam::ChronoDateTimeUtc(Some(*dt))),
            Value::ChronoDateTimeUtc(None) => Ok(OwnedParam::ChronoDateTimeUtc(None)),
            Value::ChronoDateTimeLocal(Some(dt)) => Ok(OwnedParam::ChronoDateTimeLocal(Some(*dt))),
            Value::ChronoDateTimeLocal(None) => Ok(OwnedParam::ChronoDateTimeLocal(None)),

            Value::Uuid(Some(u)) => Ok(OwnedParam::Uuid(Some(*u))),
            Value::Uuid(None) => Ok(OwnedParam::Uuid(None)),

            Value::Json(Some(j)) => Ok(OwnedParam::JsonText(Some(
                serde_json::to_string(&**j)
                    .map_err(|e| LifeError::Other(format!("Failed to serialize JSON: {e}")))?,
            ))),
            Value::Json(None) => Ok(OwnedParam::GenericNull),

            _ => Err(LifeError::Other(format!(
                "Unsupported value type for pool parameter: {value:?}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_query::Value;

    #[test]
    fn try_from_int_and_null() {
        assert!(matches!(
            OwnedParam::try_from(&Value::Int(Some(7))),
            Ok(OwnedParam::Int(Some(7)))
        ));
        assert!(matches!(
            OwnedParam::try_from(&Value::Int(None)),
            Ok(OwnedParam::GenericNull)
        ));
    }

    #[test]
    fn as_sql_ref_uuid_none_roundtrip_encoding() {
        use bytes::BytesMut;
        use postgres_types::{IsNull, Type};

        let p = OwnedParam::Uuid(None);
        let mut buf = BytesMut::new();
        let got = p.as_sql_ref().to_sql_checked(&Type::UUID, &mut buf);
        assert!(matches!(got, Ok(IsNull::Yes)));
    }
}
