//! Owned query parameters for crossing pool job channels.
//!
//! [`crate::executor::LifeExecutor`] uses `&[&dyn ToSql]`, which cannot be sent across channels.
//! [`OwnedParam`] mirrors `sea_query::Value` variants used for pool dispatch (including
//! **`Value::Decimal`** when the `with-rust_decimal` feature is enabled on `sea-query`) so workers
//! can rebuild `ToSql` references on their stack.

use crate::executor::LifeError;
use crate::value::TextParam;
use may_postgres::types::ToSql;
use sea_query::Value;

/// Single bind parameter in owned form for pool dispatch.
#[derive(Clone, Debug)]
pub enum OwnedParam {
    Bool(bool),
    Int(Option<i32>),
    /// PostgreSQL `INT2` (`Value::TinyInt` / `SmallInt` / small unsigned).
    SmallInt(Option<i16>),
    BigInt(Option<i64>),
    Float(Option<f32>),
    Double(Option<f64>),
    /// TEXT-family bind that also accepts JSON/JSONB columns with
    /// `text::jsonb` cast semantics (see [`TextParam`]); `None` is SQL NULL.
    String(TextParam),
    Bytes(Option<Vec<u8>>),
    ChronoDate(Option<chrono::NaiveDate>),
    ChronoTime(Option<chrono::NaiveTime>),
    ChronoDateTime(Option<chrono::NaiveDateTime>),
    ChronoDateTimeUtc(Option<chrono::DateTime<chrono::Utc>>),
    ChronoDateTimeLocal(Option<chrono::DateTime<chrono::Local>>),
    Uuid(Option<uuid::Uuid>),
    /// `NUMERIC` / `rust_decimal::Decimal` (from `Value::Decimal`).
    Decimal(Option<rust_decimal::Decimal>),
    /// JSON / JSONB; `None` is SQL NULL (typed JSON null, not `GenericNull`).
    Json(Option<serde_json::Value>),
    /// Legacy bucket for `Value` variants that still map to a shared placeholder (see `converted_params`).
    GenericNull,
    /// A PostgreSQL array (`= ANY($1)`, `unnest($1, ..)` batch statements); see [`OwnedArray`].
    Array(OwnedArray),
}

/// A typed, nullable PostgreSQL array parameter, built from column values by the batch APIs.
#[derive(Clone, Debug)]
pub enum OwnedArray {
    Bool(Vec<Option<bool>>),
    SmallInt(Vec<Option<i16>>),
    Int(Vec<Option<i32>>),
    BigInt(Vec<Option<i64>>),
    Float(Vec<Option<f32>>),
    Double(Vec<Option<f64>>),
    Text(Vec<Option<String>>),
    Bytes(Vec<Option<Vec<u8>>>),
    Date(Vec<Option<chrono::NaiveDate>>),
    Time(Vec<Option<chrono::NaiveTime>>),
    DateTime(Vec<Option<chrono::NaiveDateTime>>),
    DateTimeUtc(Vec<Option<chrono::DateTime<chrono::Utc>>>),
    Uuid(Vec<Option<uuid::Uuid>>),
    Decimal(Vec<Option<rust_decimal::Decimal>>),
    Json(Vec<Option<serde_json::Value>>),
}

impl OwnedArray {
    /// An empty array of the element type `value` belongs to, with its SQL array type name
    /// (`int8`, `text`, ...). `None` for value types that have no array mapping.
    #[must_use]
    pub fn for_value(value: &Value) -> Option<(Self, &'static str)> {
        Some(match value {
            Value::Bool(_) => (Self::Bool(Vec::new()), "bool"),
            Value::TinyInt(_) | Value::SmallInt(_) | Value::TinyUnsigned(_) => {
                (Self::SmallInt(Vec::new()), "int2")
            }
            Value::Int(_) | Value::SmallUnsigned(_) => (Self::Int(Vec::new()), "int4"),
            Value::BigInt(_) | Value::Unsigned(_) | Value::BigUnsigned(_) => {
                (Self::BigInt(Vec::new()), "int8")
            }
            Value::Float(_) => (Self::Float(Vec::new()), "float4"),
            Value::Double(_) => (Self::Double(Vec::new()), "float8"),
            Value::String(_) | Value::Char(_) => (Self::Text(Vec::new()), "text"),
            Value::Bytes(_) => (Self::Bytes(Vec::new()), "bytea"),
            Value::ChronoDate(_) => (Self::Date(Vec::new()), "date"),
            Value::ChronoTime(_) => (Self::Time(Vec::new()), "time"),
            Value::ChronoDateTime(_) => (Self::DateTime(Vec::new()), "timestamp"),
            Value::ChronoDateTimeUtc(_) | Value::ChronoDateTimeLocal(_) => {
                (Self::DateTimeUtc(Vec::new()), "timestamptz")
            }
            Value::Uuid(_) => (Self::Uuid(Vec::new()), "uuid"),
            Value::Decimal(_) => (Self::Decimal(Vec::new()), "numeric"),
            Value::Json(_) => (Self::Json(Vec::new()), "jsonb"),
            _ => return None,
        })
    }

    /// Append `value`; `false` when its type does not match this array's element type.
    #[allow(clippy::cast_possible_wrap)]
    pub fn push(&mut self, value: &Value) -> bool {
        match (self, value) {
            (Self::Bool(v), Value::Bool(x)) => v.push(*x),
            (Self::SmallInt(v), Value::TinyInt(x)) => v.push(x.map(i16::from)),
            (Self::SmallInt(v), Value::SmallInt(x)) => v.push(*x),
            (Self::SmallInt(v), Value::TinyUnsigned(x)) => v.push(x.map(i16::from)),
            (Self::Int(v), Value::Int(x)) => v.push(*x),
            (Self::Int(v), Value::SmallUnsigned(x)) => v.push(x.map(i32::from)),
            (Self::BigInt(v), Value::BigInt(x)) => v.push(*x),
            (Self::BigInt(v), Value::Unsigned(x)) => v.push(x.map(i64::from)),
            (Self::BigInt(v), Value::BigUnsigned(x)) => match x {
                Some(u) if *u > i64::MAX as u64 => return false,
                _ => v.push(x.map(|u| u as i64)),
            },
            (Self::Float(v), Value::Float(x)) => v.push(*x),
            (Self::Double(v), Value::Double(x)) => v.push(*x),
            (Self::Text(v), Value::String(x)) => v.push(x.clone()),
            (Self::Text(v), Value::Char(x)) => v.push(x.map(|c| c.to_string())),
            (Self::Bytes(v), Value::Bytes(x)) => v.push(x.clone()),
            (Self::Date(v), Value::ChronoDate(x)) => v.push(*x),
            (Self::Time(v), Value::ChronoTime(x)) => v.push(*x),
            (Self::DateTime(v), Value::ChronoDateTime(x)) => v.push(*x),
            (Self::DateTimeUtc(v), Value::ChronoDateTimeUtc(x)) => v.push(*x),
            (Self::DateTimeUtc(v), Value::ChronoDateTimeLocal(x)) => {
                v.push(x.map(|d| d.with_timezone(&chrono::Utc)));
            }
            (Self::Uuid(v), Value::Uuid(x)) => v.push(*x),
            (Self::Decimal(v), Value::Decimal(x)) => v.push(*x),
            (Self::Json(v), Value::Json(x)) => v.push(x.as_deref().cloned()),
            _ => return false,
        }
        true
    }

    /// Number of elements.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Bool(v) => v.len(),
            Self::SmallInt(v) => v.len(),
            Self::Int(v) => v.len(),
            Self::BigInt(v) => v.len(),
            Self::Float(v) => v.len(),
            Self::Double(v) => v.len(),
            Self::Text(v) => v.len(),
            Self::Bytes(v) => v.len(),
            Self::Date(v) => v.len(),
            Self::Time(v) => v.len(),
            Self::DateTime(v) => v.len(),
            Self::DateTimeUtc(v) => v.len(),
            Self::Uuid(v) => v.len(),
            Self::Decimal(v) => v.len(),
            Self::Json(v) => v.len(),
        }
    }

    /// `true` when empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn as_sql_ref(&self) -> &dyn ToSql {
        match self {
            Self::Bool(v) => v as &dyn ToSql,
            Self::SmallInt(v) => v as &dyn ToSql,
            Self::Int(v) => v as &dyn ToSql,
            Self::BigInt(v) => v as &dyn ToSql,
            Self::Float(v) => v as &dyn ToSql,
            Self::Double(v) => v as &dyn ToSql,
            Self::Text(v) => v as &dyn ToSql,
            Self::Bytes(v) => v as &dyn ToSql,
            Self::Date(v) => v as &dyn ToSql,
            Self::Time(v) => v as &dyn ToSql,
            Self::DateTime(v) => v as &dyn ToSql,
            Self::DateTimeUtc(v) => v as &dyn ToSql,
            Self::Uuid(v) => v as &dyn ToSql,
            Self::Decimal(v) => v as &dyn ToSql,
            Self::Json(v) => v as &dyn ToSql,
        }
    }
}

impl OwnedParam {
    /// Borrow as `dyn ToSql` for the current statement bind.
    #[must_use]
    pub fn as_sql_ref(&self) -> &dyn ToSql {
        match self {
            OwnedParam::Bool(b) => b as &dyn ToSql,
            OwnedParam::Int(i) => i as &dyn ToSql,
            OwnedParam::SmallInt(i) => i as &dyn ToSql,
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
            OwnedParam::Decimal(d) => d as &dyn ToSql,
            OwnedParam::Json(v) => v as &dyn ToSql,
            OwnedParam::GenericNull => {
                static C: Option<i32> = None;
                &C as &dyn ToSql
            }
            OwnedParam::Array(a) => a.as_sql_ref(),
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

            Value::String(Some(s)) => Ok(OwnedParam::String(TextParam::some(s.clone()))),
            Value::String(None) => Ok(OwnedParam::String(TextParam::null())),

            Value::Bytes(Some(b)) => Ok(OwnedParam::Bytes(Some(b.clone()))),
            Value::Bytes(None) => Ok(OwnedParam::Bytes(None)),

            Value::TinyInt(Some(i)) => Ok(OwnedParam::SmallInt(Some(i16::from(*i)))),
            Value::SmallInt(Some(i)) => Ok(OwnedParam::SmallInt(Some(*i))),
            Value::TinyUnsigned(Some(u)) => Ok(OwnedParam::SmallInt(Some(i16::from(*u)))),
            Value::SmallUnsigned(Some(u)) => Ok(OwnedParam::SmallInt(Some(*u as i16))),
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
            | Value::SmallUnsigned(None) => Ok(OwnedParam::SmallInt(None)),
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

            Value::Decimal(Some(d)) => Ok(OwnedParam::Decimal(Some(*d))),
            Value::Decimal(None) => Ok(OwnedParam::Decimal(None)),

            Value::Json(Some(j)) => Ok(OwnedParam::Json(Some((**j).clone()))),
            Value::Json(None) => Ok(OwnedParam::Json(None)),

            _ => Err(LifeError::Other(format!(
                "Unsupported value type for pool parameter: {value:?}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)] // test-only unwraps

    use super::*;
    use rust_decimal::Decimal;
    use sea_query::Value;

    #[test]
    fn try_from_decimal_and_null() {
        let d = Decimal::new(12345, 2);
        let p = OwnedParam::try_from(&Value::Decimal(Some(d))).expect("try_from decimal");
        assert!(
            matches!(p, OwnedParam::Decimal(Some(x)) if x == d),
            "expected Decimal(Some({d:?})), got {p:?}"
        );
        assert!(matches!(
            OwnedParam::try_from(&Value::Decimal(None)),
            Ok(OwnedParam::Decimal(None))
        ));
    }

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
    fn try_from_smallint_maps_to_int2_path() {
        assert!(
            matches!(
                OwnedParam::try_from(&Value::SmallInt(Some(-9i16))),
                Ok(OwnedParam::SmallInt(Some(v))) if v == -9i16
            ),
            "SmallInt(Some(-9)) should map to OwnedParam::SmallInt"
        );
        assert!(matches!(
            OwnedParam::try_from(&Value::SmallInt(None)),
            Ok(OwnedParam::SmallInt(None))
        ));
    }

    #[test]
    fn as_sql_ref_smallint_encodes_for_int2() {
        use bytes::BytesMut;
        use postgres_types::{IsNull, Type};

        let p = OwnedParam::SmallInt(Some(40));
        let mut buf = BytesMut::new();
        let got = p.as_sql_ref().to_sql_checked(&Type::INT2, &mut buf);
        assert!(matches!(got, Ok(IsNull::No)));
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

    #[test]
    fn string_param_binds_to_jsonb_with_cast_semantics() {
        use bytes::BytesMut;
        use postgres_types::{IsNull, Type};

        // Stringified JSON document bound to a jsonb column must encode as
        // the parsed document (regression: previously failed with "cannot
        // convert between the Rust type Option<String> and jsonb").
        let doc = serde_json::json!({"step": 3, "form": {"city": "Oslo"}});
        let p = OwnedParam::try_from(&Value::String(Some(doc.to_string()))).expect("string param");
        let mut buf = BytesMut::new();
        assert!(matches!(
            p.as_sql_ref().to_sql_checked(&Type::JSONB, &mut buf),
            Ok(IsNull::No)
        ));
        let mut native = BytesMut::new();
        doc.to_sql_checked(&Type::JSONB, &mut native)
            .expect("native");
        assert_eq!(buf, native, "same wire bytes as binding serde_json::Value");

        // TEXT binding is unchanged.
        let mut tbuf = BytesMut::new();
        assert!(matches!(
            p.as_sql_ref().to_sql_checked(&Type::TEXT, &mut tbuf),
            Ok(IsNull::No)
        ));

        // Invalid JSON against jsonb errors with a useful message.
        let bad = OwnedParam::try_from(&Value::String(Some("not json".to_string())))
            .expect("string param");
        let mut bbuf = BytesMut::new();
        let res = bad.as_sql_ref().to_sql_checked(&Type::JSONB, &mut bbuf);
        assert!(res.is_err(), "invalid JSON must not bind to jsonb");
        if let Err(err) = res {
            assert!(err.to_string().contains("not valid JSON"), "{err}");
        }
    }

    #[test]
    fn try_from_json_encodes_for_json_and_jsonb() {
        use bytes::BytesMut;
        use postgres_types::{IsNull, Type};
        use sea_query::Value;

        let v = serde_json::json!({"k": 1});
        let p = OwnedParam::try_from(&Value::Json(Some(Box::new(v.clone())))).expect("json param");

        let mut buf = BytesMut::new();
        let got = p.as_sql_ref().to_sql_checked(&Type::JSON, &mut buf);
        assert!(matches!(got, Ok(IsNull::No)));
        assert!(!buf.is_empty());

        let mut buf2 = BytesMut::new();
        let got2 = p.as_sql_ref().to_sql_checked(&Type::JSONB, &mut buf2);
        assert!(matches!(got2, Ok(IsNull::No)));
        assert!(buf2.len() > 1);
    }

    #[test]
    fn try_from_string_none_and_json_none_use_typed_nulls() {
        use bytes::BytesMut;
        use postgres_types::{IsNull, Type};
        use sea_query::Value;

        let s = OwnedParam::try_from(&Value::String(None));
        assert!(matches!(s, Ok(OwnedParam::String(TextParam(None)))));
        let mut buf = BytesMut::new();
        assert!(matches!(
            s.expect("string null")
                .as_sql_ref()
                .to_sql_checked(&Type::TEXT, &mut buf),
            Ok(IsNull::Yes)
        ));

        let j = OwnedParam::try_from(&Value::Json(None));
        assert!(matches!(j, Ok(OwnedParam::Json(None))));
        let mut buf = BytesMut::new();
        assert!(matches!(
            j.expect("json null")
                .as_sql_ref()
                .to_sql_checked(&Type::JSONB, &mut buf),
            Ok(IsNull::Yes)
        ));
    }
}
