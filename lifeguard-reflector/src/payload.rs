//! The NOTIFY payload contract.
//!
//! Lifeguard's generated triggers (`#[notify(...)]`, see `lifeguard-migrate`) publish
//! `{"table": "...", "op": "INSERT|UPDATE|DELETE", "id": <key>, "v": <i64>, "ts": <ms>}`.
//! The key is a JSON string or number depending on the column type; it is normalised to its text
//! form, which is also how cache keys spell it. `v` and `ts` are optional so payloads from older
//! triggers still parse: they lose stale-event detection and lag metrics, nothing else.

use serde_json::Value;

/// Row operation reported by the trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// A row was inserted.
    Insert,
    /// A row was updated.
    Update,
    /// A row was deleted.
    Delete,
    /// The table was truncated (statement-level trigger; no key).
    Truncate,
}

impl Op {
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_uppercase().as_str() {
            "INSERT" | "I" => Some(Self::Insert),
            "UPDATE" | "U" => Some(Self::Update),
            "DELETE" | "D" => Some(Self::Delete),
            "TRUNCATE" | "T" => Some(Self::Truncate),
            _ => None,
        }
    }
}

/// One decoded change notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Table name as the trigger reported it (`TG_TABLE_NAME`).
    pub table: String,
    /// Operation.
    pub op: Op,
    /// Primary key in text form; empty for [`Op::Truncate`].
    pub id: String,
    /// Version stamp, monotonic per key (WAL position at trigger time).
    pub version: Option<i64>,
    /// Trigger wall-clock time, milliseconds since the Unix epoch.
    pub ts_ms: Option<i64>,
}

/// Why a payload was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadError {
    /// Not JSON, or not an object.
    Malformed(String),
    /// A required field is missing or has the wrong type.
    Field(&'static str),
    /// Unknown `op`.
    Op(String),
}

impl std::fmt::Display for PayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(e) => write!(f, "payload is not a JSON object: {e}"),
            Self::Field(name) => write!(f, "payload field `{name}` missing or of the wrong type"),
            Self::Op(op) => write!(
                f,
                "payload op `{op}` is not INSERT, UPDATE, DELETE or TRUNCATE"
            ),
        }
    }
}

impl std::error::Error for PayloadError {}

/// Decode one payload.
///
/// # Errors
///
/// [`PayloadError`] when the payload is not the contract above.
pub fn parse(payload: &str) -> Result<Change, PayloadError> {
    let value: Value =
        serde_json::from_str(payload).map_err(|e| PayloadError::Malformed(e.to_string()))?;
    let Value::Object(map) = value else {
        return Err(PayloadError::Malformed("top level is not an object".into()));
    };
    let table = map
        .get("table")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .ok_or(PayloadError::Field("table"))?
        .to_owned();
    let op_text = map
        .get("op")
        .and_then(Value::as_str)
        .ok_or(PayloadError::Field("op"))?;
    let op = Op::parse(op_text).ok_or_else(|| PayloadError::Op(op_text.to_owned()))?;
    let id = match (op, map.get("id")) {
        (_, Some(Value::String(s))) if !s.is_empty() => s.clone(),
        (_, Some(Value::Number(n))) => n.to_string(),
        (Op::Truncate, None | Some(Value::Null)) => String::new(),
        _ => return Err(PayloadError::Field("id")),
    };
    let version = map.get("v").and_then(Value::as_i64);
    let ts_ms = map.get("ts").and_then(Value::as_i64);
    Ok(Change {
        table,
        op,
        id,
        version,
        ts_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_generated_trigger_payload() {
        let c = parse(r#"{"table":"items","op":"UPDATE","id":42,"v":123456,"ts":1700000000000}"#)
            .unwrap();
        assert_eq!(c.table, "items");
        assert_eq!(c.op, Op::Update);
        assert_eq!(c.id, "42");
        assert_eq!(c.version, Some(123_456));
        assert_eq!(c.ts_ms, Some(1_700_000_000_000));
    }

    #[test]
    fn string_keys_and_legacy_payloads() {
        let c = parse(r#"{"table":"t","op":"DELETE","id":"6f1c-uuid"}"#).unwrap();
        assert_eq!(c.id, "6f1c-uuid");
        assert_eq!(c.version, None);
        assert_eq!(c.ts_ms, None);
    }

    #[test]
    fn truncate_needs_no_key() {
        let c = parse(r#"{"table":"t","op":"TRUNCATE"}"#).unwrap();
        assert_eq!(c.op, Op::Truncate);
        assert_eq!(c.id, "");
    }

    #[test]
    fn rejects_bad_payloads() {
        assert!(matches!(parse("not json"), Err(PayloadError::Malformed(_))));
        assert!(matches!(parse("[1]"), Err(PayloadError::Malformed(_))));
        assert_eq!(
            parse(r#"{"op":"INSERT","id":1}"#),
            Err(PayloadError::Field("table"))
        );
        assert_eq!(
            parse(r#"{"table":"t","id":1}"#),
            Err(PayloadError::Field("op"))
        );
        assert_eq!(
            parse(r#"{"table":"t","op":"MERGE","id":1}"#),
            Err(PayloadError::Op("MERGE".into()))
        );
        assert_eq!(
            parse(r#"{"table":"t","op":"INSERT"}"#),
            Err(PayloadError::Field("id"))
        );
        assert_eq!(
            parse(r#"{"table":"t","op":"INSERT","id":null}"#),
            Err(PayloadError::Field("id"))
        );
    }
}
