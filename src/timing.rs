//! Operation timing: Lifeguard's pool, statements, row decoding and Redis cache in the shared
//! `op_duration_seconds{component, op, outcome}` histogram (`microscaler-timing`), rendered by
//! the host's `/metrics` beside every other timed operation.
//!
//! | component | op | what |
//! |---|---|---|
//! | `lifeguard.pool` | `checkout` | waiting for a worker permit and an open slot (a statement) |
//! | `lifeguard.pool` | `checkout_rls` | pinning a slot for an RLS-scoped unit |
//! | `lifeguard.query` | `execute` / `query_opt` / `query_all` | one pooled statement, dispatch to reply |
//! | `lifeguard.query` | `rls_unit` | `BEGIN`, `rls_set_session`, the statement, `COMMIT` |
//! | `lifeguard.query` | `batch_unit` | a pipelined [`crate::StatementBatch`]: `BEGIN`, `rls_set_session`, N statements, `COMMIT` |
//! | `lifeguard.query` | `decode` | turning the returned rows into models (`FromRow`) |
//! | `lifeguard.cache` | `get_hit` / `get_miss` | the model cache read by `find_by_id` (any provider) |
//! | `lifeguard.cache` | `set` / `invalidate` | the Redis cache provider's writes |
//!
//! `outcome` is `timeout` for [`LifeError::PoolAcquireTimeout`] / [`LifeError::PoolReplyTimeout`]
//! (and `LifeError` implements [`microscaler_timing::IsTimeout`], so a service's own
//! `#[timed]` functions returning `Result<_, LifeError>` get the same split).
//!
//! Recording is a few atomic adds on statics: no lock, safe on `may` coroutines.

use microscaler_timing::{IsTimeout, Outcome, Site};

use crate::executor::LifeError;

pub(crate) static CHECKOUT: Site = Site::new("lifeguard.pool", "checkout");
pub(crate) static CHECKOUT_RLS: Site = Site::new("lifeguard.pool", "checkout_rls");
pub(crate) static EXECUTE: Site = Site::new("lifeguard.query", "execute");
pub(crate) static QUERY_OPT: Site = Site::new("lifeguard.query", "query_opt");
pub(crate) static QUERY_ALL: Site = Site::new("lifeguard.query", "query_all");
pub(crate) static RLS_UNIT: Site = Site::new("lifeguard.query", "rls_unit");
pub(crate) static BATCH_UNIT: Site = Site::new("lifeguard.query", "batch_unit");
pub(crate) static DECODE: Site = Site::new("lifeguard.query", "decode");
pub(crate) static CACHE_GET_HIT: Site = Site::new("lifeguard.cache", "get_hit");
pub(crate) static CACHE_GET_MISS: Site = Site::new("lifeguard.cache", "get_miss");
pub(crate) static CACHE_SET: Site = Site::new("lifeguard.cache", "set");
pub(crate) static CACHE_INVALIDATE: Site = Site::new("lifeguard.cache", "invalidate");

impl IsTimeout for LifeError {
    fn is_timeout(&self) -> bool {
        matches!(
            self,
            Self::PoolAcquireTimeout { .. } | Self::PoolReplyTimeout { .. }
        )
    }
}

/// The outcome label for a statement's result.
pub(crate) fn outcome<T>(result: &Result<T, LifeError>) -> Outcome {
    match result {
        Ok(_) => Outcome::Ok,
        Err(e) if e.is_timeout() => Outcome::Timeout,
        Err(_) => Outcome::Err,
    }
}

/// Decode `rows` with `decode`, timed as `lifeguard.query/decode`.
pub(crate) fn decode_rows<R, M>(
    rows: impl IntoIterator<Item = R>,
    capacity: usize,
    mut decode: impl FnMut(&R) -> Result<M, LifeError>,
) -> Result<Vec<M>, LifeError> {
    let timer = DECODE.start();
    let mut out = Vec::with_capacity(capacity);
    let mut result = Ok(());
    for row in rows {
        match decode(&row) {
            Ok(model) => out.push(model),
            Err(e) => {
                result = Err(e);
                break;
            }
        }
    }
    timer.finish(outcome(&result));
    result.map(|()| out)
}

/// Decode one row, timed as `lifeguard.query/decode`.
pub(crate) fn decode_one<M>(decode: impl FnOnce() -> Result<M, LifeError>) -> Result<M, LifeError> {
    let timer = DECODE.start();
    let model = decode();
    timer.finish(outcome(&model));
    model
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn count(op: &str, outcome: Outcome) -> u64 {
        microscaler_timing::snapshot()
            .into_iter()
            .filter(|s| s.component.starts_with("lifeguard.") && s.op == op && s.outcome == outcome)
            .map(|s| s.count())
            .sum()
    }

    #[test]
    fn pool_timeouts_are_timeouts() {
        let t = LifeError::PoolAcquireTimeout {
            waited: Duration::from_millis(5),
        };
        assert!(t.is_timeout());
        assert_eq!(outcome::<()>(&Err(t)), Outcome::Timeout);
        assert_eq!(
            outcome::<()>(&Err(LifeError::Other("x".into()))),
            Outcome::Err
        );
    }

    #[test]
    fn decode_rows_records_one_call_and_stops_at_the_first_error() {
        let before_ok = count("decode", Outcome::Ok);
        let before_err = count("decode", Outcome::Err);
        let ok = decode_rows(vec![1, 2, 3], 3, |r| Ok(r * 2)).unwrap();
        assert_eq!(ok, vec![2, 4, 6]);
        let mut seen = 0;
        let err = decode_rows(vec![1, 2, 3], 3, |r| {
            seen += 1;
            if *r == 2 {
                Err(LifeError::ParseError("bad".into()))
            } else {
                Ok(*r)
            }
        });
        assert!(err.is_err());
        assert_eq!(seen, 2);
        assert_eq!(count("decode", Outcome::Ok), before_ok + 1);
        assert_eq!(count("decode", Outcome::Err), before_err + 1);
    }
}
