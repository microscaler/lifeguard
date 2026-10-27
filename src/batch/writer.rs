//! [`BatchWriter`]: coroutines push rows; one flusher writes them in batches.
//!
//! Ingest-style code that loops `record.insert(&exec)` pays one round trip per row. A
//! `BatchWriter` buffers rows and writes them with [`super::insert_many`] /
//! [`super::upsert_many`] when [`BatchWriterConfig::max_rows`] rows are buffered or
//! [`BatchWriterConfig::max_delay`] has passed since the oldest buffered row, whichever is first.
//!
//! # Semantics
//!
//! * **Backpressure.** At most [`BatchWriterConfig::capacity`] rows wait to be written. `push`
//!   parks the caller (coroutine or thread) until there is room, so a slow database slows its
//!   producers instead of growing memory.
//! * **Ordering.** One flusher writes batches strictly in push order, so for any key a later
//!   push is never overwritten by an earlier one. Within one batch, [`WriteMode::Upsert`] keeps
//!   only the **last** row per key (PostgreSQL rejects a statement that updates the same row
//!   twice), which is the same last-write-wins result.
//! * **Delivery.** At-least-once. A failed batch is retried
//!   [`BatchWriterConfig::retries`] times with backoff; then its rows go to the error handler
//!   (dead letter) and the writer carries on. A retried batch may have been partly written (a
//!   later chunk failed), so exactly-once needs idempotent writes: use [`WriteMode::Upsert`] keyed
//!   on a natural key, which makes a replay converge to the same rows.
//! * **Shutdown.** [`BatchWriter::flush`] writes everything buffered and returns.
//!   [`BatchWriter::close`] flushes and stops the flusher. Dropping an open writer closes it
//!   (best effort, bounded wait); prefer an explicit `close` so errors are seen.

use super::{insert_many, upsert_many, BatchError, BatchOptions};
use crate::active_model::{ActiveModelBehavior, ActiveModelTrait};
use crate::executor::LifeExecutor;
use crate::query::traits::{FromRow, LifeEntityName, LifeModelTrait};
use may::sync::mpsc;
use may::sync::Semphore;
use sea_query::{Iden, OnConflict};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How a [`BatchWriter`] writes its rows.
#[derive(Clone)]
pub enum WriteMode<C> {
    /// Plain multi-row `INSERT`.
    Insert,
    /// `INSERT .. ON CONFLICT ..`, de-duplicated per batch on `key` (last row wins).
    Upsert {
        /// The conflict clause, e.g. `OnConflict::columns([..]).update_columns([..])`.
        on_conflict: OnConflict,
        /// Columns identifying a row (normally the conflict target).
        key: Vec<C>,
    },
}

/// Tuning for a [`BatchWriter`].
#[derive(Debug, Clone)]
pub struct BatchWriterConfig {
    /// Write when this many rows are buffered.
    pub max_rows: usize,
    /// Write when the oldest buffered row has waited this long.
    pub max_delay: Duration,
    /// Rows that may wait to be written before `push` blocks.
    pub capacity: usize,
    /// Retries of a failed batch before it goes to the error handler.
    pub retries: u32,
    /// Delay before the first retry; doubles per retry.
    pub retry_backoff: Duration,
    /// Chunking for each write.
    pub options: BatchOptions,
}

impl Default for BatchWriterConfig {
    fn default() -> Self {
        Self {
            max_rows: 500,
            max_delay: Duration::from_millis(50),
            capacity: 10_000,
            retries: 3,
            retry_backoff: Duration::from_millis(100),
            options: BatchOptions::default(),
        }
    }
}

/// Counters of a [`BatchWriter`] (all monotonic).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BatchWriterStats {
    /// Rows accepted by `push`.
    pub pushed: u64,
    /// Rows written (statements reported them affected; `DO NOTHING` skips are not counted).
    pub written: u64,
    /// Rows dropped as duplicates of a later row with the same key in the same batch.
    pub superseded: u64,
    /// Batches written.
    pub batches: u64,
    /// Retries of failed batches.
    pub retries: u64,
    /// Rows handed to the error handler after the last retry failed.
    pub failed: u64,
}

#[derive(Default)]
struct Counters {
    pushed: AtomicU64,
    written: AtomicU64,
    superseded: AtomicU64,
    batches: AtomicU64,
    retries: AtomicU64,
    failed: AtomicU64,
}

enum Msg<R> {
    Row(R),
    Flush(mpsc::Sender<()>),
}

type ErrorHandler<R> = Arc<dyn Fn(&BatchError, Vec<R>) + Send + Sync>;

/// A buffered, batching writer for one entity. See the [module docs](self).
pub struct BatchWriter<R: Send + 'static> {
    tx: Option<mpsc::Sender<Msg<R>>>,
    room: Arc<Semphore>,
    counters: Arc<Counters>,
    closed: Arc<AtomicBool>,
    flusher: Option<may::coroutine::JoinHandle<()>>,
}

impl<R> BatchWriter<R>
where
    R: ActiveModelTrait + ActiveModelBehavior + Send + 'static,
    R::Entity: LifeEntityName + Default + Iden + Clone + 'static,
    <R::Entity as LifeModelTrait>::Column: Copy + Iden + Send + Sync + 'static,
    <R::Entity as LifeModelTrait>::Model: FromRow,
{
    /// Start a writer and its flusher. `on_error` receives batches that still failed after the
    /// last retry (log them, or keep them for replay).
    pub fn new<X>(
        executor: X,
        mode: WriteMode<<R::Entity as LifeModelTrait>::Column>,
        config: BatchWriterConfig,
        on_error: impl Fn(&BatchError, Vec<R>) + Send + Sync + 'static,
    ) -> Self
    where
        X: LifeExecutor + Send + Sync + 'static,
    {
        let (tx, rx) = mpsc::channel::<Msg<R>>();
        let room = Arc::new(Semphore::new(config.capacity.max(1)));
        let counters = Arc::new(Counters::default());
        let closed = Arc::new(AtomicBool::new(false));
        let flusher = {
            let room = Arc::clone(&room);
            let counters = Arc::clone(&counters);
            let on_error: ErrorHandler<R> = Arc::new(on_error);
            may::go!(
                may::coroutine::Builder::new()
                    .name("lifeguard-batch-writer".to_owned())
                    .stack_size(crate::pool::pooled::job_stack_size().max(64 * 1024)), // words: 512 KiB
                move || run_flusher(&executor, &mode, &config, &rx, &room, &counters, &on_error)
            )
        };
        Self {
            tx: Some(tx),
            room,
            counters,
            closed,
            // Spawning only fails for a name with a NUL byte; the name is a constant.
            flusher: flusher.ok(),
        }
    }
}

impl<R: Send + 'static> BatchWriter<R> {
    /// Queue one row. Parks while [`BatchWriterConfig::capacity`] rows are already waiting.
    ///
    /// # Errors
    ///
    /// Returns the row back when the writer is closed.
    pub fn push(&self, row: R) -> Result<(), R> {
        if self.closed.load(Ordering::Acquire) {
            return Err(row);
        }
        self.room.wait();
        let Some(tx) = &self.tx else {
            self.room.post();
            return Err(row);
        };
        match tx.send(Msg::Row(row)) {
            Ok(()) => {
                self.counters.pushed.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(std::sync::mpsc::SendError(Msg::Row(row))) => {
                self.room.post();
                Err(row)
            }
            Err(std::sync::mpsc::SendError(Msg::Flush(_))) => unreachable_flush(),
        }
    }

    /// Write everything pushed so far; returns once it has been written (or handed to the error
    /// handler).
    pub fn flush(&self) {
        let Some(tx) = &self.tx else {
            return;
        };
        let (done_tx, done_rx) = mpsc::channel();
        if tx.send(Msg::Flush(done_tx)).is_ok() {
            let _ = done_rx.recv();
        }
    }

    /// Flush and stop the flusher.
    pub fn close(mut self) -> BatchWriterStats {
        self.shutdown(None);
        self.stats()
    }

    /// Current counters.
    #[must_use]
    pub fn stats(&self) -> BatchWriterStats {
        let c = &self.counters;
        BatchWriterStats {
            pushed: c.pushed.load(Ordering::Relaxed),
            written: c.written.load(Ordering::Relaxed),
            superseded: c.superseded.load(Ordering::Relaxed),
            batches: c.batches.load(Ordering::Relaxed),
            retries: c.retries.load(Ordering::Relaxed),
            failed: c.failed.load(Ordering::Relaxed),
        }
    }

    fn shutdown(&mut self, wait: Option<Duration>) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        // Dropping the sender ends the flusher's receive loop after it drains what is queued.
        self.tx = None;
        if let Some(h) = self.flusher.take() {
            match wait {
                None => {
                    let _ = h.join();
                }
                Some(limit) => {
                    let deadline = Instant::now() + limit;
                    while !h.is_done() && Instant::now() < deadline {
                        may::coroutine::sleep(Duration::from_millis(5));
                    }
                }
            }
        }
    }
}

impl<R: Send + 'static> Drop for BatchWriter<R> {
    fn drop(&mut self) {
        self.shutdown(Some(Duration::from_secs(30)));
    }
}

#[cold]
fn unreachable_flush() -> ! {
    #[allow(clippy::panic)]
    {
        panic!("push only sends rows")
    }
}

#[allow(clippy::needless_pass_by_value)]
fn run_flusher<R, X>(
    executor: &X,
    mode: &WriteMode<<R::Entity as LifeModelTrait>::Column>,
    config: &BatchWriterConfig,
    rx: &mpsc::Receiver<Msg<R>>,
    room: &Semphore,
    counters: &Counters,
    on_error: &ErrorHandler<R>,
) where
    R: ActiveModelTrait + ActiveModelBehavior + Send + 'static,
    R::Entity: LifeEntityName + Default + Iden + Clone + 'static,
    <R::Entity as LifeModelTrait>::Column: Copy + Iden + 'static,
    <R::Entity as LifeModelTrait>::Model: FromRow,
    X: LifeExecutor,
{
    let max_rows = config.max_rows.max(1);
    let mut buf: Vec<R> = Vec::with_capacity(max_rows);
    let mut waiters: Vec<mpsc::Sender<()>> = Vec::new();
    let mut oldest: Option<Instant> = None;
    loop {
        let msg = match oldest {
            None => match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => break,
            },
            Some(t) => {
                let left = config.max_delay.saturating_sub(t.elapsed());
                match rx.recv_timeout(left) {
                    Ok(m) => Some(m),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        };
        let mut force = msg.is_none();
        match msg {
            Some(Msg::Row(r)) => {
                if buf.is_empty() {
                    oldest = Some(Instant::now());
                }
                buf.push(r);
            }
            Some(Msg::Flush(done)) => {
                waiters.push(done);
                force = true;
            }
            None => {}
        }
        if force || buf.len() >= max_rows {
            write_batch(
                executor,
                mode,
                config,
                std::mem::take(&mut buf),
                room,
                counters,
                on_error,
            );
            oldest = None;
            for w in waiters.drain(..) {
                let _ = w.send(());
            }
        }
    }
    // Sender dropped: write what is left.
    write_batch(executor, mode, config, buf, room, counters, on_error);
    for w in waiters {
        let _ = w.send(());
    }
}

fn write_batch<R, X>(
    executor: &X,
    mode: &WriteMode<<R::Entity as LifeModelTrait>::Column>,
    config: &BatchWriterConfig,
    rows: Vec<R>,
    room: &Semphore,
    counters: &Counters,
    on_error: &ErrorHandler<R>,
) where
    R: ActiveModelTrait + ActiveModelBehavior + Send + 'static,
    R::Entity: LifeEntityName + Default + Iden + Clone + 'static,
    <R::Entity as LifeModelTrait>::Column: Copy + Iden + 'static,
    <R::Entity as LifeModelTrait>::Model: FromRow,
    X: LifeExecutor,
{
    if rows.is_empty() {
        return;
    }
    let n_in = rows.len();
    let rows = match mode {
        WriteMode::Insert => rows,
        WriteMode::Upsert { key, .. } => last_per_key(rows, key),
    };
    counters
        .superseded
        .fetch_add((n_in - rows.len()) as u64, Ordering::Relaxed);
    let mut opts = config.options.clone();
    opts.returning = false;
    let mut attempt = 0u32;
    let exec: &dyn LifeExecutor = executor;
    loop {
        let res = match mode {
            WriteMode::Insert => insert_many(exec, &rows, &opts),
            WriteMode::Upsert { on_conflict, .. } => {
                upsert_many(exec, &rows, on_conflict.clone(), &opts)
            }
        };
        match res {
            Ok(out) => {
                counters
                    .written
                    .fetch_add(out.rows_affected, Ordering::Relaxed);
                counters.batches.fetch_add(1, Ordering::Relaxed);
                break;
            }
            Err(e) if attempt < config.retries => {
                attempt += 1;
                counters.retries.fetch_add(1, Ordering::Relaxed);
                log::warn!("lifeguard batch writer: retry {attempt} after: {e}");
                may::coroutine::sleep(config.retry_backoff * 2u32.saturating_pow(attempt - 1));
            }
            Err(e) => {
                counters
                    .failed
                    .fetch_add(rows.len() as u64, Ordering::Relaxed);
                log::error!(
                    "lifeguard batch writer: dropping {} rows after {attempt} retries: {e}",
                    rows.len()
                );
                on_error(&e, rows);
                break;
            }
        }
    }
    for _ in 0..n_in {
        room.post();
    }
}

/// Keep the last row per key, in the order each key was last seen.
fn last_per_key<R>(rows: Vec<R>, key: &[<R::Entity as LifeModelTrait>::Column]) -> Vec<R>
where
    R: ActiveModelTrait,
    <R::Entity as LifeModelTrait>::Column: Copy,
{
    if key.is_empty() {
        return rows;
    }
    let mut last: HashMap<String, usize> = HashMap::with_capacity(rows.len());
    for (i, r) in rows.iter().enumerate() {
        let k: Vec<_> = key.iter().map(|c| r.get(*c)).collect();
        last.insert(format!("{k:?}"), i);
    }
    let mut keep: Vec<usize> = last.into_values().collect();
    keep.sort_unstable();
    let mut rows: Vec<Option<R>> = rows.into_iter().map(Some).collect();
    keep.into_iter().filter_map(|i| rows[i].take()).collect()
}
