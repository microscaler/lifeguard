//! Value conversion utilities for `ActiveModel` operations.
//!
//! Delegates to [`crate::query::converted_params`] so parameter binding stays in sync with
//! [`crate::query::value_conversion::with_converted_params`] (`LifeError`).

use super::error::ActiveModelError;
use may_postgres::types::ToSql;
use sea_query::Value;

/// Convert `SeaQuery` values to `may_postgres` `ToSql` parameters and execute a closure
///
/// # Errors
///
/// Returns `ActiveModelError::Other` if an unsupported value type is encountered.
pub fn with_converted_params<F, R>(values: &[Value], f: F) -> Result<R, ActiveModelError>
where
    F: FnOnce(&[&dyn ToSql]) -> Result<R, ActiveModelError>,
{
    crate::query::converted_params::with_converted_value_slice(values, ActiveModelError::Other, f)
}

#[cfg(test)]
mod typed_null_sql_tests {
    use super::with_converted_params;
    use super::ActiveModelError;
    use bytes::BytesMut;
    use postgres_types::{IsNull, Type};
    use sea_query::Value;

    #[test]
    fn uuid_sql_null_encodes_as_null_for_uuid_param() -> Result<(), ActiveModelError> {
        let values = [Value::Uuid(None)];
        with_converted_params(&values, |params| {
            let mut buf = BytesMut::new();
            let got = params[0]
                .to_sql_checked(&Type::UUID, &mut buf)
                .map_err(|e| ActiveModelError::Other(format!("to_sql_checked: {e}")))?;
            match got {
                IsNull::Yes => Ok(()),
                IsNull::No => Err(ActiveModelError::Other(
                    "SQL NULL for UUID must encode as IsNull::Yes (Option<Uuid> bind)".to_string(),
                )),
            }
        })
    }
}
