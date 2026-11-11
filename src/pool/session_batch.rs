//! Pipelined statement batches: several independent statements in one round trip.
//!
//! A [`StatementBatch`] collects statements. [`LifeguardPool::with_session_batch`] (RLS-scoped)
//! and [`LifeguardPool::run_batch`] (no context) run them as **one unit** on one pinned primary
//! connection:
//!
//! ```text
//! BEGIN · [rls_set_session(..)] · stmt₁ · stmt₂ · … · stmtₙ · COMMIT
//! ```
//!
//! All messages go out in one write and every reply is read in order, so N statements cost one
//! round trip instead of N. Inside [`LifeguardPool::with_session_transaction`],
//! [`ExclusivePrimaryLifeExecutor::run_batch`] pipelines the statements the same way without its
//! own `BEGIN`/`COMMIT`.
//!
//! ## Semantics
//!
//! - **Atomic.** The statements share one transaction: either all of them commit or none do.
//! - **Independent.** A statement cannot use an earlier statement's result, because every
//!   statement is sent before any reply is read. Split dependent work into two units, or express
//!   it in SQL (a CTE).
//! - **Fail-closed RLS.** When the context cannot be set, no statement runs with a partial or
//!   missing context: PostgreSQL rejects every statement after the failed `rls_set_session` and
//!   the `COMMIT` turns into a rollback. The error is the context error.
//! - **First error wins.** After a statement fails, PostgreSQL rejects the rest with
//!   `25P02` (`in_failed_sql_transaction`). The batch returns the first real error, never a
//!   `25P02`, and nothing is committed.
//! - **The connection is clean on return.** Every reply is read before the connection goes back
//!   to the pool; the RLS settings are transaction-local, so they end with the transaction.
//!
//! ## Config switch
//!
//! `LIFEGUARD_RLS_PIPELINE=off` (or `0` / `false`) turns pipelining off for these APIs and for
//! [`LifeguardPool::with_session_transaction`]: the same calls then send one statement per round
//! trip, with the same results and the same atomicity. It exists to rule the pipeline out while
//! diagnosing a problem without a rebuild. The default is on.
//!
//! [`LifeguardPool::with_session_batch`]: crate::LifeguardPool::with_session_batch
//! [`LifeguardPool::run_batch`]: crate::LifeguardPool::run_batch
//! [`LifeguardPool::with_session_transaction`]: crate::LifeguardPool::with_session_transaction
//! [`ExclusivePrimaryLifeExecutor::run_batch`]: crate::ExclusivePrimaryLifeExecutor::run_batch

use super::pooled::{Kind, Out};
use crate::executor::LifeError;
use crate::pool::owned_param::OwnedParam;
use may_postgres::Row;
use std::marker::PhantomData;
use std::sync::Arc;

/// One statement of a batch, with owned parameters.
pub(crate) struct BatchStmt {
    pub(crate) kind: Kind,
    pub(crate) sql: Arc<str>,
    pub(crate) params: Vec<OwnedParam>,
}

/// Independent statements to run as one pipelined unit. See the [module docs](self).
///
/// ```no_run
/// use lifeguard::{LifeguardPool, SessionContext, StatementBatch};
/// use sea_query::{Value, Values};
/// # fn f(pool: &LifeguardPool, ctx: &SessionContext) -> Result<(), lifeguard::LifeError> {
/// let mut batch = StatementBatch::new();
/// let open = batch.query_opt("SELECT count(*) FROM orders WHERE status = $1",
///     &Values(vec![Value::from("open")]))?;
/// let limits = batch.query_all("SELECT key, value FROM risk_limits", &Values(vec![]))?;
/// let mut out = pool.with_session_batch(ctx, batch)?;
/// let open: Option<lifeguard::may_postgres::Row> = out.take(open);
/// let limits = out.take(limits);
/// # let _ = (open, limits); Ok(()) }
/// ```
#[derive(Default)]
pub struct StatementBatch {
    pub(crate) stmts: Vec<BatchStmt>,
}

/// Handle to one statement's result in a [`BatchResults`]; `T` is the result type.
#[must_use = "a batch handle is the only way to read that statement's result"]
pub struct BatchSlot<T> {
    index: usize,
    _t: PhantomData<fn() -> T>,
}

impl<T> Clone for BatchSlot<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for BatchSlot<T> {}

impl<T> BatchSlot<T> {
    /// Position of the statement in its batch (0-based).
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }
}

impl StatementBatch {
    /// An empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Statements queued so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.stmts.len()
    }

    /// `true` when no statement is queued.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stmts.is_empty()
    }

    fn push<T>(
        &mut self,
        kind: Kind,
        sql: &str,
        values: &sea_query::Values,
    ) -> Result<BatchSlot<T>, LifeError> {
        let params = values
            .0
            .iter()
            .map(OwnedParam::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.push_owned(kind, sql, params))
    }

    fn push_owned<T>(&mut self, kind: Kind, sql: &str, params: Vec<OwnedParam>) -> BatchSlot<T> {
        let index = self.stmts.len();
        self.stmts.push(BatchStmt {
            kind,
            sql: Arc::from(sql),
            params,
        });
        BatchSlot {
            index,
            _t: PhantomData,
        }
    }

    /// Queue a write; its result is the affected-row count.
    ///
    /// # Errors
    ///
    /// A parameter that cannot be bound (see [`OwnedParam`]).
    pub fn execute(
        &mut self,
        sql: &str,
        values: &sea_query::Values,
    ) -> Result<BatchSlot<u64>, LifeError> {
        self.push(Kind::Execute, sql, values)
    }

    /// Queue a statement returning zero or one row.
    ///
    /// # Errors
    ///
    /// A parameter that cannot be bound. More than one returned row is an error when the batch
    /// runs.
    pub fn query_opt(
        &mut self,
        sql: &str,
        values: &sea_query::Values,
    ) -> Result<BatchSlot<Option<Row>>, LifeError> {
        self.push(Kind::QueryOpt, sql, values)
    }

    /// Queue a statement returning any number of rows.
    ///
    /// # Errors
    ///
    /// A parameter that cannot be bound.
    pub fn query_all(
        &mut self,
        sql: &str,
        values: &sea_query::Values,
    ) -> Result<BatchSlot<Vec<Row>>, LifeError> {
        self.push(Kind::QueryAll, sql, values)
    }

    /// Queue a write with already-owned parameters.
    pub fn execute_owned(&mut self, sql: &str, params: Vec<OwnedParam>) -> BatchSlot<u64> {
        self.push_owned(Kind::Execute, sql, params)
    }

    /// Queue a statement returning rows, with already-owned parameters.
    pub fn query_all_owned(&mut self, sql: &str, params: Vec<OwnedParam>) -> BatchSlot<Vec<Row>> {
        self.push_owned(Kind::QueryAll, sql, params)
    }
}

/// Results of a batch, in statement order. Read each with [`BatchResults::take`].
pub struct BatchResults {
    outs: Vec<Option<Out>>,
}

impl BatchResults {
    pub(crate) fn new(outs: Vec<Out>) -> Self {
        Self {
            outs: outs.into_iter().map(Some).collect(),
        }
    }

    /// Statements in the batch.
    #[must_use]
    pub fn len(&self) -> usize {
        self.outs.len()
    }

    /// `true` for an empty batch.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.outs.is_empty()
    }

    /// Take one statement's result.
    ///
    /// # Panics
    ///
    /// When the slot came from another batch, or was taken already.
    // A slot is typed by the batch that issued it: a miss is a caller bug, like an index out of
    // bounds, not a runtime condition.
    #[allow(clippy::expect_used)]
    pub fn take<T: BatchOutput>(&mut self, slot: BatchSlot<T>) -> T {
        let out = self
            .outs
            .get_mut(slot.index)
            .and_then(Option::take)
            .expect("BatchResults::take: slot from another batch, or taken twice");
        T::from_out(out).expect("BatchResults::take: result kind mismatch")
    }
}

/// Result types a [`BatchSlot`] can carry: `u64`, `Option<Row>`, `Vec<Row>`.
pub trait BatchOutput: Sized + sealed::Sealed {
    #[doc(hidden)]
    fn from_out(out: Out) -> Option<Self>;
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for u64 {}
    impl Sealed for Option<may_postgres::Row> {}
    impl Sealed for Vec<may_postgres::Row> {}
}

impl BatchOutput for u64 {
    fn from_out(out: Out) -> Option<Self> {
        match out {
            Out::Count(n) => Some(n),
            _ => None,
        }
    }
}

impl BatchOutput for Option<Row> {
    fn from_out(out: Out) -> Option<Self> {
        match out {
            Out::Opt(r) => Some(r),
            _ => None,
        }
    }
}

impl BatchOutput for Vec<Row> {
    fn from_out(out: Out) -> Option<Self> {
        match out {
            Out::All(r) => Some(r),
            _ => None,
        }
    }
}

/// Process-wide override of `LIFEGUARD_RLS_PIPELINE`: 0 = follow the env, 1 = on, 2 = off.
static OVERRIDE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Force pipelining on or off for this process (`None` follows `LIFEGUARD_RLS_PIPELINE`).
///
/// For tests and benchmarks that compare both paths in one process; services use the env var.
#[doc(hidden)]
pub fn set_pipeline_override(on: Option<bool>) {
    let v = match on {
        None => 0,
        Some(true) => 1,
        Some(false) => 2,
    };
    OVERRIDE.store(v, std::sync::atomic::Ordering::Relaxed);
}

/// `LIFEGUARD_RLS_PIPELINE`: pipelined batches and session transactions (default on).
pub(crate) fn pipeline_enabled() -> bool {
    match OVERRIDE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => return true,
        2 => return false,
        _ => {}
    }
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("LIFEGUARD_RLS_PIPELINE").map_or(true, |v| {
            !matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "off" | "0" | "false" | "no"
            )
        })
    })
}
