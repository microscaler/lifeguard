//! Connection pool with **one OS thread per slot** (SPSC per slot, MPSC overall).
//!
//! Workers use dedicated threads so dispatchers can block on [`std::sync::mpsc`] replies without
//! relying on the `may` scheduler to run pool consumers (see PRD / integration tests).
//!
//! Each worker owns one [`may_postgres::Client`] and drains a dedicated **bounded**
//! [`crossbeam_channel`] queue. Saturated workers apply [`LifeguardPoolSettings::acquire_timeout`]
//! when enqueueing jobs (PRD P0: bounded queues + acquire timeout).
//!
//! ## Routing (transparent to typical callers)
//!
//! - **Writes** ([`LifeExecutor::execute_values`]) always use the **primary** pool.
//! - **Reads** ([`LifeExecutor::query_one_values`] / [`LifeExecutor::query_all_values`]) use the
//!   **replica** pool when replica URLs and a non-zero replica pool size are configured **and**
//!   [`crate::pool::wal::WalLagMonitor`] reports the replica is not lagging; otherwise reads use
//!   the primary pool.

use crate::connection::connect;
use crate::executor::{LifeError, LifeExecutor};
use crate::pool::config::{DatabaseConfig, LifeguardPoolSettings};
use crate::pool::owned_param::OwnedParam;
use crate::pool::wal::WalLagMonitor;
use crossbeam_channel::SendTimeoutError;
use may_postgres::types::ToSql;
use may_postgres::{Client, Row};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(feature = "tracing")]
use crate::metrics::tracing_helpers;
#[cfg(feature = "metrics")]
use crate::metrics::METRICS;

/// One tier of workers (primary or replica) with round-robin dispatch.
struct WorkerPool {
    worker_txs: Arc<[crossbeam_channel::Sender<WorkerJob>]>,
    next_worker: AtomicUsize,
    pool_size: usize,
    acquire_timeout: Duration,
}

impl WorkerPool {
    fn new(
        pool_size: usize,
        mut url_for_slot: impl FnMut(usize) -> String,
        role: &str,
        settings: &LifeguardPoolSettings,
    ) -> Result<Self, LifeError> {
        if pool_size == 0 {
            return Err(LifeError::Pool(format!(
                "LifeguardPool worker tier ({role}): pool_size must be at least 1"
            )));
        }

        let mut txs = Vec::with_capacity(pool_size);
        let qcap = settings.job_queue_capacity_per_worker;

        for slot in 0..pool_size {
            let url = url_for_slot(slot);
            let client = connect(&url).map_err(|e| {
                LifeError::Other(format!("{role} pool connection slot {slot}: {e}"))
            })?;

            let (job_tx, job_rx) = crossbeam_channel::bounded::<WorkerJob>(qcap);

            let name = format!("lifeguard-pool-{role}-{slot}");
            let handle = thread::Builder::new()
                .name(name)
                .spawn(move || {
                    run_worker(client, job_rx);
                })
                .map_err(|e| LifeError::Other(format!("{role} pool worker thread {slot}: {e}")))?;
            #[allow(clippy::mem_forget)] // Workers must outlive the pool handle.
            std::mem::forget(handle);

            txs.push(job_tx);
        }

        let worker_txs: Arc<[crossbeam_channel::Sender<WorkerJob>]> = txs.into();

        Ok(Self {
            worker_txs,
            next_worker: AtomicUsize::new(0),
            pool_size,
            acquire_timeout: settings.acquire_timeout,
        })
    }

    fn pick_worker(&self) -> &crossbeam_channel::Sender<WorkerJob> {
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
        let deadline = wait_start + self.acquire_timeout;
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
        let job = build(reply_tx);
        let tx = self.pick_worker();

        let mut current_job = job;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(LifeError::PoolAcquireTimeout {
                    waited: wait_start.elapsed(),
                });
            }
            let slice = deadline.saturating_duration_since(now);
            match tx.send_timeout(current_job, slice) {
                Ok(()) => break,
                Err(SendTimeoutError::Timeout(j)) => {
                    current_job = j;
                }
                Err(SendTimeoutError::Disconnected(_)) => {
                    return Err(LifeError::Pool(
                        "pool worker queue disconnected".to_string(),
                    ));
                }
            }
        }

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

/// Pool of `PostgreSQL` connections with **primary** workers for writes and optional **replica**
/// workers for reads when lag allows.
///
/// # Which constructor?
///
/// | Entry point | Use when |
/// |-------------|----------|
/// | [`LifeguardPool::new`] | Tests and embedders that want **defaults** (30s acquire timeout, queue depth 8, 500ms WAL poll). |
/// | [`LifeguardPool::new_with_settings`] | Full control without loading `DatabaseConfig` from disk/env. |
/// | [`LifeguardPool::from_database_config`] | **`config/config.toml`** + optional **`LIFEGUARD__DATABASE__*`** env overrides (see [`DatabaseConfig::load`](crate::DatabaseConfig::load)). |
///
/// Default **acquire timeout** is **30 seconds**, aligned with [`DatabaseConfig::default`](crate::DatabaseConfig) **`pool_timeout_seconds`** and [`LifeguardPoolSettings::default`](crate::LifeguardPoolSettings).
pub struct LifeguardPool {
    primary: WorkerPool,
    /// Present only when `replica_pool_size > 0` and `replica_urls` is non-empty.
    replicas: Option<WorkerPool>,
    /// When reads should not use replicas (lag, monitor absent, or no replica tier).
    wal_monitor: Option<WalLagMonitor>,
}

impl LifeguardPool {
    /// Open pools with default [`LifeguardPoolSettings`] (30s acquire timeout, queue depth 8 per worker).
    ///
    /// For file/env-driven timeouts and queue depth, use [`Self::new_with_settings`] or
    /// [`Self::from_database_config`].
    ///
    /// # Errors
    ///
    /// Returns [`LifeError::Pool`] for invalid size/URL combinations, [`LifeError::PoolAcquireTimeout`]
    /// when no worker accepts a job within the acquire timeout, or [`LifeError::Other`] when a
    /// connection slot fails (see [`connect`]).
    pub fn new(
        primary_url: &str,
        primary_pool_size: usize,
        replica_urls: Vec<String>,
        replica_pool_size: usize,
    ) -> Result<Self, LifeError> {
        Self::new_with_settings(
            primary_url,
            primary_pool_size,
            replica_urls,
            replica_pool_size,
            &LifeguardPoolSettings::default(),
        )
    }

    /// Open pools: `primary_pool_size` connections to `primary_url`, and optionally
    /// `replica_pool_size` connections spread round-robin across `replica_urls`.
    ///
    /// Routing is internal: [`PooledLifeExecutor`] sends mutations to the primary tier and
    /// `query_*_values` to the replica tier when configured and [`WalLagMonitor`] reports the
    /// replica is acceptable; otherwise queries use the primary tier.
    ///
    /// # Replica configuration rules
    ///
    /// - `replica_urls` empty and `replica_pool_size == 0`: primary-only (all traffic to primary).
    /// - Non-empty `replica_urls` requires `replica_pool_size >= 1`.
    /// - `replica_pool_size >= 1` requires non-empty `replica_urls`.
    pub fn new_with_settings(
        primary_url: &str,
        primary_pool_size: usize,
        replica_urls: Vec<String>,
        replica_pool_size: usize,
        settings: &LifeguardPoolSettings,
    ) -> Result<Self, LifeError> {
        let primary = WorkerPool::new(
            primary_pool_size,
            |_| primary_url.to_string(),
            "primary",
            settings,
        )?;

        let replica_urls_trimmed: Vec<String> = replica_urls
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let has_replica_tier = !replica_urls_trimmed.is_empty() && replica_pool_size > 0;

        if !replica_urls_trimmed.is_empty() && replica_pool_size == 0 {
            return Err(LifeError::Pool(
                "LifeguardPool::new: replica_urls is non-empty but replica_pool_size is 0"
                    .to_string(),
            ));
        }
        if replica_urls_trimmed.is_empty() && replica_pool_size > 0 {
            return Err(LifeError::Pool(
                "LifeguardPool::new: replica_pool_size > 0 requires at least one replica URL"
                    .to_string(),
            ));
        }

        let (replicas, wal_monitor) = if has_replica_tier {
            let urls = replica_urls_trimmed.clone();
            let rp = WorkerPool::new(
                replica_pool_size,
                move |slot| urls[slot % urls.len()].clone(),
                "replica",
                settings,
            )?;
            let monitor_url = replica_urls_trimmed[0].clone();
            let wal = WalLagMonitor::start_monitor_with_poll_interval(
                monitor_url,
                settings.wal_lag_poll_interval,
            );
            (Some(rp), Some(wal))
        } else {
            (None, None)
        };

        #[cfg(feature = "metrics")]
        {
            let total = primary_pool_size + replica_pool_size;
            METRICS.set_pool_size(total as u64);
            METRICS.set_active_connections(total as u64);
        }

        Ok(Self {
            primary,
            replicas,
            wal_monitor,
        })
    }

    /// Builds a pool from [`DatabaseConfig`] (URL, `max_connections` as primary pool width, timeouts,
    /// per-worker queue depth) plus explicit replica list and replica tier width.
    ///
    /// Prefer this over [`Self::new`] when configuration comes from [`DatabaseConfig::load`] (TOML
    /// `[database]` + `LIFEGUARD__DATABASE__*` env). Replica URLs are **not** in [`DatabaseConfig`];
    /// pass them explicitly for the same reason as [`Self::new_with_settings`].
    ///
    /// # Errors
    ///
    /// Returns [`LifeError::Pool`] if `max_connections` is zero.
    pub fn from_database_config(
        cfg: &DatabaseConfig,
        replica_urls: Vec<String>,
        replica_pool_size: usize,
    ) -> Result<Self, LifeError> {
        if cfg.max_connections == 0 {
            return Err(LifeError::Pool(
                "LifeguardPool::from_database_config: max_connections must be at least 1"
                    .to_string(),
            ));
        }
        let settings = LifeguardPoolSettings::from_database_config(cfg);
        Self::new_with_settings(
            cfg.url.trim(),
            cfg.max_connections,
            replica_urls,
            replica_pool_size,
            &settings,
        )
    }

    /// Workers connected to the primary URL (writes and fallback reads).
    #[must_use]
    pub fn primary_pool_size(&self) -> usize {
        self.primary.pool_size
    }

    /// Workers connected to replica URLs; `0` if this pool is primary-only.
    #[must_use]
    pub fn replica_pool_size(&self) -> usize {
        self.replicas.as_ref().map_or(0, |r| r.pool_size)
    }

    /// Same as [`Self::primary_pool_size`] (historical name for “main” pool width).
    #[must_use]
    pub fn pool_size(&self) -> usize {
        self.primary_pool_size()
    }

    /// `true` when replica workers exist and the lag monitor reports the replica is behind.
    #[must_use]
    pub fn is_replica_lagging(&self) -> bool {
        self.wal_monitor
            .as_ref()
            .map(WalLagMonitor::is_replica_lagging)
            .unwrap_or(true)
    }

    /// Reference to the lag monitor when replica routing is enabled.
    #[must_use]
    pub fn wal_lag_monitor(&self) -> Option<&WalLagMonitor> {
        self.wal_monitor.as_ref()
    }

    fn read_tier(&self) -> &WorkerPool {
        match &self.replicas {
            None => &self.primary,
            Some(replica_pool) => {
                let lagging = self
                    .wal_monitor
                    .as_ref()
                    .map(WalLagMonitor::is_replica_lagging)
                    .unwrap_or(true);
                if lagging {
                    &self.primary
                } else {
                    replica_pool
                }
            }
        }
    }

    fn dispatch_write<T: Send + 'static>(
        &self,
        build: impl FnOnce(std::sync::mpsc::SyncSender<Result<T, LifeError>>) -> WorkerJob,
    ) -> Result<T, LifeError> {
        self.primary.dispatch(build)
    }

    fn dispatch_read<T: Send + 'static>(
        &self,
        build: impl FnOnce(std::sync::mpsc::SyncSender<Result<T, LifeError>>) -> WorkerJob,
    ) -> Result<T, LifeError> {
        self.read_tier().dispatch(build)
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

fn run_worker(client: Client, job_rx: crossbeam_channel::Receiver<WorkerJob>) {
    for job in job_rx.iter() {
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
    values.0.iter().map(OwnedParam::try_from).collect()
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

    fn execute_values(&self, query: &str, values: &sea_query::Values) -> Result<u64, LifeError> {
        let params = values_to_owned(values)?;
        let query = query.to_string();
        self.pool.dispatch_write(|reply| WorkerJob::Execute {
            query,
            params,
            reply,
        })
    }

    fn query_one_values(&self, query: &str, values: &sea_query::Values) -> Result<Row, LifeError> {
        let params = values_to_owned(values)?;
        let query = query.to_string();
        self.pool.dispatch_read(|reply| WorkerJob::QueryOne {
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
        self.pool.dispatch_read(|reply| WorkerJob::QueryAll {
            query,
            params,
            reply,
        })
    }
}
