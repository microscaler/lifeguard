//! Text bind parameter with JSON/JSONB cast semantics.
//!
//! `postgres-types` only lets `String` bind to TEXT-family columns, so a
//! serialized JSON document passed as a string against a `jsonb` column fails
//! at bind time with *"cannot convert between the Rust type
//! `Option<String>` and the Postgres type `jsonb`"* — a recurring footgun
//! for raw statements and `execute_values` callers (`Value::Json` is the
//! idiomatic carrier, but stringified JSON is common at API boundaries).
//!
//! [`TextParam`] is the bind-time carrier for `sea_query::Value::String` on
//! both dispatch paths (`converted_params` for direct executors,
//! `OwnedParam` for the pool). For TEXT-family columns it behaves exactly
//! like `String`. For JSON/JSONB columns it applies PostgreSQL's own
//! `text::jsonb` cast semantics: parse the string as a JSON document and
//! bind that, erroring on invalid JSON (matching what `($n::text)::jsonb`
//! would do server-side).

use bytes::BytesMut;
use may_postgres::types::{IsNull, ToSql, Type};

/// Owned text parameter: binds as TEXT, and as a parsed JSON document when
/// the target column is JSON/JSONB. `None` is a typed SQL NULL for all
/// accepted column types.
#[derive(Clone, Debug)]
pub struct TextParam(pub Option<String>);

impl TextParam {
    /// Wrap an owned string.
    #[must_use]
    pub fn some(s: String) -> Self {
        Self(Some(s))
    }

    /// Typed NULL (TEXT or JSON/JSONB).
    #[must_use]
    pub fn null() -> Self {
        Self(None)
    }
}

impl ToSql for TextParam {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        let Some(s) = &self.0 else {
            return Ok(IsNull::Yes);
        };
        if *ty == Type::JSON || *ty == Type::JSONB {
            // PostgreSQL `text::jsonb` semantics: the string must itself be a
            // valid JSON document ('{"k":1}', '"str"', '1', …). Encoding the
            // raw string as a JSON string scalar here would silently change
            // the document shape, so invalid JSON is an error instead.
            let doc: serde_json::Value = serde_json::from_str(s).map_err(|e| {
                format!(
                    "string parameter bound to a {ty} column is not valid JSON \
                     (use Value::Json / serde_json::Value for documents): {e}"
                )
            })?;
            doc.to_sql(ty, out)
        } else {
            <String as ToSql>::to_sql(s, ty, out)
        }
    }

    fn accepts(ty: &Type) -> bool {
        <String as ToSql>::accepts(ty) || *ty == Type::JSON || *ty == Type::JSONB
    }

    postgres_types::to_sql_checked!();
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)] // test-only unwraps

    use super::*;

    /// `Ok(true)` = bound as SQL NULL, `Ok(false)` = bound a value.
    fn checked(param: &TextParam, ty: &Type) -> Result<bool, String> {
        let mut buf = BytesMut::new();
        param
            .to_sql_checked(ty, &mut buf)
            .map(|n| matches!(n, IsNull::Yes))
            .map_err(|e| e.to_string())
    }

    #[test]
    fn accepts_text_family_and_json_types() {
        assert!(TextParam::accepts(&Type::TEXT));
        assert!(TextParam::accepts(&Type::VARCHAR));
        assert!(TextParam::accepts(&Type::JSON));
        assert!(TextParam::accepts(&Type::JSONB));
        assert!(!TextParam::accepts(&Type::INT4));
    }

    #[test]
    fn text_binding_is_unchanged_from_string() {
        let p = TextParam::some("hello".to_string());
        let mut ours = BytesMut::new();
        p.to_sql_checked(&Type::TEXT, &mut ours).expect("text");
        let mut theirs = BytesMut::new();
        "hello"
            .to_string()
            .to_sql_checked(&Type::TEXT, &mut theirs)
            .expect("string text");
        assert_eq!(ours, theirs, "TEXT encoding identical to String");
    }

    #[test]
    fn json_document_string_binds_to_jsonb_as_parsed_document() {
        let doc = serde_json::json!({"status": "completed", "n": 1});
        let p = TextParam::some(doc.to_string());
        let mut ours = BytesMut::new();
        let is_null = p.to_sql_checked(&Type::JSONB, &mut ours).expect("jsonb");
        assert!(matches!(is_null, IsNull::No));

        // Identical wire bytes to binding the parsed serde_json::Value.
        let mut theirs = BytesMut::new();
        doc.to_sql_checked(&Type::JSONB, &mut theirs)
            .expect("value jsonb");
        assert_eq!(ours, theirs, "matches native serde_json::Value encoding");
    }

    #[test]
    fn json_scalar_strings_follow_pg_cast_semantics() {
        // '"abc"'::jsonb is a JSON string scalar; '1'::jsonb a number.
        assert!(checked(&TextParam::some("\"abc\"".into()), &Type::JSONB).is_ok());
        assert!(checked(&TextParam::some("1".into()), &Type::JSONB).is_ok());
    }

    #[test]
    fn invalid_json_errors_on_jsonb_but_binds_as_text() {
        let p = TextParam::some("not json".to_string());
        let err = checked(&p, &Type::JSONB).expect_err("invalid JSON must not bind to jsonb");
        assert!(err.contains("not valid JSON"), "useful error, got: {err}");
        assert!(
            checked(&p, &Type::TEXT).is_ok(),
            "same value is fine as TEXT"
        );
    }

    #[test]
    fn null_binds_to_text_and_jsonb() {
        let p = TextParam::null();
        assert!(matches!(checked(&p, &Type::TEXT), Ok(true)));
        assert!(matches!(checked(&p, &Type::JSONB), Ok(true)));
    }
}
