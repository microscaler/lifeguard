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
//! - Callers can override routing per [`PooledLifeExecutor`] with [`ReadPreference`]:
//!   [`ReadPreference::Primary`] forces reads onto the primary tier (read-your-writes); the default
//!   is [`ReadPreference::Default`] (WAL-based routing above).
//!
//! With the **`metrics`** feature, pool-scoped series use the OpenTelemetry attribute **`pool_tier`**
//! (`primary` \| `replica`); see [`crate::metrics::METRICS`].

use crate::connection::connect;
use crate::executor::{LifeError, LifeExecutor};
use crate::pool::config::{DatabaseConfig, LifeguardPoolSettings};
use crate::pool::connectivity::life_error_is_connectivity_heal_candidate;
use crate::pool::owned_param::OwnedParam;
use crate::pool::wal::{WalLagMonitor, WalLagPolicy};
use crossbeam_channel::{RecvTimeoutError, SendTimeoutError};
use may_postgres::types::ToSql;
use may_postgres::{Client, Row};
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(feature = "tracing")]
use crate::metrics::tracing_helpers;
#[cfg(feature = "metrics")]
use crate::metrics::METRICS;

/// Where [`PooledLifeExecutor`] should send **read** queries (`query_one_values` / `query_all_values`).
///
/// Writes always use the primary tier regardless of this value.
///
/// # When to use [`ReadPreference::Primary`]
///
/// After your code **writes** and then **reads** through the same logical pool, the replica may not
/// have applied WAL yet. Use [`PooledLifeExecutor::with_read_preference`] with [`ReadPreference::Primary`]
/// for those reads so they hit the primary and see your own commit. Leave [`ReadPreference::Default`]
/// for read-mostly paths where slightly stale reads are acceptable and you want load on standbys.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{LifeguardPool, PooledLifeExecutor, ReadPreference};
/// use std::sync::Arc;
///
/// # fn take_pool() -> Arc<LifeguardPool> { todo!() }
/// let pool = take_pool();
/// let exec = PooledLifeExecutor::new(pool).with_read_preference(ReadPreference::Primary);
/// let _ = exec.read_preference();
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ReadPreference {
    /// Route reads using [`LifeguardPool`]'s built-in policy: replica tier when configured and WAL
    /// lag allows, otherwise primary.
    #[default]
    Default,
    /// Always read from the **primary** tier (strong consistency, read-your-writes after a write
    /// on the same pool).
    Primary,
}

/// One tier of workers (primary or replica) with round-robin dispatch.
struct WorkerPool {
    worker_txs: Arc<[crossbeam_channel::Sender<WorkerJob>]>,
    /// One mutex per slot: held for the duration of each dispatched job, or for the whole lifetime
    /// of [`ExclusivePrimaryLifeExecutor`] (U-4 pin-slot) so other dispatchers block on that slot.
    slot_locks: Arc<[Mutex<()>]>,
    next_worker: AtomicUsize,
    pool_size: usize,
    acquire_timeout: Duration,
    /// `primary` or `replica` for [`crate::metrics::METRICS`] `pool_tier` labels.
    metrics_tier: &'static str,
}

impl WorkerPool {
    fn new(
        pool_size: usize,
        mut url_for_slot: impl FnMut(usize) -> String,
        tier: &'static str,
        settings: &LifeguardPoolSettings,
    ) -> Result<Self, LifeError> {
        if pool_size == 0 {
            return Err(LifeError::Pool(format!(
                "LifeguardPool worker tier ({tier}): pool_size must be at least 1"
            )));
        }

        let mut txs = Vec::with_capacity(pool_size);
        let qcap = settings.job_queue_capacity_per_worker;

        let idle = settings.idle_liveness_interval;
        for slot in 0..pool_size {
            let url = url_for_slot(slot);
            let client = connect(&url).map_err(|e| {
                LifeError::Other(format!("{tier} pool connection slot {slot}: {e}"))
            })?;

            let (job_tx, job_rx) = crossbeam_channel::bounded::<WorkerJob>(qcap);

            let max_lifetime = settings.max_connection_lifetime;
            let lifetime_jitter = settings.max_connection_lifetime_jitter;
            let name = format!("lifeguard-pool-{tier}-{slot}");
            let handle = thread::Builder::new()
                .name(name)
                .spawn(move || {
                    run_worker(WorkerThreadStart {
                        connection_string: url,
                        client,
                        job_rx,
                        idle_liveness: idle,
                        max_connection_lifetime: max_lifetime,
                        max_connection_lifetime_jitter: lifetime_jitter,
                        slot,
                        tier,
                    });
                })
                .map_err(|e| LifeError::Other(format!("{tier} pool worker thread {slot}: {e}")))?;
            #[allow(clippy::mem_forget)] // Workers must outlive the pool handle.
            std::mem::forget(handle);

            txs.push(job_tx);
        }

        let worker_txs: Arc<[crossbeam_channel::Sender<WorkerJob>]> = txs.into();

        let slot_locks: Vec<Mutex<()>> = (0..pool_size).map(|_| Mutex::new(())).collect();
        let slot_locks: Arc<[Mutex<()>]> = slot_locks.into_boxed_slice().into();

        Ok(Self {
            worker_txs,
            slot_locks,
            next_worker: AtomicUsize::new(0),
            pool_size,
            acquire_timeout: settings.acquire_timeout,
            metrics_tier: tier,
        })
    }

    fn pick_worker_index(&self) -> usize {
        self.next_worker.fetch_add(1, Ordering::Relaxed) % self.pool_size
    }

    fn dispatch<T: Send + 'static>(
        &self,
        build: impl FnOnce(std::sync::mpsc::SyncSender<Result<T, LifeError>>) -> WorkerJob,
    ) -> Result<T, LifeError> {
        let slot = self.pick_worker_index();
        let _slot_guard = self.slot_locks[slot]
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.dispatch_locked(slot, build)
    }

    /// Dispatch to `slot` without acquiring [`Self::slot_locks`]. Caller must already hold the
    /// mutex for `slot` (see [`ExclusivePrimaryLifeExecutor`]).
    fn dispatch_locked<T: Send + 'static>(
        &self,
        slot: usize,
        build: impl FnOnce(std::sync::mpsc::SyncSender<Result<T, LifeError>>) -> WorkerJob,
    ) -> Result<T, LifeError> {
        if slot >= self.pool_size {
            return Err(LifeError::Pool(format!(
                "internal pool error: slot {slot} out of range (pool_size {})",
                self.pool_size
            )));
        }
        self.dispatch_on_sender(&self.worker_txs[slot], build)
    }

    fn dispatch_on_sender<T: Send + 'static>(
        &self,
        tx: &crossbeam_channel::Sender<WorkerJob>,
        build: impl FnOnce(std::sync::mpsc::SyncSender<Result<T, LifeError>>) -> WorkerJob,
    ) -> Result<T, LifeError> {
        #[cfg(feature = "tracing")]
        let _span = tracing_helpers::acquire_connection_span().entered();

        let wait_start = Instant::now();
        let deadline = wait_start + self.acquire_timeout;
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
        let job = build(reply_tx);

        let mut current_job = job;
        loop {
            let now = Instant::now();
            if now >= deadline {
                #[cfg(feature = "metrics")]
                METRICS.record_pool_acquire_timeout(self.metrics_tier);
                return Err(LifeError::PoolAcquireTimeout {
                    waited: wait_start.elapsed(),
                });
            }
            let slice = deadline.saturating_duration_since(now);
            // Stamp when we *attempt* enqueue; successful send leaves the job in the queue with this
            // instant so the worker can measure queue wait (time behind prior jobs), not just
            // `send_timeout` blocking on a full queue.
            current_job = current_job.with_enqueued_at(Instant::now());
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
            let wal_policy = WalLagPolicy::from_pool_settings(settings);
            let wal = WalLagMonitor::start_monitor_with_poll_interval(
                monitor_url,
                settings.wal_lag_poll_interval,
                wal_policy,
                settings.wal_lag_monitor_max_connect_retries,
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
            METRICS.set_pool_workers_by_tier(primary_pool_size as u64, replica_pool_size as u64);
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

    /// `true` when the WAL lag monitor **gave up** connecting to the replica (PRD R7.3). Reads use the
    /// primary tier until process restart.
    #[must_use]
    pub fn is_replica_routing_disabled(&self) -> bool {
        self.wal_monitor
            .as_ref()
            .map(WalLagMonitor::is_replica_routing_disabled)
            .unwrap_or(false)
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

    fn read_pool_for(&self, preference: ReadPreference) -> &WorkerPool {
        match preference {
            ReadPreference::Primary => &self.primary,
            ReadPreference::Default => self.read_tier(),
        }
    }

    fn dispatch_write<T: Send + 'static>(
        &self,
        build: impl FnOnce(std::sync::mpsc::SyncSender<Result<T, LifeError>>) -> WorkerJob,
    ) -> Result<T, LifeError> {
        self.primary.dispatch(build)
    }

    fn dispatch_read_with_preference<T: Send + 'static>(
        &self,
        preference: ReadPreference,
        build: impl FnOnce(std::sync::mpsc::SyncSender<Result<T, LifeError>>) -> WorkerJob,
    ) -> Result<T, LifeError> {
        self.read_pool_for(preference).dispatch(build)
    }

    /// Pin one primary worker slot for a multi-statement unit of work (for example `BEGIN` → ORM
    /// work → `COMMIT`). While the returned [`ExclusivePrimaryLifeExecutor`] is alive, other
    /// dispatchers that target the same slot block on the per-slot mutex, so work on this handle is
    /// not interleaved with unrelated jobs on that connection.
    pub fn exclusive_primary_write_executor(
        &self,
    ) -> Result<ExclusivePrimaryLifeExecutor<'_>, LifeError> {
        let slot = self.primary.pick_worker_index();
        let _guard = self.primary.slot_locks[slot]
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Ok(ExclusivePrimaryLifeExecutor {
            pool: self,
            slot,
            _guard,
        })
    }
}

/// Pins one primary [`LifeguardPool`] worker slot: every [`LifeExecutor`] call uses the same
/// underlying client until this value is dropped (PRD U-4).
///
/// Both reads and writes go through the **primary** tier (no replica routing), which matches
/// PostgreSQL transaction semantics for `BEGIN`/`COMMIT` on a single connection.
pub struct ExclusivePrimaryLifeExecutor<'a> {
    pool: &'a LifeguardPool,
    slot: usize,
    _guard: MutexGuard<'a, ()>,
}

impl fmt::Debug for ExclusivePrimaryLifeExecutor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExclusivePrimaryLifeExecutor")
            .field("slot", &self.slot)
            .finish_non_exhaustive()
    }
}

impl LifeExecutor for ExclusivePrimaryLifeExecutor<'_> {
    fn execute(&self, query: &str, params: &[&dyn ToSql]) -> Result<u64, LifeError> {
        if params.is_empty() {
            return self.execute_values(query, &sea_query::Values(Vec::new()));
        }
        Err(LifeError::Pool(
            "ExclusivePrimaryLifeExecutor: use execute_values(query, &sea_query::Values) or ORM APIs; dynamic &dyn ToSql cannot cross the pool channel".to_string(),
        ))
    }

    fn query_one(&self, query: &str, params: &[&dyn ToSql]) -> Result<Row, LifeError> {
        if params.is_empty() {
            return self.query_one_values(query, &sea_query::Values(Vec::new()));
        }
        Err(LifeError::Pool(
            "ExclusivePrimaryLifeExecutor: use query_one_values(query, &sea_query::Values) or ORM APIs; dynamic &dyn ToSql cannot cross the pool channel".to_string(),
        ))
    }

    fn query_all(&self, query: &str, params: &[&dyn ToSql]) -> Result<Vec<Row>, LifeError> {
        if params.is_empty() {
            return self.query_all_values(query, &sea_query::Values(Vec::new()));
        }
        Err(LifeError::Pool(
            "ExclusivePrimaryLifeExecutor: use query_all_values(query, &sea_query::Values) or ORM APIs; dynamic &dyn ToSql cannot cross the pool channel".to_string(),
        ))
    }

    fn execute_values(&self, query: &str, values: &sea_query::Values) -> Result<u64, LifeError> {
        let params = values_to_owned(values)?;
        let query = query.to_string();
        self.pool
            .primary
            .dispatch_locked(self.slot, |reply| WorkerJob::Execute {
                enqueued_at: Instant::now(),
                query,
                params,
                reply,
            })
    }

    fn query_one_values(&self, query: &str, values: &sea_query::Values) -> Result<Row, LifeError> {
        let params = values_to_owned(values)?;
        let query = query.to_string();
        self.pool
            .primary
            .dispatch_locked(self.slot, |reply| WorkerJob::QueryOne {
                enqueued_at: Instant::now(),
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
        self.pool
            .primary
            .dispatch_locked(self.slot, |reply| WorkerJob::QueryAll {
                enqueued_at: Instant::now(),
                query,
                params,
                reply,
            })
    }
}

/// One reconnect attempt after a connectivity-class failure (PRD R5.2).
const POOL_HEAL_MAX_ATTEMPTS: usize = 2;

fn exec_with_optional_heal<T>(
    connection_string: &str,
    client: &mut Client,
    tier: &'static str,
    op: impl Fn(&Client) -> Result<T, LifeError>,
) -> Result<T, LifeError> {
    for attempt in 0..POOL_HEAL_MAX_ATTEMPTS {
        match op(client) {
            Ok(v) => return Ok(v),
            Err(e) => {
                let can_heal = life_error_is_connectivity_heal_candidate(&e)
                    && attempt + 1 < POOL_HEAL_MAX_ATTEMPTS;
                if can_heal {
                    #[cfg(feature = "tracing")]
                    let _heal_span = tracing_helpers::pool_slot_heal_span().entered();
                    match connect(connection_string) {
                        Ok(c) => {
                            *client = c;
                            #[cfg(feature = "metrics")]
                            METRICS.record_pool_slot_heal(tier);
                            log::warn!(
                                "lifeguard pool: replaced client after connectivity error (attempt {})",
                                attempt + 1
                            );
                            continue;
                        }
                        Err(_) => return Err(e),
                    }
                } else {
                    return Err(e);
                }
            }
        }
    }
    Err(LifeError::Pool(
        "lifeguard pool: internal heal loop exhausted".into(),
    ))
}

enum WorkerJob {
    Execute {
        /// When this job was last stamped for enqueue (`dispatch`); used for queue-wait metrics.
        enqueued_at: Instant,
        query: String,
        params: Vec<OwnedParam>,
        reply: std::sync::mpsc::SyncSender<Result<u64, LifeError>>,
    },
    QueryOne {
        enqueued_at: Instant,
        query: String,
        params: Vec<OwnedParam>,
        reply: std::sync::mpsc::SyncSender<Result<Row, LifeError>>,
    },
    QueryAll {
        enqueued_at: Instant,
        query: String,
        params: Vec<OwnedParam>,
        reply: std::sync::mpsc::SyncSender<Result<Vec<Row>, LifeError>>,
    },
}

impl WorkerJob {
    fn with_enqueued_at(self, at: Instant) -> Self {
        match self {
            WorkerJob::Execute {
                query,
                params,
                reply,
                ..
            } => WorkerJob::Execute {
                enqueued_at: at,
                query,
                params,
                reply,
            },
            WorkerJob::QueryOne {
                query,
                params,
                reply,
                ..
            } => WorkerJob::QueryOne {
                enqueued_at: at,
                query,
                params,
                reply,
            },
            WorkerJob::QueryAll {
                query,
                params,
                reply,
                ..
            } => WorkerJob::QueryAll {
                enqueued_at: at,
                query,
                params,
                reply,
            },
        }
    }
}

/// Arguments for [`run_worker`], grouped so the worker entry point stays within Clippy argument limits.
struct WorkerThreadStart {
    connection_string: String,
    client: Client,
    job_rx: crossbeam_channel::Receiver<WorkerJob>,
    idle_liveness: Option<Duration>,
    max_connection_lifetime: Option<Duration>,
    max_connection_lifetime_jitter: Duration,
    slot: usize,
    tier: &'static str,
}

fn run_worker(w: WorkerThreadStart) {
    let WorkerThreadStart {
        connection_string,
        mut client,
        job_rx,
        idle_liveness,
        max_connection_lifetime,
        max_connection_lifetime_jitter,
        slot,
        tier,
    } = w;

    let mut opened_at = Instant::now();
    let idle = idle_liveness.map(|d| d.max(Duration::from_millis(1)));

    match idle {
        None => {
            for job in job_rx.iter() {
                dispatch_worker_job(tier, &connection_string, &mut client, job);
                maybe_rotate_for_max_lifetime(
                    &connection_string,
                    &mut client,
                    &mut opened_at,
                    max_connection_lifetime,
                    max_connection_lifetime_jitter,
                    slot,
                    tier,
                );
            }
        }
        Some(interval) => {
            loop {
                match job_rx.recv_timeout(interval) {
                    Ok(job) => {
                        dispatch_worker_job(tier, &connection_string, &mut client, job);
                        maybe_rotate_for_max_lifetime(
                            &connection_string,
                            &mut client,
                            &mut opened_at,
                            max_connection_lifetime,
                            max_connection_lifetime_jitter,
                            slot,
                            tier,
                        );
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        idle_liveness_probe(&connection_string, &mut client, tier);
                        maybe_rotate_for_max_lifetime(
                            &connection_string,
                            &mut client,
                            &mut opened_at,
                            max_connection_lifetime,
                            max_connection_lifetime_jitter,
                            slot,
                            tier,
                        );
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        }
    }
}

/// Per-slot jitter on top of base max lifetime (PRD R3.1).
///
/// `salt` must be **stable** for the lifetime of a connection (we use the worker `slot` index).
/// Do not mix in time-varying values: the limit is compared on every job/idle tick, and an unstable
/// salt would change the threshold each call and break rotation timing.
fn connection_lifetime_effective_limit(
    base: Duration,
    jitter: Duration,
    salt: usize,
) -> Duration {
    if jitter.is_zero() {
        return base;
    }
    let j = (salt as u128).wrapping_mul(1_100_003) % (jitter.as_millis() + 1);
    base + Duration::from_millis(u64::try_from(j).unwrap_or(0))
}

fn maybe_rotate_for_max_lifetime(
    connection_string: &str,
    client: &mut Client,
    opened_at: &mut Instant,
    max_lifetime: Option<Duration>,
    jitter: Duration,
    slot: usize,
    tier: &'static str,
) {
    let Some(base) = max_lifetime else {
        return;
    };
    if base.is_zero() {
        return;
    }
    let limit = connection_lifetime_effective_limit(base, jitter, slot);
    if opened_at.elapsed() < limit {
        return;
    }
    match connect(connection_string) {
        Ok(c) => {
            *client = c;
            *opened_at = Instant::now();
            log::debug!(
                "lifeguard pool: rotated client after max_connection_lifetime (slot {slot})",
            );
            #[cfg(feature = "metrics")]
            METRICS.record_pool_connection_rotated(tier);
        }
        Err(e) => {
            log::warn!(
                "lifeguard pool: max_connection_lifetime rotation failed (slot {slot}): {e}",
            );
        }
    }
}

/// Cheap `SELECT 1` on idle slots (PRD R4.2); connectivity failures use the same heal path as queries.
fn idle_liveness_probe(connection_string: &str, client: &mut Client, tier: &'static str) {
    let _ = exec_with_optional_heal(connection_string, client, tier, |c| {
        c.query_one("SELECT 1", &[]).map_err(LifeError::from)
    });
}

fn dispatch_worker_job(
    tier: &'static str,
    connection_string: &str,
    client: &mut Client,
    job: WorkerJob,
) {
    #[cfg(feature = "metrics")]
    {
        let enqueued_at = match &job {
            WorkerJob::Execute { enqueued_at, .. }
            | WorkerJob::QueryOne { enqueued_at, .. }
            | WorkerJob::QueryAll { enqueued_at, .. } => *enqueued_at,
        };
        METRICS.record_connection_wait(enqueued_at.elapsed(), Some(tier));
    }

    match job {
        WorkerJob::Execute {
            query,
            params,
            reply,
            ..
        } => {
            let result = exec_with_optional_heal(connection_string, client, tier, |c| {
                exec_on_client(tier, c, &query, &params, |c, q, r| {
                    c.execute(q, r).map_err(LifeError::from)
                })
            });
            let _ = reply.send(result);
        }
        WorkerJob::QueryOne {
            query,
            params,
            reply,
            ..
        } => {
            let result = exec_with_optional_heal(connection_string, client, tier, |c| {
                exec_on_client(tier, c, &query, &params, |c, q, r| {
                    c.query_one(q, r).map_err(LifeError::from)
                })
            });
            let _ = reply.send(result);
        }
        WorkerJob::QueryAll {
            query,
            params,
            reply,
            ..
        } => {
            let result = exec_with_optional_heal(connection_string, client, tier, |c| {
                exec_on_client(tier, c, &query, &params, |c, q, r| {
                    c.query(q, r).map_err(LifeError::from)
                })
            });
            let _ = reply.send(result);
        }
    }
}

fn exec_on_client<T>(
    pool_tier: &'static str,
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
        METRICS.record_query_duration(duration, Some(pool_tier));
        if out.is_err() {
            METRICS.record_query_error(Some(pool_tier));
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
///
/// **Read routing:** by default, reads may go to a configured replica when WAL lag allows; see
/// [`ReadPreference`] and [`Self::with_read_preference`]. Writes always use the primary tier.
#[derive(Clone)]
pub struct PooledLifeExecutor {
    pool: Arc<LifeguardPool>,
    read_preference: ReadPreference,
}

impl PooledLifeExecutor {
    #[must_use]
    pub fn new(pool: Arc<LifeguardPool>) -> Self {
        Self {
            pool,
            read_preference: ReadPreference::default(),
        }
    }

    fn dispatch_read<T: Send + 'static>(
        &self,
        build: impl FnOnce(std::sync::mpsc::SyncSender<Result<T, LifeError>>) -> WorkerJob,
    ) -> Result<T, LifeError> {
        self.pool
            .dispatch_read_with_preference(self.read_preference, build)
    }

    #[must_use]
    pub fn pool(&self) -> &Arc<LifeguardPool> {
        &self.pool
    }

    /// Returns a copy of this executor with the given [`ReadPreference`].
    ///
    /// Use [`ReadPreference::Primary`] when you need **read-your-writes** (e.g. insert then select
    /// on the same request). [`ReadPreference::Default`] restores pool policy (replica when allowed).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use lifeguard::{LifeguardPool, LifeExecutor, PooledLifeExecutor, ReadPreference};
    /// use sea_query::Values;
    /// use std::sync::Arc;
    ///
    /// # fn take_pool() -> Arc<LifeguardPool> { todo!() }
    /// let base = PooledLifeExecutor::new(take_pool());
    /// let primary_reads = base.with_read_preference(ReadPreference::Primary);
    /// let _ = primary_reads.query_one_values("SELECT 1", &Values(Vec::new()));
    /// ```
    #[must_use]
    pub fn with_read_preference(self, read_preference: ReadPreference) -> Self {
        Self {
            read_preference,
            ..self
        }
    }

    #[must_use]
    pub fn read_preference(&self) -> ReadPreference {
        self.read_preference
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
            enqueued_at: Instant::now(),
            query,
            params,
            reply,
        })
    }

    fn query_one_values(&self, query: &str, values: &sea_query::Values) -> Result<Row, LifeError> {
        let params = values_to_owned(values)?;
        let query = query.to_string();
        self.dispatch_read(|reply| WorkerJob::QueryOne {
            enqueued_at: Instant::now(),
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
        self.dispatch_read(|reply| WorkerJob::QueryAll {
            enqueued_at: Instant::now(),
            query,
            params,
            reply,
        })
    }
}

#[cfg(test)]
mod lifetime_effective_limit_tests {
    use super::connection_lifetime_effective_limit;
    use super::ReadPreference;
    use std::time::Duration;

    #[test]
    fn read_preference_default_matches_variant() {
        assert_eq!(ReadPreference::default(), ReadPreference::Default);
    }

    #[test]
    fn same_slot_same_limit_across_calls() {
        let base = Duration::from_secs(60);
        let jitter = Duration::from_millis(500);
        let a = connection_lifetime_effective_limit(base, jitter, 2);
        let b = connection_lifetime_effective_limit(base, jitter, 2);
        assert_eq!(a, b, "salt must not depend on time; limit must be stable per slot");
    }

    #[test]
    fn zero_jitter_returns_base() {
        let base = Duration::from_secs(30);
        assert_eq!(
            connection_lifetime_effective_limit(base, Duration::ZERO, 99),
            base
        );
    }
}
