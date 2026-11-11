//! Batch operations: multi-row writes, batched reads by key, and a buffering writer.
//!
//! Single-row ORM calls cost one statement (one round trip) per row. These APIs write or read
//! many rows per statement while keeping the ORM's semantics: `before_insert` / `before_update`
//! hooks and validators run per record, `save_as` expressions apply, RLS context and replica
//! routing come from the executor, and results map through the entity's `FromRow`.
//!
//! | API | Statement |
//! |---|---|
//! | [`insert_many`] | `INSERT INTO t (..) VALUES (..), (..) [RETURNING *]`, or `SELECT * FROM unnest($1::int8[], ..)` for large batches |
//! | [`upsert_many`] | the same with `ON CONFLICT (..) DO UPDATE / DO NOTHING` |
//! | [`update_many`] | `UPDATE t SET c = v.c .. FROM unnest(..) AS v(..) WHERE t.pk = v.pk` |
//! | [`delete_many`] | `DELETE FROM t WHERE pk = ANY($1)` |
//! | [`find_by_ids`] | `SELECT .. FROM t WHERE pk = ANY($1)` |
//! | [`BatchWriter`] | a buffer that coroutines push rows into; a flusher writes them with [`insert_many`] / [`upsert_many`] |
//!
//! ## Chunking and statement reuse
//!
//! PostgreSQL accepts at most 65,535 parameters per statement. A `VALUES` batch uses one per
//! value, so rows are split into chunks of at most [`BatchOptions::max_rows`] rows and
//! [`BatchOptions::max_params`] parameters. Full chunks have identical SQL, so each chunk size is
//! prepared once per connection and then served from the statement cache. The `unnest` form uses
//! one array parameter per column whatever the row count, so its SQL is constant per column set:
//! it is chosen when a chunk has at least [`BatchOptions::unnest_min_rows`] rows and every value
//! is a plain value (no `save_as` expression).
//!
//! ## Errors
//!
//! Chunks run in order, each as its own statement. By default ([`BatchFailure::Strict`]) a
//! failure returns [`BatchError`] naming the chunk and its row range, with the rows written by
//! earlier chunks.
//!
//! ## Partial success (opt-in)
//!
//! Lifeguard cannot know whether a batch's rows are independent, so splitting a failed chunk is
//! never the default. A caller (per call: [`BatchOptions::failure`]) or an entity (per entity:
//! `#[batch_failure = "bisect"]` on the model) opts into [`BatchFailure::Bisect`]: a chunk that
//! fails on bad data is halved until the bad rows stand alone; they come back in
//! [`BatchOutcome::failed`] and every other row is written. The per-call setting wins over the
//! entity's. Configuration (`LIFEGUARD_BATCH_CHUNK_SIZE`, `LIFEGUARD_BATCH_MAX_DEPTH`,
//! `LIFEGUARD_BATCH_TRANSIENT_RETRIES`) sets defaults for chunk size, bisect depth and transient
//! retries only; there is deliberately no global switch that turns Bisect on.
//!
//! Bisect applies to these homogeneous row batches ([`insert_many`], [`upsert_many`],
//! [`update_many`], [`delete_many`], [`BatchWriter`]) only. A multi-statement unit
//! ([`crate::StatementBatch`], [`crate::LifeguardPool::with_session_transaction`]) is always
//! atomic: one failure fails the unit.
//!
//! Even when requested, Bisect falls back to Strict (and says why in
//! [`BatchOutcome::fallback_reason`]) when the batch shows it is not safe to split: duplicate
//! keys, rows that reference other rows of the batch through a self-referencing foreign key, or
//! [`BatchOptions::ordered`]. Run the call inside
//! [`crate::LifeguardPool::with_session_transaction`] (or a `Transaction`) to make the whole batch
//! atomic.

pub mod writer;

pub use writer::{BatchWriter, BatchWriterConfig, BatchWriterStats, WriteMode};

use crate::active_model::{ActiveModelBehavior, ActiveModelError, ActiveModelTrait};
use crate::executor::{LifeError, LifeExecutor};
use crate::pool::{OwnedArray, OwnedParam};
use crate::query::column::column_trait::ColumnDefHelper;
use crate::query::traits::{FromRow, LifeEntityName, LifeModelTrait};
use sea_query::{Expr, Iden, OnConflict, PostgresQueryBuilder, Query, Value};
use std::fmt;

/// PostgreSQL's limit on bind parameters in one statement.
pub const POSTGRES_MAX_PARAMS: usize = 65_535;

/// Tuning for the batch APIs.
#[derive(Debug, Clone)]
pub struct BatchOptions {
    /// Rows per statement (chunk), at most.
    pub max_rows: usize,
    /// Bind parameters per statement, at most (capped at [`POSTGRES_MAX_PARAMS`]).
    pub max_params: usize,
    /// Use `unnest(..)` arrays instead of `VALUES` when a chunk has at least this many rows
    /// (`usize::MAX` disables `unnest`).
    pub unnest_min_rows: usize,
    /// Add `RETURNING *` and map the written rows back to models.
    pub returning: bool,
    /// The failure policy for this call. `None` (the default) uses the entity's declared policy
    /// (`#[batch_failure = ".."]`), and [`BatchFailure::Strict`] when it declares none. A value
    /// here wins over the entity's.
    pub failure: Option<BatchFailure>,
    /// Under [`BatchFailure::Bisect`]: re-runs of a chunk after a transient error (serialization
    /// failure, deadlock, dropped connection) before giving up on it. Strict never re-runs a
    /// statement. Default `LIFEGUARD_BATCH_TRANSIENT_RETRIES`, else 1.
    pub transient_retries: u32,
    /// The caller needs each chunk all-or-nothing in input order: Bisect is not applied (it falls
    /// back to Strict with a reason).
    pub ordered: bool,
}

/// Process-wide defaults for the batch APIs, from the environment (read once).
///
/// Only sizes and limits are configurable here. Whether a batch may be split is not: that is
/// declared by the caller ([`BatchOptions::failure`]) or the entity (`#[batch_failure]`).
#[derive(Debug, Clone, Copy)]
pub struct BatchDefaults {
    /// Rows per chunk (`LIFEGUARD_BATCH_CHUNK_SIZE`, default 1000).
    pub chunk_size: usize,
    /// Halvings before Bisect reports the rest of a failing part as failed
    /// (`LIFEGUARD_BATCH_MAX_DEPTH`, default 20).
    pub max_depth: u32,
    /// Transient re-runs under Bisect (`LIFEGUARD_BATCH_TRANSIENT_RETRIES`, default 1).
    pub transient_retries: u32,
}

/// The process-wide [`BatchDefaults`].
#[must_use]
pub fn defaults() -> BatchDefaults {
    static D: std::sync::OnceLock<BatchDefaults> = std::sync::OnceLock::new();
    *D.get_or_init(|| {
        fn env<T: std::str::FromStr>(name: &str, default: T) -> T {
            std::env::var(name)
                .ok()
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(default)
        }
        BatchDefaults {
            chunk_size: env("LIFEGUARD_BATCH_CHUNK_SIZE", 1000_usize).max(1),
            max_depth: env("LIFEGUARD_BATCH_MAX_DEPTH", 20_u32),
            transient_retries: env("LIFEGUARD_BATCH_TRANSIENT_RETRIES", 1_u32),
        }
    })
}

/// What a homogeneous row batch ([`insert_many`], [`upsert_many`], [`update_many`],
/// [`delete_many`], [`BatchWriter`]) does when a chunk's statement fails.
///
/// Each chunk is one statement, so a failed chunk writes nothing.
///
/// ## When Bisect is safe
///
/// Only when the rows are **independent**: each row may be written without the others, and
/// writing some but not all of them leaves the data correct. Typical: idempotent ingest of
/// separate facts (prices, documents, events keyed by their own id). Not safe: rows that only
/// make sense together (an order and its lines), rows whose order matters, or a batch inside a
/// transaction that must commit all of it.
///
/// ## What partial success means
///
/// The call returns `Ok` with [`BatchOutcome::failed`] listing the rows that were not written
/// (input index, SQLSTATE, error); every other row is committed. [`BatchOutcome::is_complete`]
/// is `false` then. The caller logs, retries or dead-letters the failed rows.
///
/// ## How to opt in
///
/// - Per call: `BatchOptions { failure: Some(BatchFailure::bisect()), ..Default::default() }`
///   (or [`BatchOptions::bisect`]).
/// - Per entity: `#[batch_failure = "bisect"]` on the model, which declares its rows independent.
///   A per-call `failure` overrides it either way.
///
/// There is no global switch: only the caller or the entity knows whether rows are independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BatchFailure {
    /// The call stops at the failing chunk and returns [`BatchError`] for all of its rows.
    /// Earlier chunks stay written. The default.
    #[default]
    Strict,
    /// Find the bad rows and write the rest.
    ///
    /// - A **row error** (SQLSTATE class `22` data exception, class `23` constraint violation)
    ///   splits the failed part in half and retries each half, recursively, down to single rows
    ///   or `max_depth` halvings. Rows of a part that still fails go to
    ///   [`BatchOutcome::failed`]; every other row is written. That costs about `k · log2(n)`
    ///   extra statements for `k` bad rows in a chunk of `n`, against `n` for row by row.
    /// - A **transient error** (`40001`, `40P01`, a pool acquire timeout, a connection that failed
    ///   before the part was sent) re-runs the whole part
    ///   up to [`BatchOptions::transient_retries`] times, then returns [`BatchError`] without
    ///   splitting: halving would not help. A dropped connection (class `08`, I/O error) or a
    ///   reply timeout is returned at once: the part may already have been applied. (The pool
    ///   retries a part once by itself when its connection failed before it was sent.)
    /// - **Any other error** returns [`BatchError`] at once.
    ///
    /// Each part is its own statement, so on a pool executor (autocommit) a failed part rolls back
    /// alone. Inside a transaction the first error aborts it and every retry fails with `25P02`:
    /// the call then returns that error.
    Bisect {
        /// Halvings before giving up on a failing part (and reporting its rows as failed).
        max_depth: u32,
    },
}

impl BatchFailure {
    /// [`BatchFailure::Bisect`] with the configured default depth.
    #[must_use]
    pub fn bisect() -> Self {
        Self::Bisect {
            max_depth: defaults().max_depth,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Bisect { .. } => "bisect",
        }
    }
}

/// A row that [`BatchFailure::Bisect`] could not write.
#[derive(Debug)]
pub struct FailedRow {
    /// Index of the row in the input slice.
    pub index: usize,
    /// The PostgreSQL SQLSTATE, e.g. `23505` (unique violation).
    pub sqlstate: Option<String>,
    /// The error for this row (or for the smallest part containing it, at `max_depth`).
    pub error: ActiveModelError,
}

/// A chunk's failure: from the database (classified for [`BatchFailure::Bisect`]) or not.
enum ChunkError {
    Db(LifeError),
    Other(ActiveModelError),
}

/// Why a batch statement failed, as far as a retry is concerned. See
/// [`BatchError::kind`] and the [`super::writer`] retry rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FailureKind {
    /// The connection failed before the statement reached the server
    /// ([`LifeError::NotSent`]): running it again cannot write twice.
    NotSent,
    /// The server rejected the statement and rolled it back (`40001` serialization failure,
    /// `40P01` deadlock), or no worker took it (pool acquire timeout): safe to run again.
    Transient,
    /// The connection was lost after the statement was sent: it may or may not have been applied.
    ConnectionLost,
    /// No reply within the pool's reply deadline: the statement may still have been applied.
    ReplyTimeout,
    /// Bad data in a row (SQLSTATE class `22` data exception, `23` integrity constraint).
    Data,
    /// Anything else (syntax, permissions, a record that failed validation before any write).
    Other,
}

impl FailureKind {
    /// Classify a statement's error.
    #[must_use]
    pub fn of(e: &LifeError) -> Self {
        match e {
            LifeError::NotSent(_) => Self::NotSent,
            LifeError::PoolAcquireTimeout { .. } => Self::Transient,
            LifeError::PoolReplyTimeout { .. } => Self::ReplyTimeout,
            LifeError::PostgresError(pe) => {
                match pe.code().map(may_postgres::error::SqlState::code) {
                    Some("40001" | "40P01") => Self::Transient,
                    Some(c) if c.starts_with("22") || c.starts_with("23") => Self::Data,
                    _ if crate::pool::connectivity::life_error_is_connectivity_heal_candidate(
                        e,
                    ) =>
                    {
                        Self::ConnectionLost
                    }
                    // No SQLSTATE and not a recognised connectivity error: an I/O failure on a
                    // connection that had the statement, so treated as lost after send.
                    None => Self::ConnectionLost,
                    Some(_) => Self::Other,
                }
            }
            _ => Self::Other,
        }
    }

    /// `true` when running the statement again cannot apply it twice.
    #[must_use]
    pub const fn safe_to_retry(self) -> bool {
        matches!(self, Self::NotSent | Self::Transient)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorClass {
    /// Bad data in some row: class 22 (data exception) or 23 (integrity constraint).
    RowData,
    /// Worth a re-run of the same statement: 40001, 40P01, or a pool acquire timeout.
    Transient,
    Other,
}

fn sqlstate(e: &LifeError) -> Option<String> {
    match e {
        LifeError::PostgresError(pe) => pe.code().map(|c| c.code().to_string()),
        _ => None,
    }
}

fn classify(e: &LifeError) -> ErrorClass {
    // A connection lost after send or a reply timeout is not re-run: the part may have been
    // applied.
    match FailureKind::of(e) {
        FailureKind::Data => ErrorClass::RowData,
        k if k.safe_to_retry() => ErrorClass::Transient,
        _ => ErrorClass::Other,
    }
}

/// The policy a call runs under, after the per-call / entity / default resolution and the
/// safety checks.
struct Plan {
    bisect: Option<u32>,
    transient_retries: u32,
    fallback: Option<String>,
}

impl Plan {
    fn resolve<E: LifeModelTrait>(
        opts: &BatchOptions,
        keys: Option<&[String]>,
        refs_in_batch: bool,
    ) -> Self {
        let requested = opts
            .failure
            .or_else(E::__lg_batch_failure)
            .unwrap_or_default();
        let mut plan = Self {
            bisect: None,
            transient_retries: opts.transient_retries,
            fallback: None,
        };
        if let BatchFailure::Bisect { max_depth } = requested {
            let reason = if opts.ordered {
                Some("BatchOptions::ordered: chunks must stay whole and in order")
            } else if keys.is_some_and(has_duplicates) {
                Some("duplicate keys in the batch: splitting could change which row wins")
            } else if refs_in_batch {
                Some("rows reference other rows of the same batch (self-referencing foreign key)")
            } else {
                None
            };
            match reason {
                Some(r) => plan.fallback = Some(r.to_string()),
                None => plan.bisect = Some(max_depth),
            }
        }
        plan
    }

    fn applied(&self) -> BatchFailure {
        match self.bisect {
            Some(max_depth) => BatchFailure::Bisect { max_depth },
            None => BatchFailure::Strict,
        }
    }

    fn record<M>(
        &self,
        metrics: Option<&'static crate::metrics::entity::EntityMetrics>,
        out: &BatchOutcome<M>,
        extra_statements: u64,
    ) {
        if let Some(m) = metrics {
            m.record_batch(
                self.applied().label(),
                out.rows_affected,
                out.failed.len() as u64,
                extra_statements,
                self.fallback.is_some(),
            );
        }
    }
}

fn has_duplicates(keys: &[String]) -> bool {
    let mut seen = std::collections::HashSet::with_capacity(keys.len());
    keys.iter().any(|k| !seen.insert(k))
}

/// `true` when a column's foreign key targets the entity's own table.
fn references_own_table<E>(c: E::Column) -> bool
where
    E: LifeModelTrait + LifeEntityName + Default,
{
    let Some(fk) = c.column_def().foreign_key else {
        return false;
    };
    fk_targets_table(&fk, E::default().table_name())
}

/// `fk` is a column's `foreign_key` text, e.g. `accounts(id) ON DELETE SET NULL` or
/// `ledger.accounts (id)`.
fn fk_targets_table(fk: &str, table: &str) -> bool {
    let target = fk.split('(').next().unwrap_or("").trim().trim_matches('"');
    let target = target
        .rsplit('.')
        .next()
        .unwrap_or(target)
        .trim_matches('"');
    target.eq_ignore_ascii_case(table)
}

/// Whether any self-referencing column value of a row equals a key in the batch.
fn refs_within<T>(
    keys: Option<&[String]>,
    self_ref_cols: &[usize],
    rows: &[Vec<T>],
    render: impl Fn(&T) -> String,
) -> bool {
    let Some(keys) = keys else {
        return false;
    };
    if self_ref_cols.is_empty() {
        return false;
    }
    let set: std::collections::HashSet<&String> = keys.iter().collect();
    rows.iter().any(|r| {
        self_ref_cols
            .iter()
            .any(|&c| r.get(c).is_some_and(|v| set.contains(&render(v))))
    })
}

/// Run one chunk's statement: rows affected and (with RETURNING) the models.
fn exec_chunk<M: FromRow>(
    executor: &dyn LifeExecutor,
    metrics: Option<&'static crate::metrics::entity::EntityMetrics>,
    op: crate::metrics::entity::Op,
    sql: &str,
    params: Vec<OwnedParam>,
    returning: bool,
) -> Result<(u64, Vec<M>), ChunkError> {
    let timer = crate::metrics::entity::OpTimer::start(metrics, op);
    if returning {
        let res = executor.__lg_query_all_owned(sql, params);
        timer.finish(res.as_ref().map_or(0, |r| r.len() as u64), res.is_ok());
        let rows_back = res.map_err(ChunkError::Db)?;
        let mut models = Vec::with_capacity(rows_back.len());
        for row in &rows_back {
            models.push(
                M::from_row(row).map_err(|e| {
                    ChunkError::Other(ActiveModelError::DatabaseError(e.to_string()))
                })?,
            );
        }
        Ok((rows_back.len() as u64, models))
    } else {
        let res = executor.__lg_execute_owned(sql, params);
        timer.finish(*res.as_ref().unwrap_or(&0), res.is_ok());
        Ok((res.map_err(ChunkError::Db)?, Vec::new()))
    }
}

/// Run every chunk under `plan`. Returns the result and the statements run beyond one per chunk
/// (bisect halves and transient re-runs: the extra round trips the policy cost).
fn drive<M>(
    plan: &Plan,
    chunks: Vec<std::ops::Range<usize>>,
    out: &mut BatchOutcome<M>,
    mut run: impl FnMut(std::ops::Range<usize>) -> Result<(u64, Vec<M>), ChunkError>,
) -> (Result<(), BatchError>, u64) {
    let mut extra = 0u64;
    for (chunk_index, range) in chunks.into_iter().enumerate() {
        // Parts still to write, last on top: in order, so RETURNING models keep input order.
        let mut todo = vec![(range, 0u32)];
        let mut first = true;
        while let Some((part, depth)) = todo.pop() {
            let fail = |source: ActiveModelError, written: u64, kind: FailureKind| BatchError {
                chunk: chunk_index,
                rows: part.clone(),
                rows_written_before: written,
                source,
                kind,
            };
            out.statements += 1;
            if !first {
                extra += 1;
            }
            first = false;
            let mut res = run(part.clone());
            if plan.bisect.is_some() {
                let mut tries = 0;
                while tries < plan.transient_retries
                    && matches!(&res, Err(ChunkError::Db(e)) if classify(e) == ErrorClass::Transient)
                {
                    tries += 1;
                    out.statements += 1;
                    extra += 1;
                    res = run(part.clone());
                }
            }
            match res {
                Ok((n, models)) => {
                    out.rows_affected += n;
                    out.models.extend(models);
                }
                Err(ChunkError::Db(e))
                    if plan.bisect.is_some() && classify(&e) == ErrorClass::RowData =>
                {
                    let max_depth = plan.bisect.unwrap_or(0);
                    if part.len() == 1 || depth >= max_depth {
                        for index in part.clone() {
                            out.failed.push(FailedRow {
                                index,
                                sqlstate: sqlstate(&e),
                                error: db(&e),
                            });
                        }
                    } else {
                        let mid = part.start + part.len() / 2;
                        todo.push((mid..part.end, depth + 1));
                        todo.push((part.start..mid, depth + 1));
                    }
                }
                Err(ChunkError::Db(e)) => {
                    return (
                        Err(fail(db(&e), out.rows_affected, FailureKind::of(&e))),
                        extra,
                    )
                }
                Err(ChunkError::Other(e)) => {
                    return (Err(fail(e, out.rows_affected, FailureKind::Other)), extra)
                }
            }
        }
    }
    out.failed.sort_by_key(|f| f.index);
    (Ok(()), extra)
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            max_rows: defaults().chunk_size,
            max_params: 32_000,
            unnest_min_rows: 64,
            returning: false,
            failure: None,
            transient_retries: defaults().transient_retries,
            ordered: false,
        }
    }
}

impl BatchOptions {
    /// Default options that opt into [`BatchFailure::Bisect`] for this call (the caller declares
    /// the rows independent). See [`BatchFailure`] for when that is safe.
    #[must_use]
    pub fn bisect() -> Self {
        Self {
            failure: Some(BatchFailure::bisect()),
            ..Self::default()
        }
    }

    /// Default options with `RETURNING *`.
    #[must_use]
    pub fn returning() -> Self {
        Self {
            returning: true,
            ..Self::default()
        }
    }
}

/// What a batch call wrote or read.
#[derive(Debug)]
pub struct BatchOutcome<M> {
    /// Rows the statements reported as affected (inserted, updated or deleted).
    pub rows_affected: u64,
    /// Rows returned (`RETURNING *`, or the rows found by [`find_by_ids`]).
    pub models: Vec<M>,
    /// Statements run (with [`BatchFailure::Bisect`], retries and halves included: the round
    /// trips the call cost).
    pub statements: usize,
    /// Rows [`BatchFailure::Bisect`] could not write, in input order (always empty with
    /// [`BatchFailure::Strict`]). Non-empty means **partial success**: see
    /// [`BatchOutcome::is_complete`].
    pub failed: Vec<FailedRow>,
    /// The policy the call actually ran under (after any fallback).
    pub policy_applied: BatchFailure,
    /// Why a requested [`BatchFailure::Bisect`] ran as [`BatchFailure::Strict`], if it did.
    pub fallback_reason: Option<String>,
}

impl<M> BatchOutcome<M> {
    /// `true` when every input row was written: no row failed. A [`BatchFailure::Bisect`] call
    /// can return `Ok` with failed rows; check this (or [`BatchOutcome::failed`]) before
    /// treating the call as complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.failed.is_empty()
    }
}

impl<M> Default for BatchOutcome<M> {
    fn default() -> Self {
        Self {
            rows_affected: 0,
            models: Vec::new(),
            statements: 0,
            failed: Vec::new(),
            policy_applied: BatchFailure::Strict,
            fallback_reason: None,
        }
    }
}

/// A batch call failed in one chunk; earlier chunks were written.
#[derive(Debug)]
pub struct BatchError {
    /// Zero-based index of the failing chunk.
    pub chunk: usize,
    /// Input rows `rows.start..rows.end` made up the failing chunk.
    pub rows: std::ops::Range<usize>,
    /// Rows affected by the chunks before it.
    pub rows_written_before: u64,
    /// The underlying error.
    pub source: ActiveModelError,
    /// What kind of failure it was: whether running the failed rows again could write twice.
    pub kind: FailureKind,
}

/// For use inside closures that return [`LifeError`], e.g.
/// [`crate::LifeguardPool::with_session_transaction`]. Chunk details move into the message.
impl From<BatchError> for LifeError {
    fn from(e: BatchError) -> Self {
        Self::QueryError(e.to_string())
    }
}

impl fmt::Display for BatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "batch chunk {} (rows {}..{}) failed after {} rows were written: {}",
            self.chunk, self.rows.start, self.rows.end, self.rows_written_before, self.source
        )
    }
}

impl std::error::Error for BatchError {}

impl BatchError {
    fn before_any(source: ActiveModelError) -> Self {
        Self {
            chunk: 0,
            rows: 0..0,
            rows_written_before: 0,
            source,
            kind: FailureKind::Other,
        }
    }
}

fn db(e: &LifeError) -> ActiveModelError {
    ActiveModelError::DatabaseError(e.to_string())
}

// ---------------------------------------------------------------------------------------------
// Helpers

fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn table_sql<E: LifeEntityName>(entity: &E) -> String {
    match entity.schema_name() {
        Some(schema) => format!(
            "{}.{}",
            quote_ident(schema),
            quote_ident(entity.table_name())
        ),
        None => quote_ident(entity.table_name()),
    }
}

fn column_name<C: Iden>(c: &C) -> String {
    c.to_string()
}

/// The entity's single primary-key column.
fn primary_key_column<E: LifeModelTrait>() -> Result<E::Column, ActiveModelError>
where
    E::Column: Copy,
{
    let pks: Vec<E::Column> = E::all_columns()
        .iter()
        .copied()
        .filter(|c| <E::Column as ColumnDefHelper>::column_def(*c).primary_key)
        .collect();
    match pks.as_slice() {
        [one] => Ok(*one),
        [] => Err(ActiveModelError::Other(
            "batch by key needs an entity with a #[primary_key] column".to_string(),
        )),
        _ => Err(ActiveModelError::Other(
            "batch by key supports single-column primary keys only".to_string(),
        )),
    }
}

fn is_json_column<E: LifeModelTrait>(c: E::Column) -> bool {
    <E::Column as ColumnDefHelper>::column_def(c)
        .column_type
        .is_some_and(|t| t.to_ascii_uppercase().contains("JSON"))
}

/// Typed arrays for `unnest`, one per column, or `None` when a column cannot be expressed as an
/// array (expression values, mixed or unsupported types, all-NULL columns).
fn columns_as_arrays(rows: &[Vec<Expr>], ncols: usize) -> Option<Vec<(OwnedArray, &'static str)>> {
    let mut out = Vec::with_capacity(ncols);
    for col in 0..ncols {
        let first = rows.iter().find_map(|r| match &r[col] {
            Expr::Value(v) if !value_is_null(v) => Some(v),
            _ => None,
        })?;
        let (mut arr, ty) = OwnedArray::for_value(first)?;
        for r in rows {
            match &r[col] {
                Expr::Value(v) => {
                    if !arr.push(v) {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        out.push((arr, ty));
    }
    Some(out)
}

fn value_is_null(v: &Value) -> bool {
    matches!(
        v,
        Value::Bool(None)
            | Value::TinyInt(None)
            | Value::SmallInt(None)
            | Value::Int(None)
            | Value::BigInt(None)
            | Value::TinyUnsigned(None)
            | Value::SmallUnsigned(None)
            | Value::Unsigned(None)
            | Value::BigUnsigned(None)
            | Value::Float(None)
            | Value::Double(None)
            | Value::String(None)
            | Value::Char(None)
            | Value::Bytes(None)
            | Value::Json(None)
            | Value::ChronoDate(None)
            | Value::ChronoTime(None)
            | Value::ChronoDateTime(None)
            | Value::ChronoDateTimeUtc(None)
            | Value::ChronoDateTimeLocal(None)
            | Value::Uuid(None)
            | Value::Decimal(None)
    )
}

fn values_to_owned(values: &sea_query::Values) -> Result<Vec<OwnedParam>, ActiveModelError> {
    values
        .0
        .iter()
        .map(OwnedParam::try_from)
        .collect::<Result<_, _>>()
        .map_err(|e| db(&e))
}

/// Split `n` rows of `ncols` values into chunk ranges under the row and parameter caps.
fn chunk_ranges(n: usize, ncols: usize, opts: &BatchOptions) -> Vec<std::ops::Range<usize>> {
    let max_params = opts.max_params.clamp(1, POSTGRES_MAX_PARAMS);
    let by_params = (max_params / ncols.max(1)).max(1);
    let size = opts.max_rows.max(1).min(by_params);
    (0..n).step_by(size).map(|s| s..(s + size).min(n)).collect()
}

// ---------------------------------------------------------------------------------------------
// insert_many / upsert_many

/// Insert many records, chunked, optionally returning the written rows.
///
/// Every record runs its `before_insert` hook and validators first, exactly as
/// [`ActiveModelTrait::insert`] does. Records in one call must set the same columns (the batch
/// statement has one column list); mixing column sets is an error.
///
/// # Errors
///
/// [`BatchError`] for the first failing chunk (or chunk 0 when a record fails its hooks).
pub fn insert_many<R>(
    executor: &dyn LifeExecutor,
    records: &[R],
    opts: &BatchOptions,
) -> Result<BatchOutcome<<R::Entity as LifeModelTrait>::Model>, BatchError>
where
    R: ActiveModelTrait + ActiveModelBehavior,
    R::Entity: LifeEntityName + Default + Iden + Clone + 'static,
    <R::Entity as LifeModelTrait>::Column: Copy + Iden + 'static,
    <R::Entity as LifeModelTrait>::Model: FromRow,
{
    write_many(executor, records, None, opts)
}

/// Insert many records with an `ON CONFLICT` clause (idempotent ingest).
///
/// Use `OnConflict::columns([..]).update_columns([..])` for last-write-wins upserts, or
/// `.do_nothing()` to skip existing keys. PostgreSQL rejects a statement that would update the
/// same row twice ("ON CONFLICT DO UPDATE command cannot affect row a second time"), so callers
/// that may repeat a key within one call should de-duplicate first ([`BatchWriter`] does).
///
/// # Errors
///
/// [`BatchError`] for the first failing chunk.
pub fn upsert_many<R>(
    executor: &dyn LifeExecutor,
    records: &[R],
    on_conflict: OnConflict,
    opts: &BatchOptions,
) -> Result<BatchOutcome<<R::Entity as LifeModelTrait>::Model>, BatchError>
where
    R: ActiveModelTrait + ActiveModelBehavior,
    R::Entity: LifeEntityName + Default + Iden + Clone + 'static,
    <R::Entity as LifeModelTrait>::Column: Copy + Iden + 'static,
    <R::Entity as LifeModelTrait>::Model: FromRow,
{
    write_many(executor, records, Some(on_conflict), opts)
}

#[allow(clippy::too_many_lines)]
fn write_many<R>(
    executor: &dyn LifeExecutor,
    records: &[R],
    on_conflict: Option<OnConflict>,
    opts: &BatchOptions,
) -> Result<BatchOutcome<<R::Entity as LifeModelTrait>::Model>, BatchError>
where
    R: ActiveModelTrait + ActiveModelBehavior,
    R::Entity: LifeEntityName + Default + Iden + Clone + 'static,
    <R::Entity as LifeModelTrait>::Column: Copy + Iden + 'static,
    <R::Entity as LifeModelTrait>::Model: FromRow,
{
    let mut out = BatchOutcome::default();
    if records.is_empty() {
        return Ok(out);
    }
    // Hooks, validators and column extraction for every record up front: a bad record fails the
    // call before anything is written.
    let mut columns: Option<Vec<<R::Entity as LifeModelTrait>::Column>> = None;
    let mut rows: Vec<Vec<Expr>> = Vec::with_capacity(records.len());
    for (i, r) in records.iter().enumerate() {
        let (cols, exprs) = r.__lg_insert_row().map_err(|source| BatchError {
            chunk: 0,
            rows: i..i + 1,
            rows_written_before: 0,
            source,
            kind: FailureKind::Other,
        })?;
        match &columns {
            None => columns = Some(cols),
            Some(first) => {
                let same = first.len() == cols.len()
                    && first
                        .iter()
                        .zip(&cols)
                        .all(|(a, b)| column_name(a) == column_name(b));
                if !same {
                    return Err(BatchError {
                        chunk: 0,
                        rows: i..i + 1,
                        rows_written_before: 0,
                        source: ActiveModelError::Other(format!(
                            "batch insert: record {i} sets different columns than record 0; one call writes one column set"
                        )),
                        kind: FailureKind::Other,
                    });
                }
            }
        }
        rows.push(exprs);
    }
    let columns = columns.unwrap_or_default();
    let ncols = columns.len();
    let entity = R::Entity::default();
    let metrics = <R::Entity as LifeModelTrait>::__lg_entity_metrics();
    let op = if on_conflict.is_some() {
        crate::metrics::entity::Op::Upsert
    } else {
        crate::metrics::entity::Op::Insert
    };

    // One chunk's statement: `VALUES` or `unnest`, plus the ON CONFLICT / RETURNING clauses.
    let build = |chunk: &[Vec<Expr>]| -> Result<(String, Vec<OwnedParam>), ActiveModelError> {
        // Single-row-shaped statement built by SeaQuery: gives the quoted prefix, and the
        // ON CONFLICT / RETURNING suffix for the unnest form.
        let mut q = Query::insert();
        if let Some(schema) = entity.schema_name() {
            q.into_table((sea_query::Alias::new(schema), entity.clone()));
        } else {
            q.into_table(entity.clone());
        }
        q.columns(columns.iter().copied());

        let use_unnest = chunk.len() >= opts.unnest_min_rows;
        let arrays = if use_unnest {
            columns_as_arrays(chunk, ncols)
        } else {
            None
        };

        if let Some(arrays) = arrays {
            // Render the clauses around a one-row VALUES, then swap VALUES for unnest.
            q.values_panic(chunk[0].iter().cloned());
            if let Some(oc) = on_conflict.clone() {
                q.on_conflict(oc);
            }
            if opts.returning {
                q.returning_all();
            }
            let (one_row_sql, one_row_values) = q.build(PostgresQueryBuilder);
            if let Some(sql) = unnest_sql::<R::Entity>(
                &one_row_sql,
                one_row_values.0.len(),
                ncols,
                &arrays,
                &columns,
            ) {
                return Ok((
                    sql,
                    arrays
                        .into_iter()
                        .map(|(a, _)| OwnedParam::Array(a))
                        .collect(),
                ));
            }
        }
        let (sql, values) = values_statement(
            &entity,
            &columns,
            chunk,
            on_conflict.clone(),
            opts.returning,
        );
        Ok((sql, values_to_owned(&values)?))
    };

    let pk_index = primary_key_column::<R::Entity>().ok().and_then(|pk| {
        columns
            .iter()
            .position(|c| column_name(c) == column_name(&pk))
    });
    let keys: Option<Vec<String>> =
        pk_index.map(|i| rows.iter().map(|r| format!("{:?}", r[i])).collect());
    let self_ref: Vec<usize> = columns
        .iter()
        .enumerate()
        .filter(|(_, c)| references_own_table::<R::Entity>(**c))
        .map(|(i, _)| i)
        .collect();
    let refs_in_batch =
        refs_within::<Expr>(keys.as_deref(), &self_ref, &rows, |e| format!("{e:?}"));
    let plan = Plan::resolve::<R::Entity>(opts, keys.as_deref(), refs_in_batch);
    out.policy_applied = plan.applied();
    out.fallback_reason = plan.fallback.clone();
    let chunks = chunk_ranges(rows.len(), ncols, opts);
    let (r, extra) = drive(&plan, chunks, &mut out, |part| {
        let (sql, params) = build(&rows[part]).map_err(ChunkError::Other)?;
        exec_chunk(executor, metrics, op, &sql, params, opts.returning)
    });
    plan.record(metrics, &out, extra);
    r.map(|()| out)
}

fn values_statement<E, C>(
    entity: &E,
    columns: &[C],
    chunk: &[Vec<Expr>],
    on_conflict: Option<OnConflict>,
    returning: bool,
) -> (String, sea_query::Values)
where
    E: LifeEntityName + Iden + Clone + 'static,
    C: Iden + Copy + 'static,
{
    let mut q = Query::insert();
    if let Some(schema) = entity.schema_name() {
        q.into_table((sea_query::Alias::new(schema), entity.clone()));
    } else {
        q.into_table(entity.clone());
    }
    q.columns(columns.iter().copied());
    for row in chunk {
        q.values_panic(row.iter().cloned());
    }
    if let Some(oc) = on_conflict {
        q.on_conflict(oc);
    }
    if returning {
        q.returning_all();
    }
    q.build(PostgresQueryBuilder)
}

/// Turn a rendered one-row `INSERT .. VALUES ($1, .., $n) <suffix>` into
/// `INSERT .. SELECT .. FROM unnest($1::t[], ..) AS lg_u(..) <suffix>`. `None` when the suffix
/// carries parameters of its own (they would need renumbering) or the shape is unexpected.
fn unnest_sql<E: LifeModelTrait>(
    one_row_sql: &str,
    one_row_params: usize,
    ncols: usize,
    arrays: &[(OwnedArray, &'static str)],
    columns: &[E::Column],
) -> Option<String>
where
    E::Column: Copy,
{
    if one_row_params != ncols {
        return None;
    }
    let values_at = one_row_sql.find(" VALUES (")?;
    let close = one_row_sql[values_at..].find(')')? + values_at;
    let prefix = &one_row_sql[..values_at];
    let suffix = &one_row_sql[close + 1..];
    let mut sql = String::with_capacity(one_row_sql.len() + ncols * 24);
    sql.push_str(prefix);
    sql.push_str(" SELECT ");
    for (i, c) in columns.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("lg_u.c{i}"));
        // Text arrays feeding JSON columns need an explicit cast (no assignment cast exists).
        if matches!(arrays[i].0, OwnedArray::Text(_)) && is_json_column::<E>(*c) {
            sql.push_str("::jsonb");
        }
    }
    sql.push_str(" FROM unnest(");
    for (i, (_, ty)) in arrays.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("${}::{ty}[]", i + 1));
    }
    sql.push_str(") AS lg_u(");
    for i in 0..ncols {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("c{i}"));
    }
    sql.push(')');
    sql.push_str(suffix);
    Some(sql)
}

// ---------------------------------------------------------------------------------------------
// update_many / delete_many / find_by_ids

/// Update `columns` of many rows, each row keyed by its primary key, in one statement per chunk:
/// `UPDATE t SET c = lg_v.c .. FROM unnest(..) AS lg_v(..) WHERE t.pk = lg_v.pk`.
///
/// Each record runs `before_update` and its validators. Every record must have its primary key
/// and each of `columns` set.
///
/// # Errors
///
/// [`BatchError`] for the first failing chunk.
pub fn update_many<R>(
    executor: &dyn LifeExecutor,
    records: &[R],
    columns: &[<R::Entity as LifeModelTrait>::Column],
    opts: &BatchOptions,
) -> Result<BatchOutcome<<R::Entity as LifeModelTrait>::Model>, BatchError>
where
    R: ActiveModelTrait + ActiveModelBehavior,
    R::Entity: LifeEntityName + Default,
    <R::Entity as LifeModelTrait>::Column: Copy + Iden,
    <R::Entity as LifeModelTrait>::Model: FromRow,
{
    let mut out = BatchOutcome::default();
    if records.is_empty() || columns.is_empty() {
        return Ok(out);
    }
    let pk = primary_key_column::<R::Entity>().map_err(BatchError::before_any)?;
    let mut rows: Vec<Vec<Expr>> = Vec::with_capacity(records.len());
    for (i, r) in records.iter().enumerate() {
        let mut rec = r.clone();
        let bad = |source| BatchError {
            chunk: 0,
            rows: i..i + 1,
            rows_written_before: 0,
            source,
            kind: FailureKind::Other,
        };
        rec.before_update().map_err(bad)?;
        crate::active_model::run_validators(&rec, crate::active_model::ValidateOp::Update)
            .map_err(bad)?;
        let mut row = Vec::with_capacity(columns.len() + 1);
        let key = rec
            .get(pk)
            .ok_or_else(|| bad(ActiveModelError::PrimaryKeyRequired))?;
        row.push(Expr::Value(key));
        for c in columns {
            let v = rec.get(*c).ok_or_else(|| {
                bad(ActiveModelError::Other(format!(
                    "update_many: record {i} does not set column {}",
                    column_name(c)
                )))
            })?;
            row.push(Expr::Value(v));
        }
        rows.push(row);
    }
    let entity = R::Entity::default();
    let table = table_sql(&entity);
    let pk_name = quote_ident(&column_name(&pk));
    let metrics = <R::Entity as LifeModelTrait>::__lg_entity_metrics();
    let ncols = columns.len() + 1;
    // Row layout: key first, then the updated columns.
    let keys: Vec<String> = rows.iter().map(|r| format!("{:?}", r[0])).collect();
    let self_ref: Vec<usize> = columns
        .iter()
        .enumerate()
        .filter(|(_, c)| references_own_table::<R::Entity>(**c))
        .map(|(i, _)| i + 1)
        .collect();
    let refs_in_batch = refs_within::<Expr>(Some(&keys), &self_ref, &rows, |e| format!("{e:?}"));
    let plan = Plan::resolve::<R::Entity>(opts, Some(&keys), refs_in_batch);
    out.policy_applied = plan.applied();
    out.fallback_reason = plan.fallback.clone();
    let build = |part: &[Vec<Expr>]| -> Result<(String, Vec<OwnedParam>), ActiveModelError> {
        let arrays = columns_as_arrays(part, ncols).ok_or_else(|| {
            ActiveModelError::Other(
                "update_many: a column has no array mapping (all NULL, mixed types or an expression)"
                    .to_string(),
            )
        })?;
        let mut sql = format!("UPDATE {table} AS lg_t SET ");
        for (i, c) in columns.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }
            let cast = if matches!(arrays[i + 1].0, OwnedArray::Text(_))
                && is_json_column::<R::Entity>(*c)
            {
                "::jsonb"
            } else {
                ""
            };
            sql.push_str(&format!(
                "{} = lg_v.c{}{cast}",
                quote_ident(&column_name(c)),
                i + 1
            ));
        }
        sql.push_str(" FROM unnest(");
        for (i, (_, ty)) in arrays.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }
            sql.push_str(&format!("${}::{ty}[]", i + 1));
        }
        sql.push_str(") AS lg_v(");
        for i in 0..ncols {
            if i > 0 {
                sql.push_str(", ");
            }
            sql.push_str(&format!("c{i}"));
        }
        sql.push_str(&format!(") WHERE lg_t.{pk_name} = lg_v.c0"));
        if opts.returning {
            sql.push_str(" RETURNING lg_t.*");
        }
        Ok((
            sql,
            arrays
                .into_iter()
                .map(|(a, _)| OwnedParam::Array(a))
                .collect(),
        ))
    };
    let chunks = chunk_ranges(rows.len(), 1, opts);
    let (r, extra) = drive(&plan, chunks, &mut out, |part| {
        let (sql, params) = build(&rows[part]).map_err(ChunkError::Other)?;
        exec_chunk(
            executor,
            metrics,
            crate::metrics::entity::Op::Update,
            &sql,
            params,
            opts.returning,
        )
    });
    plan.record(metrics, &out, extra);
    r.map(|()| out)
}

fn key_array(ids: &[Value]) -> Result<(OwnedArray, &'static str), ActiveModelError> {
    let first = ids
        .iter()
        .find(|v| !value_is_null(v))
        .ok_or_else(|| ActiveModelError::Other("batch by key: no non-NULL keys".to_string()))?;
    let (mut arr, ty) = OwnedArray::for_value(first).ok_or_else(|| {
        ActiveModelError::Other(format!("batch by key: unsupported key type {first:?}"))
    })?;
    for v in ids {
        if !arr.push(v) {
            return Err(ActiveModelError::Other(format!(
                "batch by key: key {v:?} does not match the type of the first key"
            )));
        }
    }
    Ok((arr, ty))
}

/// Delete rows by primary key: `DELETE FROM t WHERE pk = ANY($1)`, one statement per chunk.
///
/// This is a direct delete: per-record `before_delete` hooks do not run (there are no records).
///
/// # Errors
///
/// [`BatchError`] for the first failing chunk.
pub fn delete_many<E>(
    executor: &dyn LifeExecutor,
    ids: &[Value],
    opts: &BatchOptions,
) -> Result<BatchOutcome<E::Model>, BatchError>
where
    E: LifeModelTrait + LifeEntityName + Default,
    E::Column: Copy + Iden,
    E::Model: FromRow,
{
    let mut out = BatchOutcome::default();
    if ids.is_empty() {
        return Ok(out);
    }
    let pk = primary_key_column::<E>().map_err(BatchError::before_any)?;
    let table = table_sql(&E::default());
    let pk_name = quote_ident(&column_name(&pk));
    let metrics = E::__lg_entity_metrics();
    let keys: Vec<String> = ids.iter().map(|v| format!("{v:?}")).collect();
    let plan = Plan::resolve::<E>(opts, Some(&keys), false);
    out.policy_applied = plan.applied();
    out.fallback_reason = plan.fallback.clone();
    let chunks = chunk_ranges(ids.len(), 1, opts);
    let (r, extra) = drive(&plan, chunks, &mut out, |part| {
        let (arr, ty) = key_array(&ids[part]).map_err(ChunkError::Other)?;
        let mut sql = format!("DELETE FROM {table} WHERE {pk_name} = ANY($1::{ty}[])");
        if opts.returning {
            sql.push_str(" RETURNING *");
        }
        exec_chunk(
            executor,
            metrics,
            crate::metrics::entity::Op::Delete,
            &sql,
            vec![OwnedParam::Array(arr)],
            opts.returning,
        )
    });
    plan.record(metrics, &out, extra);
    r.map(|()| out)
}

/// Load the rows whose primary key is in `ids`: `SELECT .. FROM t WHERE pk = ANY($1)` (the
/// entity's normal projection and soft-delete filter apply). One statement per chunk of
/// [`BatchOptions::max_rows`] keys, whatever their number, instead of one per key.
///
/// Rows come back in database order, not in `ids` order; missing keys are simply absent.
/// Reads run on the executor's primary tier.
///
/// # Errors
///
/// [`BatchError`] for the first failing chunk.
pub fn find_by_ids<E>(
    executor: &dyn LifeExecutor,
    ids: &[Value],
    opts: &BatchOptions,
) -> Result<Vec<E::Model>, BatchError>
where
    E: LifeModelTrait + LifeEntityName + Default,
    E::Column: Copy + Iden,
    E::Model: FromRow,
{
    let mut models = Vec::with_capacity(ids.len());
    if ids.is_empty() {
        return Ok(models);
    }
    let pk = primary_key_column::<E>().map_err(BatchError::before_any)?;
    let pk_name = quote_ident(&column_name(&pk));
    let metrics = E::__lg_entity_metrics();
    const MARK: &str = "lg_batch_ids_marker";
    for (chunk_index, range) in chunk_ranges(ids.len(), 1, opts).into_iter().enumerate() {
        let fail = |source: ActiveModelError| BatchError {
            chunk: chunk_index,
            rows: range.clone(),
            rows_written_before: 0,
            source,
            kind: FailureKind::Other,
        };
        let (arr, ty) = key_array(&ids[range.clone()]).map_err(fail)?;
        // The entity's own SELECT (projection, soft delete) with a marker condition that is then
        // replaced by the array predicate, numbered after SeaQuery's own parameters.
        let select = E::find().filter(Expr::cust(MARK));
        let (sql, values) = select.query.build(PostgresQueryBuilder);
        let mut params = values_to_owned(&values).map_err(fail)?;
        params.push(OwnedParam::Array(arr));
        let sql = sql.replacen(
            MARK,
            &format!("{pk_name} = ANY(${}::{ty}[])", params.len()),
            1,
        );
        let timer =
            crate::metrics::entity::OpTimer::start(metrics, crate::metrics::entity::Op::Select);
        let res = executor.__lg_query_all_owned(&sql, params);
        timer.finish(res.as_ref().map_or(0, |r| r.len() as u64), res.is_ok());
        let rows = res.map_err(|e| fail(db(&e)))?;
        for row in &rows {
            models.push(
                E::Model::from_row(row)
                    .map_err(|e| fail(ActiveModelError::DatabaseError(e.to_string())))?,
            );
        }
    }
    Ok(models)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn fk_target_parsing() {
        assert!(fk_targets_table(
            "accounts(id) ON DELETE SET NULL",
            "accounts"
        ));
        assert!(fk_targets_table("ledger.accounts (id)", "accounts"));
        assert!(fk_targets_table("\"Accounts\"(id)", "accounts"));
        assert!(!fk_targets_table("users(id)", "accounts"));
    }

    #[test]
    fn duplicate_detection() {
        assert!(has_duplicates(&["1".into(), "2".into(), "1".into()]));
        assert!(!has_duplicates(&["1".into(), "2".into()]));
    }

    #[test]
    fn chunks_respect_row_and_param_caps() {
        let opts = BatchOptions {
            max_rows: 1000,
            max_params: 100,
            ..BatchOptions::default()
        };
        // 10 columns: at most 10 rows per chunk by the parameter cap.
        let r = chunk_ranges(25, 10, &opts);
        assert_eq!(r, vec![0..10, 10..20, 20..25]);
        let r = chunk_ranges(5, 1, &opts);
        assert_eq!(r, vec![0..5]);
        // Never above PostgreSQL's limit even if asked.
        let big = BatchOptions {
            max_rows: usize::MAX,
            max_params: usize::MAX,
            ..BatchOptions::default()
        };
        let r = chunk_ranges(100_000, 3, &big);
        assert!(r.iter().all(|c| c.len() * 3 <= POSTGRES_MAX_PARAMS));
    }

    #[test]
    fn arrays_from_plain_values_only() {
        let rows = vec![
            vec![
                Expr::Value(Value::BigInt(Some(1))),
                Expr::Value(Value::String(Some("a".into()))),
            ],
            vec![
                Expr::Value(Value::BigInt(Some(2))),
                Expr::Value(Value::String(None)),
            ],
        ];
        let arrays = columns_as_arrays(&rows, 2).expect("plain values map to arrays");
        assert_eq!(arrays[0].1, "int8");
        assert_eq!(arrays[1].1, "text");
        assert_eq!(arrays[1].0.len(), 2);
        // An all-NULL column has no type to cast to: no unnest.
        let nulls = vec![vec![Expr::Value(Value::Int(None))]];
        assert!(columns_as_arrays(&nulls, 1).is_none());
        // Expressions (save_as) are not values: no unnest.
        let exprs = vec![vec![Expr::cust("now()")]];
        assert!(columns_as_arrays(&exprs, 1).is_none());
    }

    #[test]
    fn identifiers_are_quoted() {
        assert_eq!(quote_ident("we\"ird"), "\"we\"\"ird\"");
    }

    #[test]
    fn batch_error_names_its_chunk() {
        let e = BatchError {
            chunk: 2,
            rows: 2000..3000,
            rows_written_before: 2000,
            source: ActiveModelError::Other("boom".into()),
            kind: FailureKind::Other,
        };
        let s = e.to_string();
        assert!(
            s.contains("chunk 2") && s.contains("2000..3000") && s.contains("boom"),
            "{s}"
        );
    }
}
