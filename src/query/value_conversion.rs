//! Value conversion utilities for `SeaQuery` to `may_postgres`.
//!
//! Delegates to [`super::converted_params`] so parameter binding stays in sync with
//! [`crate::active_model::conversion::with_converted_params`].

use crate::executor::LifeError;
use may_postgres::types::ToSql;

/// Convert `SeaQuery` values to `may_postgres` `ToSql` parameters.
///
/// The conversion follows the same pattern as `SelectQuery::all()` and `SelectQuery::one()`:
/// 1. First pass: collect all values into typed vectors
/// 2. Second pass: create references to the stored values
/// 3. Execute closure with the parameters (references are valid within closure scope)
///
/// # Errors
///
/// Returns `LifeError::Other` if an unsupported value type is encountered.
pub fn with_converted_params<F, R>(values: &sea_query::Values, f: F) -> Result<R, LifeError>
where
    F: FnOnce(&[&dyn ToSql]) -> Result<R, LifeError>,
{
    super::converted_params::with_converted_value_slice(&values.0, LifeError::Other, f)
}

#[cfg(test)]
mod typed_null_sql_tests {
    use super::with_converted_params;
    use crate::executor::LifeError;
    use bytes::BytesMut;
    use postgres_types::{IsNull, Type};
    use sea_query::{Value, Values};

    #[test]
    fn uuid_sql_null_encodes_as_null_for_uuid_param() -> Result<(), LifeError> {
        let values = Values(vec![Value::Uuid(None)]);
        with_converted_params(&values, |params| {
            let mut buf = BytesMut::new();
            let got = params[0]
                .to_sql_checked(&Type::UUID, &mut buf)
                .map_err(|e| LifeError::Other(format!("to_sql_checked: {e}")))?;
            match got {
                IsNull::Yes => Ok(()),
                IsNull::No => Err(LifeError::Other(
                    "SQL NULL for UUID must encode as IsNull::Yes (Option<Uuid> bind)".to_string(),
                )),
            }
        })
    }
}
