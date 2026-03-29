//! `may`-scheduled connection pool with one coroutine consumer per slot (SPSC per slot, MPSC overall).
//!
//! Each worker owns one [`may_postgres::Client`] and drains a dedicated [`may::sync::mpsc`] queue.
//! **Note:** `may` 0.3 exposes an unbounded `channel()` on this path; bounded `sync_channel` is not
//! part of the public `may::sync::mpsc` API in the pinned release. Callers should size `pool_size`
//! to match expected concurrency.

use crate::connection::connect;
use crate::executor::{LifeError, LifeExecutor};
use crate::pool::owned_param::OwnedParam;
use may_postgres::{Client, Row};
use may_postgres::types::ToSql;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[cfg(feature = "metrics")]
use crate::metrics::METRICS;
#[cfg(feature = "tracing")]
use crate::metrics::tracing_helpers;

/// Pool of `PostgreSQL` connections with round-robin dispatch to worker coroutines.
pub struct LifeguardPool {
    worker_txs: Arc<[may::sync::mpsc::Sender<WorkerJob>]>,
    next_worker: AtomicUsize,
    pool_size: usize,
}

impl LifeguardPool {
    /// Open `pool_size` connections and spawn one `may` worker per connection.
    ///
    /// # Errors
    ///
    /// Returns [`LifeError::Other`] when a slot cannot connect (see [`connect`]).
    pub fn new(connection_string: &str, pool_size: usize) -> Result<Self, LifeError> {
        if pool_size == 0 {
            return Err(LifeError::Pool(
                "LifeguardPool::new: pool_size must be at least 1".to_string(),
            ));
        }

        let mut txs = Vec::with_capacity(pool_size);

        for slot in 0..pool_size {
            let client = connect(connection_string).map_err(|e| {
                LifeError::Other(format!("pool connection slot {slot}: {e}"))
            })?;

            let (job_tx, job_rx) = may::sync::mpsc::channel::<WorkerJob>();

            let handle = may::go!(move || {
                run_worker(client, job_rx);
            });
            #[allow(clippy::mem_forget)] // Workers must outlive the pool handle.
            std::mem::forget(handle);

            txs.push(job_tx);
        }

        let worker_txs: Arc<[may::sync::mpsc::Sender<WorkerJob>]> = txs.into();

        #[cfg(feature = "metrics")]
        {
            METRICS.set_pool_size(pool_size as u64);
            METRICS.set_active_connections(pool_size as u64);
        }

        Ok(Self {
            worker_txs,
            next_worker: AtomicUsize::new(0),
            pool_size,
        })
    }

    #[must_use]
    pub fn pool_size(&self) -> usize {
        self.pool_size
    }

    fn pick_worker(&self) -> &may::sync::mpsc::Sender<WorkerJob> {
        let i = self.next_worker.fetch_add(1, Ordering::Relaxed) % self.pool_size;
        &self.worker_txs[i]
    }

    fn dispatch<T: Send + 'static>(
        &self,
        build: impl FnOnce(std::sync::mpsc::SyncSender<Result<T, LifeError>>) -> WorkerJob,
    ) -> Result<T, LifeError> {
        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::acquire_connection_span().entered();

        let wait_start = Instant::now();
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
        let job = build(reply_tx);
        let tx = self.pick_worker();
        tx.send(job)
            .map_err(|e| LifeError::Pool(format!("pool job send failed: {e}")))?;

        #[cfg(feature = "metrics")]
        METRICS.record_connection_wait(wait_start.elapsed());

        match reply_rx.recv() {
            Ok(r) => r,
            Err(_) => Err(LifeError::Pool(
                "pool reply channel closed unexpectedly".to_string(),
            )),
        }
    }
}

enum WorkerJob {
    Execute {
        query: String,
        params: Vec<OwnedParam>,
        reply: std::sync::mpsc::SyncSender<Result<u64, LifeError>>,
    },
    QueryOne {
        query: String,
        params: Vec<OwnedParam>,
        reply: std::sync::mpsc::SyncSender<Result<Row, LifeError>>,
    },
    QueryAll {
        query: String,
        params: Vec<OwnedParam>,
        reply: std::sync::mpsc::SyncSender<Result<Vec<Row>, LifeError>>,
    },
}

fn run_worker(client: Client, job_rx: may::sync::mpsc::Receiver<WorkerJob>) {
    for job in job_rx {
        match job {
            WorkerJob::Execute {
                query,
                params,
                reply,
            } => {
                let result = exec_on_client(&client, &query, &params, |c, q, r| {
                    c.execute(q, r).map_err(LifeError::from)
                });
                let _ = reply.send(result);
            }
            WorkerJob::QueryOne {
                query,
                params,
                reply,
            } => {
                let result = exec_on_client(&client, &query, &params, |c, q, r| {
                    c.query_one(q, r).map_err(LifeError::from)
                });
                let _ = reply.send(result);
            }
            WorkerJob::QueryAll {
                query,
                params,
                reply,
            } => {
                let result = exec_on_client(&client, &query, &params, |c, q, r| {
                    c.query(q, r).map_err(LifeError::from)
                });
                let _ = reply.send(result);
            }
        }
    }
}

fn exec_on_client<T>(
    client: &Client,
    query: &str,
    params: &[OwnedParam],
    op: impl FnOnce(&Client, &str, &[&dyn ToSql]) -> Result<T, LifeError>,
) -> Result<T, LifeError> {
    let refs: Vec<&dyn ToSql> = params.iter().map(OwnedParam::as_sql_ref).collect();

    #[cfg(feature = "tracing")]
    let _span = tracing_helpers::execute_query_span(query).entered();

    let start = Instant::now();
    let out = op(client, query, &refs);
    let duration = start.elapsed();

    #[cfg(feature = "metrics")]
    {
        METRICS.record_query_duration(duration);
        if out.is_err() {
            METRICS.record_query_error();
        }
    }

    out
}

fn values_to_owned(values: &sea_query::Values) -> Result<Vec<OwnedParam>, LifeError> {
    values
        .0
        .iter()
        .map(OwnedParam::try_from)
        .collect()
}

/// [`LifeExecutor`] that dispatches through a [`LifeguardPool`].
///
/// Use [`LifeExecutor::execute_values`], [`LifeExecutor::query_one_values`], and
/// [`LifeExecutor::query_all_values`] (or ORM methods built on them). Raw
/// `execute` / `query_*` with non-empty `&[&dyn ToSql]` are rejected because those
/// references cannot cross the pool channel.
#[derive(Clone)]
pub struct PooledLifeExecutor {
    pool: Arc<LifeguardPool>,
}

impl PooledLifeExecutor {
    #[must_use]
    pub fn new(pool: Arc<LifeguardPool>) -> Self {
        Self { pool }
    }

    #[must_use]
    pub fn pool(&self) -> &Arc<LifeguardPool> {
        &self.pool
    }
}

impl LifeExecutor for PooledLifeExecutor {
    fn execute(&self, query: &str, params: &[&dyn ToSql]) -> Result<u64, LifeError> {
        if params.is_empty() {
            return self.execute_values(query, &sea_query::Values(Vec::new()));
        }
        Err(LifeError::Pool(
            "PooledLifeExecutor: use execute_values(query, &sea_query::Values) or ORM APIs; dynamic &dyn ToSql cannot cross the pool channel".to_string(),
        ))
    }

    fn query_one(&self, query: &str, params: &[&dyn ToSql]) -> Result<Row, LifeError> {
        if params.is_empty() {
            return self.query_one_values(query, &sea_query::Values(Vec::new()));
        }
        Err(LifeError::Pool(
            "PooledLifeExecutor: use query_one_values(query, &sea_query::Values) or ORM APIs; dynamic &dyn ToSql cannot cross the pool channel".to_string(),
        ))
    }

    fn query_all(&self, query: &str, params: &[&dyn ToSql]) -> Result<Vec<Row>, LifeError> {
        if params.is_empty() {
            return self.query_all_values(query, &sea_query::Values(Vec::new()));
        }
        Err(LifeError::Pool(
            "PooledLifeExecutor: use query_all_values(query, &sea_query::Values) or ORM APIs; dynamic &dyn ToSql cannot cross the pool channel".to_string(),
        ))
    }

    fn execute_values(
        &self,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<u64, LifeError> {
        let params = values_to_owned(values)?;
        let query = query.to_string();
        self.pool.dispatch(|reply| WorkerJob::Execute {
            query,
            params,
            reply,
        })
    }

    fn query_one_values(
        &self,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<Row, LifeError> {
        let params = values_to_owned(values)?;
        let query = query.to_string();
        self.pool.dispatch(|reply| WorkerJob::QueryOne {
            query,
            params,
            reply,
        })
    }

    fn query_all_values(
        &self,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<Vec<Row>, LifeError> {
        let params = values_to_owned(values)?;
        let query = query.to_string();
        self.pool.dispatch(|reply| WorkerJob::QueryAll {
            query,
            params,
            reply,
        })
    }
}
