//! Stable primary-key fingerprints for identity-map keys.
//!
//! [`sea_query::Value`] does not implement [`Hash`], so we use a deterministic string
//! representation of [`ModelTrait::get_primary_key_values`] for map keys.

use sea_query::Value;

/// Build a stable key string from primary-key values (for [`super::ModelIdentityMap`]).
///
/// A subset of [`Value`] variants is encoded explicitly for stability; all others use
/// [`Debug`](std::fmt::Debug) (stable enough for debugging; prefer extending the `match` for
/// production keys if you rely on exotic types).
#[must_use]
pub fn fingerprint_pk_values(values: &[Value]) -> String {
    values
        .iter()
        .map(fingerprint_one_value)
        .collect::<Vec<_>>()
        .join("\x1f")
}

fn fingerprint_one_value(v: &Value) -> String {
    match v {
        Value::Bool(Some(b)) => format!("bool:{b}"),
        Value::Bool(None) => "bool:null".to_string(),
        Value::TinyInt(Some(i)) => format!("ti:{i}"),
        Value::TinyInt(None) => "ti:null".to_string(),
        Value::SmallInt(Some(i)) => format!("si:{i}"),
        Value::SmallInt(None) => "si:null".to_string(),
        Value::Int(Some(i)) => format!("i:{i}"),
        Value::Int(None) => "i:null".to_string(),
        Value::BigInt(Some(i)) => format!("bi:{i}"),
        Value::BigInt(None) => "bi:null".to_string(),
        Value::Float(Some(f)) => format!("f:{f:?}"),
        Value::Float(None) => "f:null".to_string(),
        Value::Double(Some(d)) => format!("d:{d:?}"),
        Value::Double(None) => "d:null".to_string(),
        Value::String(Some(s)) => format!("s:{}", s.escape_debug()),
        Value::String(None) => "s:null".to_string(),
        Value::Uuid(Some(u)) => format!("u:{u}"),
        Value::Uuid(None) => "u:null".to_string(),
        _ => format!("dbg:{v:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_int_stable() {
        assert_eq!(fingerprint_pk_values(&[Value::Int(Some(7))]), "i:7");
    }

    #[test]
    fn fingerprint_composite_order() {
        let a = fingerprint_pk_values(&[Value::Int(Some(1)), Value::Int(Some(2))]);
        let b = fingerprint_pk_values(&[Value::Int(Some(2)), Value::Int(Some(1))]);
        assert_ne!(a, b);
    }
}
