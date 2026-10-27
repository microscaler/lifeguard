//! Coroutine-native connection pool: callers use the slot's connection directly and pipeline.
//!
//! ## Design
//!
//! Each slot owns one `PostgreSQL` connection. `may_postgres` already pipelines: every `Client`
//! clone of a connection can have requests in flight at the same time, and responses come back in
//! order on that clone's channel. So a statement runs on the **caller's** coroutine, on a reusable
//! `Client` handle of the least-loaded slot, with no worker thread and no job queue in between.
//! Up to [`LifeguardPoolSettings::job_queue_capacity_per_worker`] statements are in flight per
//! slot; past that, callers wait for capacity, bounded by [`LifeguardPoolSettings::acquire_timeout`].
//!
//! Standalone statements (autocommit) interleave safely on one connection: each one is its own
//! implicit transaction. A **unit** that spans several statements (`BEGIN` … `COMMIT`, and every
//! RLS-scoped statement, which runs `BEGIN; rls_set_session(..); stmt; COMMIT`) must not have
//! other callers' statements interleaved, so it **pins** its slot: it waits for the slot's
//! in-flight statements to finish, holds the slot exclusively, and other callers pick other slots
//! meanwhile. Inside a pinned unit the statements are still pipelined: the four statements of an
//! RLS-scoped query go out in one write and cost one round trip.
//!
//! ## Reply deadline
//!
//! With [`LifeguardPoolSettings::reply_timeout`] set (the default), the statement runs on a
//! short-lived coroutine and the caller waits on it with that deadline. A connection whose I/O
//! never completes (a starved `may` runtime, a half-open socket) therefore yields
//! [`LifeError::PoolReplyTimeout`] instead of hanging the caller. The slot's connection is then
//! replaced on a separate OS thread, never inline on the caller (2 Oct 2026 wedge: an inline
//! reconnect needs the same starved runtime and hung).
//!
//! ## Routing (unchanged)
//!
//! - **Writes** ([`LifeExecutor::execute_values`]) use the **primary** tier.
//! - **Reads** ([`LifeExecutor::query_one_values`] / [`LifeExecutor::query_all_values`]) use the
//!   **replica** tier when replica URLs and a non-zero replica pool size are configured **and**
//!   [`crate::pool::wal::WalLagMonitor`] reports the replica is not lagging; otherwise the primary.
//! - [`ReadPreference::Primary`] on a [`PooledLifeExecutor`] forces reads onto the primary.
//!
//! ## Healing
//!
//! A connectivity-class error (see `connectivity`) replaces the slot's connection and retries the
//! statement once on the fresh one. A background thread per tier runs the idle liveness probe
//! ([`LifeguardPoolSettings::idle_liveness_interval`]) and rotates connections past
//! [`LifeguardPoolSettings::max_connection_lifetime`].
//!
//! With the **`metrics`** feature, pool-scoped series use the attribute **`pool_tier`**
//! (`primary` \| `replica`); see [`crate::metrics::METRICS`].

use crate::connection::connect;
use crate::executor::{LifeError, LifeExecutor, SessionContext};
use crate::metrics::pool_gauges::Track;
use crate::pool::config::{DatabaseConfig, LifeguardPoolSettings};
use crate::pool::connectivity::life_error_is_connectivity_heal_candidate;
use crate::pool::owned_param::OwnedParam;
use crate::pool::stmt_cache::{Lookup, StatementCache};
use crate::pool::wal::{WalLagMonitor, WalLagPolicy};
use crate::value::TextParam;
use may::sync::Semphore;
use may_postgres::types::ToSql;
use may_postgres::{Client, Row};
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(feature = "tracing")]
use crate::metrics::tracing_helpers;
#[cfg(feature = "metrics")]
use crate::metrics::METRICS;

/// Milliseconds since the first call (monotonic, never 0), for cheap atomics-only timestamps.
fn mono_ms() -> u64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64 + 1
}

const RLS_SET_SESSION_SQL: &str = "SELECT public.rls_set_session($1::text, $2::uuid, $3::uuid, $4::text, $5::jsonb, $6::jsonb, $7::text, $8::text)";

/// Client handles kept per connection for reuse (a `Client` clone allocates a response channel and
/// a 64 KiB write buffer, so handles are recycled rather than cloned per statement).
const MAX_IDLE_HANDLES: usize = 256;

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

// ---------------------------------------------------------------------------------------------
// Connection and slot

/// One live connection of a slot: the client it was opened with, recycled handles, and the
/// statements prepared on it.
struct SlotConn {
    /// Behind a mutex only so `SlotConn` is `Sync` (`Client` is `Send`, not `Sync`); it is
    /// locked just long enough to clone a new handle.
    master: Mutex<Client>,
    handles: Mutex<Vec<Client>>,
    stmts: StatementCache,
    opened_at: Instant,
    /// Set when the connection failed or wedged; the slot then replaces it.
    broken: AtomicBool,
    /// Set when the reply-deadline watchdog shut the connection down.
    deadline_closed: AtomicBool,
}

impl SlotConn {
    fn open(url: &str) -> Result<Arc<Self>, LifeError> {
        let master = connect(url).map_err(|e| LifeError::Other(format!("pool connection: {e}")))?;
        Ok(Arc::new(Self {
            master: Mutex::new(master),
            handles: Mutex::new(Vec::new()),
            stmts: StatementCache::new(),
            opened_at: Instant::now(),
            broken: AtomicBool::new(false),
            deadline_closed: AtomicBool::new(false),
        }))
    }

    fn take_handle(&self) -> Client {
        let popped = self
            .handles
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop();
        popped.unwrap_or_else(|| {
            self.master
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
        })
    }

    /// Shut the connection's socket down so its I/O coroutine fails every request waiting on it
    /// (the reply-deadline watchdog's way to wake callers parked on a wedged connection).
    ///
    /// `may_postgres` uses the socket's file descriptor as the connection id. The descriptor is
    /// closed only when the I/O coroutine exits, which also marks the client closed, so it is
    /// checked first; the remaining window between that check and the call is a few instructions
    /// on a path that only runs after a missed deadline.
    fn force_close(&self) {
        self.deadline_closed.store(true, Ordering::Release);
        #[cfg(unix)]
        {
            let master = self
                .master
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if master.is_closed() {
                return;
            }
            if let Ok(fd) = i32::try_from(master.id()) {
                // SAFETY: `shutdown` takes no pointers; on a descriptor that is not a socket it
                // fails with ENOTSOCK and does nothing.
                let rc = unsafe { libc::shutdown(fd, libc::SHUT_RDWR) };
                if rc != 0 {
                    log::warn!(
                        "lifeguard pool: shutting down a wedged connection failed: {}",
                        std::io::Error::last_os_error()
                    );
                }
            }
        }
    }

    fn put_handle(&self, h: Client) {
        if self.broken.load(Ordering::Relaxed) || h.is_closed() {
            return;
        }
        let mut handles = self
            .handles
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if handles.len() < MAX_IDLE_HANDLES {
            handles.push(h);
        }
    }
}

struct Slot {
    index: usize,
    url: String,
    conn: RwLock<Arc<SlotConn>>,
    /// Shared for standalone statements, exclusive for pinned units.
    gate: Gate,
    in_flight: AtomicUsize,
    /// Pinned units holding or waiting for this slot; pickers avoid pinned slots.
    pinned: AtomicUsize,
    replacing: AtomicBool,
    replaced_at_ms: AtomicU64,
    last_used_ms: AtomicU64,
    /// Serialises reconnects of this slot.
    heal_lock: may::sync::Mutex<()>,
    /// Statements running inline on a caller coroutine under the reply deadline (see `guarded`).
    watches: std::sync::Mutex<Vec<(u64, Arc<Watch>)>>,
}

impl Slot {
    fn current(&self) -> Arc<SlotConn> {
        Arc::clone(
            &self
                .conn
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }

    fn swap(&self, fresh: Arc<SlotConn>) {
        let old = std::mem::replace(
            &mut *self
                .conn
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            fresh,
        );
        old.broken.store(true, Ordering::Release);
    }

    /// Replace `seen` with a fresh connection unless someone already did; returns the current one.
    fn heal(&self, seen: &Arc<SlotConn>, tier: &'static str) -> Result<Arc<SlotConn>, LifeError> {
        let _g = self
            .heal_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let cur = self.current();
        if !Arc::ptr_eq(&cur, seen) && !cur.broken.load(Ordering::Acquire) {
            return Ok(cur);
        }
        #[cfg(feature = "tracing")]
        let _heal_span = tracing_helpers::pool_slot_heal_span(); // created, not entered
        let fresh = SlotConn::open(&self.url)?;
        self.swap(Arc::clone(&fresh));
        #[cfg(feature = "metrics")]
        METRICS.record_pool_slot_heal(tier);
        log::warn!(
            "lifeguard pool ({tier}): slot {} replaced its connection after a connectivity error",
            self.index
        );
        let _ = tier;
        Ok(fresh)
    }
}

// ---------------------------------------------------------------------------------------------
// Tier

struct TierInner {
    slots: Vec<Slot>,
    permits: Semphore,
    acquire_timeout: Duration,
    reply_timeout: Option<Duration>,
    idle_liveness: Option<Duration>,
    max_connection_lifetime: Option<Duration>,
    max_connection_lifetime_jitter: Duration,
    tier: &'static str,
    last_timeout_report_ms: AtomicU64,
    timeouts_since_report: AtomicU64,
    next_watch: AtomicU64,
}

/// One tier of slots (primary or replica).
struct WorkerPool {
    inner: Arc<TierInner>,
    pool_size: usize,
}

/// What a statement returns.
enum Out {
    Count(u64),
    One(Row),
    All(Vec<Row>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Execute,
    QueryOne,
    QueryAll,
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
        let mut slots = Vec::with_capacity(pool_size);
        for index in 0..pool_size {
            let url = url_for_slot(index);
            let conn = SlotConn::open(&url).map_err(|e| {
                LifeError::Other(format!("{tier} pool connection slot {index}: {e}"))
            })?;
            slots.push(Slot {
                index,
                url,
                conn: RwLock::new(conn),
                gate: Gate::default(),
                in_flight: AtomicUsize::new(0),
                pinned: AtomicUsize::new(0),
                replacing: AtomicBool::new(false),
                replaced_at_ms: AtomicU64::new(0),
                last_used_ms: AtomicU64::new(mono_ms()),
                heal_lock: may::sync::Mutex::new(()),
                watches: std::sync::Mutex::new(Vec::new()),
            });
        }
        let depth = settings.job_queue_capacity_per_worker.max(1);
        let inner = Arc::new(TierInner {
            slots,
            permits: Semphore::new(pool_size * depth),
            acquire_timeout: settings.acquire_timeout,
            reply_timeout: settings.reply_timeout,
            idle_liveness: settings.idle_liveness_interval,
            max_connection_lifetime: settings.max_connection_lifetime.filter(|d| !d.is_zero()),
            max_connection_lifetime_jitter: settings.max_connection_lifetime_jitter,
            tier,
            last_timeout_report_ms: AtomicU64::new(0),
            timeouts_since_report: AtomicU64::new(0),
            next_watch: AtomicU64::new(0),
        });
        if inner.idle_liveness.is_some()
            || inner.max_connection_lifetime.is_some()
            || inner.reply_timeout.is_some()
        {
            spawn_maintenance(&inner);
        }
        Ok(Self { inner, pool_size })
    }

    /// Run one standalone statement on the least-loaded slot.
    fn run(
        &self,
        kind: Kind,
        sql: Arc<str>,
        params: Vec<OwnedParam>,
        session: Option<Arc<SessionArgs>>,
    ) -> Result<Out, LifeError> {
        if let Some(args) = session {
            return self.run_contextual(kind, sql, params, &args);
        }
        let inner = &*self.inner;
        let started = Instant::now();
        let permit = Permit::acquire(inner, started)?;
        let (index, gate) = inner.enter_shared(started)?;
        let slot = &inner.slots[index];
        #[cfg(feature = "metrics")]
        {
            METRICS.record_pool_checkout(started.elapsed(), inner.tier);
            METRICS.record_connection_wait(started.elapsed(), Some(inner.tier));
        }
        let _busy = InFlight::enter(slot, inner.tier);
        let conn = slot.current();
        let tier_arc = Arc::clone(&self.inner);
        #[cfg(feature = "tracing")]
        let ctx = may_tracing::current();
        let watched = Arc::clone(&conn);
        let result = guarded(&self.inner, index, &watched, move || {
            #[cfg(feature = "tracing")]
            let _ctx = may_tracing::set_current(ctx);
            let refs: Vec<&dyn ToSql> = params.iter().map(OwnedParam::as_sql_ref).collect();
            run_shared_with_heal(&tier_arc, index, conn, &sql, &refs, kind)
        });
        drop(gate);
        drop(permit);
        match result {
            Some(r) => r,
            None => Err(self.inner.on_reply_timeout(index, started, true)),
        }
    }

    /// An RLS-scoped statement: pin a slot and pipeline `BEGIN`, `rls_set_session`, the statement
    /// and `COMMIT`.
    fn run_contextual(
        &self,
        kind: Kind,
        sql: Arc<str>,
        params: Vec<OwnedParam>,
        args: &Arc<SessionArgs>,
    ) -> Result<Out, LifeError> {
        let inner = &*self.inner;
        let started = Instant::now();
        let pin = Pin::acquire(&self.inner)?;
        #[cfg(feature = "metrics")]
        {
            METRICS.record_pool_checkout(started.elapsed(), inner.tier);
            METRICS.record_connection_wait(started.elapsed(), Some(inner.tier));
        }
        let index = pin.index;
        let conn = Arc::clone(&pin.conn);
        let args = Arc::clone(args);
        #[cfg(feature = "tracing")]
        let ctx = may_tracing::current();
        let watched = Arc::clone(&conn);
        let result = guarded(&self.inner, index, &watched, move || {
            #[cfg(feature = "tracing")]
            let _ctx = may_tracing::set_current(ctx);
            let h = conn.take_handle();
            let refs: Vec<&dyn ToSql> = params.iter().map(OwnedParam::as_sql_ref).collect();
            let r = contextual_unit(&h, &conn, &args, &sql, &refs, kind);
            if r.as_ref()
                .is_err_and(life_error_is_connectivity_heal_candidate)
            {
                conn.broken.store(true, Ordering::Release);
            } else {
                conn.put_handle(h);
            }
            r
        });
        let broken = pin.conn.broken.load(Ordering::Acquire);
        drop(pin);
        match result {
            Some(r) => {
                if broken {
                    self.inner.replace_in_background(index, None);
                }
                r
            }
            None => Err(self.inner.on_reply_timeout(index, started, true)),
        }
    }
}

impl TierInner {
    /// Enter a slot for one shared statement: the least-loaded slot whose gate is open. A slot's
    /// gate is closed only while a transaction or RLS unit holds it exclusively; when every slot
    /// is held that way, wait (polling, so a pinned caller that also issues a pooled statement
    /// gets [`LifeError::PoolAcquireTimeout`] instead of waiting on itself forever).
    fn enter_shared(&self, started: Instant) -> Result<(usize, ReadGate<'_>), LifeError> {
        let mut backoff = Duration::from_micros(100);
        loop {
            let index = self.pick_shared()?;
            // A slot with a pin holding or waiting for it is closed to new statements, so a pin
            // waits only for the statements already inside.
            if self.slots[index].pinned.load(Ordering::Acquire) == 0 {
                if let Some(gate) = self.slots[index].gate.try_read() {
                    return Ok((index, gate));
                }
            }
            // Pinned, or a pin is draining it: any other open gate?
            for (i, s) in self.slots.iter().enumerate() {
                if i != index
                    && !s.replacing.load(Ordering::Acquire)
                    && s.pinned.load(Ordering::Acquire) == 0
                {
                    if let Some(gate) = s.gate.try_read() {
                        return Ok((i, gate));
                    }
                }
            }
            if started.elapsed() >= self.acquire_timeout {
                #[cfg(feature = "metrics")]
                METRICS.record_pool_acquire_timeout(self.tier);
                return Err(LifeError::PoolAcquireTimeout {
                    waited: started.elapsed(),
                });
            }
            let _waiting = Track::waiting(self.tier);
            may::coroutine::sleep(backoff);
            backoff = (backoff * 2).min(Duration::from_millis(5));
        }
    }

    /// Least-loaded slot that is not pinned, being replaced, or broken.
    fn pick_shared(&self) -> Result<usize, LifeError> {
        let mut best: Option<(usize, usize)> = None;
        let mut any_serviceable = false;
        for (i, s) in self.slots.iter().enumerate() {
            if s.replacing.load(Ordering::Acquire) {
                continue;
            }
            any_serviceable = true;
            let load =
                s.in_flight.load(Ordering::Relaxed) + s.pinned.load(Ordering::Relaxed) * 1024;
            if best.is_none_or(|(_, l)| load < l) {
                best = Some((i, load));
                if load == 0 {
                    break;
                }
            }
        }
        match best {
            Some((i, _)) => Ok(i),
            None if !any_serviceable => Err(LifeError::Pool(format!(
                "lifeguard pool ({}): all {} slots are being replaced after reply timeouts; no connection is being serviced. \
                 If this persists the may runtime that drives connection I/O is starved - look for OS-blocking calls in coroutines",
                self.tier,
                self.slots.len()
            ))),
            None => Err(LifeError::Pool("internal pool error: no slot".to_string())),
        }
    }

    /// The reply budget elapsed: report, mark the connection wedged and replace it off-thread.
    fn on_reply_timeout(
        self: &Arc<Self>,
        index: usize,
        started: Instant,
        replace: bool,
    ) -> LifeError {
        #[cfg(feature = "metrics")]
        METRICS.record_pool_reply_timeout(self.tier);
        if let Some(limit) = self.reply_timeout {
            log::warn!(
                "lifeguard pool ({}): no reply within {limit:?}; returning bounded error (connection wedged or statement overran the reply budget)",
                self.tier
            );
            self.report_reply_timeout(index, limit);
        }
        if replace {
            let seen = self.slots[index].current();
            seen.broken.store(true, Ordering::Release);
            self.replace_in_background(index, Some(seen));
        }
        LifeError::PoolReplyTimeout {
            waited: started.elapsed(),
        }
    }

    /// One stderr line per 10 s for reply timeouts, with the count since the last line: the
    /// `log` output of embedders can go to an exporter that is not the pod log, and on 2 Oct 2026
    /// seven hours of timeouts left no trace there.
    fn report_reply_timeout(&self, slot: usize, limit: Duration) {
        let n = self.timeouts_since_report.fetch_add(1, Ordering::AcqRel) + 1;
        let now = mono_ms();
        let last = self.last_timeout_report_ms.load(Ordering::Acquire);
        if (last == 0 || now.saturating_sub(last) >= 10_000)
            && self
                .last_timeout_report_ms
                .compare_exchange(last, now, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            self.timeouts_since_report.store(0, Ordering::Release);
            eprintln!(
                "lifeguard pool ({}): {n} job(s) missed the {limit:?} reply budget since the last report (latest on slot {slot}); replacing wedged slots",
                self.tier
            );
        }
    }

    /// Replace the slot's connection on its own OS thread. A connect needs the `may` runtime, the
    /// starvation of which is the usual reason a connection stops answering, so it must never run
    /// on the caller.
    fn replace_in_background(self: &Arc<Self>, index: usize, seen: Option<Arc<SlotConn>>) {
        let slot = &self.slots[index];
        if slot.replacing.swap(true, Ordering::AcqRel) {
            return;
        }
        let tier = self.tier;
        let last = slot.replaced_at_ms.load(Ordering::Acquire);
        let recent = self.reply_timeout.map_or(Duration::from_secs(60), |t| {
            (t * 3).max(Duration::from_secs(60))
        });
        if last != 0 && mono_ms().saturating_sub(last) < recent.as_millis() as u64 {
            let msg = format!(
                "lifeguard pool ({tier}): slot {index} was replaced {}s ago and its FRESH connection missed the reply budget too. \
                 The database is not the problem: the may runtime that drives connection I/O is starved - something is making \
                 OS-blocking calls in coroutines (a crossbeam/std channel wait, a std Mutex held across a park, blocking I/O)",
                mono_ms().saturating_sub(last) / 1000
            );
            eprintln!("{msg}");
            log::error!("{msg}");
        }
        let url = slot.url.clone();
        let tier_arc = Arc::clone(self);
        let started = thread::Builder::new()
            .name(format!("lifeguard-heal-{tier}-{index}"))
            .spawn(move || {
                let slot = &tier_arc.slots[index];
                let t0 = Instant::now();
                let still_seen = seen.as_ref().is_none_or(|s| Arc::ptr_eq(s, &slot.current()));
                if still_seen || slot.current().broken.load(Ordering::Acquire) {
                    match SlotConn::open(&url) {
                        Ok(fresh) => {
                            slot.swap(fresh);
                            slot.replaced_at_ms.store(mono_ms(), Ordering::Release);
                            #[cfg(feature = "metrics")]
                            METRICS.record_pool_slot_heal(tier);
                            let msg = format!(
                                "lifeguard pool ({tier}): slot {index} wedged past the reply budget; replaced with a fresh connection in {:?}",
                                t0.elapsed()
                            );
                            eprintln!("{msg}");
                            log::error!("{msg}");
                        }
                        Err(e) => {
                            let msg = format!(
                                "lifeguard pool ({tier}): slot {index} is wedged and could not be replaced: {e}"
                            );
                            eprintln!("{msg}");
                            log::error!("{msg}");
                        }
                    }
                }
                slot.replacing.store(false, Ordering::Release);
            });
        if let Err(e) = started {
            slot.replacing.store(false, Ordering::Release);
            log::error!(
                "lifeguard pool ({tier}): could not start the replacement thread for slot {index}: {e}"
            );
        }
    }
}

/// Background liveness probes and lifetime rotation for one tier, on an OS thread.
fn spawn_maintenance(inner: &Arc<TierInner>) {
    let weak = Arc::downgrade(inner);
    let tier = inner.tier;
    let tick = inner
        .idle_liveness
        .unwrap_or(Duration::from_secs(1))
        .min(Duration::from_secs(1))
        .min(
            inner
                .reply_timeout
                .map_or(Duration::from_secs(1), |t| t / 4),
        )
        .max(Duration::from_millis(10));
    let _ = thread::Builder::new()
        .name(format!("lifeguard-maint-{tier}"))
        .spawn(move || loop {
            thread::sleep(tick);
            let Some(t) = weak.upgrade() else {
                return;
            };
            for slot in &t.slots {
                if let Some(limit) = t.reply_timeout {
                    expire_watches(slot, limit);
                }
                if slot.replacing.load(Ordering::Acquire) {
                    continue;
                }
                let conn = slot.current();
                if let Some(base) = t.max_connection_lifetime {
                    let limit =
                        connection_lifetime_effective_limit(base, t.max_connection_lifetime_jitter, slot.index);
                    if conn.opened_at.elapsed() >= limit {
                        match SlotConn::open(&slot.url) {
                            Ok(fresh) => {
                                slot.swap(fresh);
                                log::debug!(
                                    "lifeguard pool: rotated client after max_connection_lifetime (slot {})",
                                    slot.index
                                );
                                #[cfg(feature = "metrics")]
                                METRICS.record_pool_connection_rotated(tier);
                            }
                            Err(e) => log::warn!(
                                "lifeguard pool: max_connection_lifetime rotation failed (slot {}): {e}",
                                slot.index
                            ),
                        }
                        continue;
                    }
                }
                if let Some(idle) = t.idle_liveness {
                    let idle_ms = mono_ms().saturating_sub(slot.last_used_ms.load(Ordering::Relaxed));
                    if slot.in_flight.load(Ordering::Relaxed) == 0
                        && slot.pinned.load(Ordering::Relaxed) == 0
                        && u128::from(idle_ms) >= idle.as_millis()
                    {
                        let h = conn.take_handle();
                        let probe = h.batch_execute("SELECT 1").map_err(LifeError::from);
                        slot.last_used_ms.store(mono_ms(), Ordering::Relaxed);
                        match probe {
                            Ok(()) => conn.put_handle(h),
                            Err(e) if life_error_is_connectivity_heal_candidate(&e) => {
                                conn.broken.store(true, Ordering::Release);
                                let _ = slot.heal(&conn, tier);
                            }
                            Err(_) => conn.put_handle(h),
                        }
                    }
                }
            }
        });
}

/// Per-slot jitter on top of base max lifetime (PRD R3.1).
///
/// `salt` must be **stable** for the lifetime of a connection (we use the slot index). Do not mix
/// in time-varying values: the limit is compared on every tick, and an unstable salt would change
/// the threshold each call and break rotation timing.
fn connection_lifetime_effective_limit(base: Duration, jitter: Duration, salt: usize) -> Duration {
    if jitter.is_zero() {
        return base;
    }
    let j = (salt as u128).wrapping_mul(1_100_003) % (jitter.as_millis() + 1);
    base + Duration::from_millis(u64::try_from(j).unwrap_or(0))
}

/// A unit of in-flight capacity on a tier, released on drop.
struct Permit<'a> {
    sem: &'a Semphore,
}

impl<'a> Permit<'a> {
    fn acquire(inner: &'a TierInner, started: Instant) -> Result<Self, LifeError> {
        let _waiting = Track::waiting(inner.tier);
        if inner.permits.wait_timeout(inner.acquire_timeout) {
            Ok(Self {
                sem: &inner.permits,
            })
        } else {
            #[cfg(feature = "metrics")]
            METRICS.record_pool_acquire_timeout(inner.tier);
            Err(LifeError::PoolAcquireTimeout {
                waited: started.elapsed(),
            })
        }
    }
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.sem.post();
    }
}

/// Counts a statement in its slot's load and in the in-flight gauge.
struct InFlight<'a> {
    slot: &'a Slot,
    _track: Track,
}

impl<'a> InFlight<'a> {
    fn enter(slot: &'a Slot, tier: &'static str) -> Self {
        slot.in_flight.fetch_add(1, Ordering::Relaxed);
        Self {
            slot,
            _track: Track::in_flight(tier),
        }
    }
}

impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        self.slot.in_flight.fetch_sub(1, Ordering::Relaxed);
        self.slot.last_used_ms.store(mono_ms(), Ordering::Relaxed);
    }
}

/// A slot's admission gate: many shared statements, or one pinned unit.
///
/// A bare atomic rather than `may::sync::RwLock`: callers never block on it (they poll with
/// backoff under the acquire timeout), and it has no poisoning. A pinned unit runs the caller's
/// closure while it holds the gate, so a panic there unwinds through the hold; a poisoned lock
/// would then fail every later statement on the slot (and `may`'s `try_read` miscounts readers
/// on a poisoned lock).
#[derive(Default)]
struct Gate {
    /// Readers in the low bits; `WRITER` while a pin holds it.
    state: AtomicUsize,
}

const WRITER: usize = 1 << (usize::BITS - 1);

impl Gate {
    fn try_read(&self) -> Option<ReadGate<'_>> {
        let mut s = self.state.load(Ordering::Acquire);
        loop {
            if s & WRITER != 0 {
                return None;
            }
            match self
                .state
                .compare_exchange_weak(s, s + 1, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return Some(ReadGate(self)),
                Err(now) => s = now,
            }
        }
    }

    /// Exclusive hold when no statement is inside; the caller releases it with `write_unlock`.
    fn try_write(&self) -> bool {
        self.state
            .compare_exchange(0, WRITER, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn write_unlock(&self) {
        self.state.fetch_and(!WRITER, Ordering::AcqRel);
    }
}

/// One shared statement inside a slot's gate.
struct ReadGate<'a>(&'a Gate);

impl Drop for ReadGate<'_> {
    fn drop(&mut self) {
        self.0.state.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Exclusive hold of one slot for a multi-statement unit.
struct Pin {
    tier: Arc<TierInner>,
    index: usize,
    conn: Arc<SlotConn>,
    /// This pin holds the slot's gate exclusively (released in `Drop`).
    holds_gate: bool,
    _track: Track,
}

impl Pin {
    fn acquire(tier: &Arc<TierInner>) -> Result<Self, LifeError> {
        let inner = &**tier;
        let started = Instant::now();
        // Claim a slot nobody holds, least loaded first, with a compare-and-swap so that
        // concurrent pins spread over distinct slots instead of queueing on the same one (two
        // transactions that wait on each other would otherwise deadlock on one slot while
        // another sits idle). Only when every slot is held does a pin queue, on the least-held.
        let mut order: Vec<(usize, usize)> = inner
            .slots
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.replacing.load(Ordering::Acquire))
            .map(|(i, s)| (i, s.in_flight.load(Ordering::Relaxed)))
            .collect();
        if order.is_empty() {
            order.push((inner.pick_shared()?, 0));
        }
        order.sort_by_key(|&(_, load)| load);
        let claimed = order.iter().map(|&(i, _)| i).find(|&i| {
            inner.slots[i]
                .pinned
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        });
        let index = claimed.unwrap_or_else(|| {
            let i = order
                .iter()
                .map(|&(i, _)| i)
                .min_by_key(|&i| inner.slots[i].pinned.load(Ordering::Relaxed))
                .unwrap_or(order[0].0);
            inner.slots[i].pinned.fetch_add(1, Ordering::AcqRel);
            i
        });
        let slot = &inner.slots[index];
        let waiting = Track::waiting(inner.tier);
        let mut backoff = Duration::from_micros(100);
        loop {
            if slot.gate.try_write() {
                break;
            }
            if started.elapsed() >= inner.acquire_timeout {
                slot.pinned.fetch_sub(1, Ordering::AcqRel);
                #[cfg(feature = "metrics")]
                METRICS.record_pool_acquire_timeout(inner.tier);
                return Err(LifeError::PoolAcquireTimeout {
                    waited: started.elapsed(),
                });
            }
            may::coroutine::sleep(backoff);
            backoff = (backoff * 2).min(Duration::from_millis(5));
        }
        drop(waiting);
        slot.in_flight.fetch_add(1, Ordering::Relaxed);
        let conn = slot.current();
        Ok(Self {
            tier: Arc::clone(tier),
            index,
            conn,
            holds_gate: true,
            _track: Track::in_flight(inner.tier),
        })
    }
}

impl Drop for Pin {
    fn drop(&mut self) {
        let slot = &self.tier.slots[self.index];
        if std::mem::take(&mut self.holds_gate) {
            slot.gate.write_unlock();
        }
        slot.in_flight.fetch_sub(1, Ordering::Relaxed);
        slot.pinned.fetch_sub(1, Ordering::AcqRel);
        slot.last_used_ms.store(mono_ms(), Ordering::Relaxed);
    }
}

/// A statement running inline under the reply deadline: the watchdog marks it expired.
struct Watch {
    started_ms: u64,
    conn: Arc<SlotConn>,
    expired: AtomicBool,
}

/// Run `job` under the tier's reply deadline. `None` means the deadline passed.
///
/// - No deadline: inline.
/// - Called from a coroutine: inline too, registered with the slot's watchdog (the maintenance
///   thread). Past the deadline the watchdog shuts the connection's socket down, which makes
///   `may_postgres` fail every request waiting on it, so the caller wakes with an error that is
///   reported as `PoolReplyTimeout`. No helper coroutine, channel or timer per statement.
/// - Called from an OS thread: on a helper coroutine the thread waits on with a timeout
///   (`bounded`). This keeps the guarantee the starvation reproducer checks: a thread caller gets
///   its bounded error even when every `may` worker is blocked, because its wait does not need
///   one.
fn guarded<T: Send + 'static>(
    tier: &Arc<TierInner>,
    index: usize,
    conn: &Arc<SlotConn>,
    job: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let Some(limit) = tier.reply_timeout else {
        return Some(job());
    };
    if !may::coroutine::is_coroutine() {
        return bounded(Some(limit), job);
    }
    let watch = Arc::new(Watch {
        started_ms: mono_ms(),
        conn: Arc::clone(conn),
        expired: AtomicBool::new(false),
    });
    let key = tier.next_watch.fetch_add(1, Ordering::Relaxed);
    let slot = &tier.slots[index];
    slot.watches
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push((key, Arc::clone(&watch)));
    let out = job();
    {
        let mut w = slot
            .watches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(pos) = w.iter().position(|(k, _)| *k == key) {
            w.swap_remove(pos);
        }
    }
    if watch.expired.load(Ordering::Acquire) {
        None
    } else {
        Some(out)
    }
}

/// Watchdog pass over one slot: expire inline statements past `limit` and shut down the socket of
/// each connection they wait on.
fn expire_watches(slot: &Slot, limit: Duration) {
    let now = mono_ms();
    let limit_ms = u64::try_from(limit.as_millis()).unwrap_or(u64::MAX);
    let mut to_close: Vec<Arc<SlotConn>> = Vec::new();
    {
        let w = slot
            .watches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (_, watch) in w.iter() {
            if now.saturating_sub(watch.started_ms) >= limit_ms
                && !watch.expired.swap(true, Ordering::AcqRel)
                && !to_close.iter().any(|c| Arc::ptr_eq(c, &watch.conn))
            {
                to_close.push(Arc::clone(&watch.conn));
            }
        }
    }
    for conn in to_close {
        conn.broken.store(true, Ordering::Release);
        conn.force_close();
    }
}

/// Run `job` within `deadline` (on a coroutine the caller waits on), or inline without one.
/// `None` means the deadline elapsed; the job may still be running and finishes on its own.
fn bounded<T: Send + 'static>(
    deadline: Option<Duration>,
    job: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let Some(limit) = deadline else {
        return Some(job());
    };
    let (tx, rx) = may::sync::mpsc::channel();
    let job = move || {
        let _ = tx.send(job());
    };
    let need = job_stack_size();
    if may::config().get_stack_size() >= need {
        // The runtime's default stack is large enough: spawn from may's coroutine pool (no
        // stack allocation per statement).
        let _ = may::go!(job);
    } else {
        // A smaller default: a dedicated stack of the size jobs need (mmap per spawn; raise the
        // runtime's stack size to avoid it).
        let _ = may::go!(may::coroutine::Builder::new().stack_size(need), job);
    }
    rx.recv_timeout(limit).ok()
}

/// Stack, in machine words (may's unit), for the coroutines that run pool jobs (statement,
/// reconnect, TLS handshake).
///
/// Jobs used to run on dedicated OS threads; a coroutine with may's default stack (4096 words,
/// 32 KiB) can overflow in a TLS reconnect and die without unwinding. The default need is 256 KiB
/// (`LIFEGUARD_JOB_STACK_BYTES` overrides it). When the runtime's default stack
/// (`may::config().set_stack_size`, in words) is at least this large, jobs come from may's
/// coroutine pool; otherwise each job gets its own stack, which costs an `mmap` per statement.
pub(crate) fn job_stack_size() -> usize {
    static SIZE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *SIZE.get_or_init(|| {
        let bytes: usize = std::env::var("LIFEGUARD_JOB_STACK_BYTES")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(256 * 1024);
        (bytes / std::mem::size_of::<usize>()).max(0x1000)
    })
}

fn timed<T>(
    tier: &'static str,
    sql: &str,
    f: impl FnOnce() -> Result<T, may_postgres::Error>,
) -> Result<T, LifeError> {
    #[cfg(feature = "tracing")]
    let _span = tracing_helpers::execute_query_span(sql); // created, not entered: see tracing_helpers
    let _ = sql;
    let start = Instant::now();
    let out = f().map_err(LifeError::from);
    #[cfg(feature = "metrics")]
    {
        METRICS.record_query_duration(start.elapsed(), Some(tier));
        if out.is_err() {
            METRICS.record_query_error(Some(tier));
        }
    }
    let _ = (start, tier);
    out
}

fn exec_kind(
    tier: &'static str,
    h: &Client,
    conn: &SlotConn,
    sql: &str,
    refs: &[&dyn ToSql],
    kind: Kind,
) -> Result<Out, LifeError> {
    timed(tier, sql, || {
        let (out, lookup) = conn.stmts.with_statement(h, sql, |st| match kind {
            Kind::Execute => h.execute(st, refs).map(Out::Count),
            Kind::QueryOne => h.query_one(st, refs).map(Out::One),
            Kind::QueryAll => h.query(st, refs).map(Out::All),
        })?;
        #[cfg(feature = "metrics")]
        METRICS.record_statement_cache(lookup == Lookup::Hit, tier);
        let _ = lookup;
        Ok(out)
    })
}

/// A standalone statement on a shared slot; a connectivity error heals the slot and retries once.
fn run_shared_with_heal(
    tier: &Arc<TierInner>,
    index: usize,
    conn: Arc<SlotConn>,
    sql: &str,
    refs: &[&dyn ToSql],
    kind: Kind,
) -> Result<Out, LifeError> {
    let name = tier.tier;
    let h = conn.take_handle();
    match exec_kind(name, &h, &conn, sql, refs, kind) {
        // Closed by the reply-deadline watchdog: the caller is out of time, so no retry.
        Err(e) if conn.deadline_closed.load(Ordering::Acquire) => Err(e),
        Err(e) if life_error_is_connectivity_heal_candidate(&e) => {
            conn.broken.store(true, Ordering::Release);
            let fresh = match tier.slots[index].heal(&conn, name) {
                Ok(f) => f,
                Err(_) => return Err(e),
            };
            let h2 = fresh.take_handle();
            let r = exec_kind(name, &h2, &fresh, sql, refs, kind);
            if !r
                .as_ref()
                .is_err_and(life_error_is_connectivity_heal_candidate)
            {
                fresh.put_handle(h2);
            }
            r
        }
        r => {
            conn.put_handle(h);
            r
        }
    }
}

fn drain(stream: may_postgres::RowStream) -> Result<(), may_postgres::Error> {
    for r in stream {
        r?;
    }
    Ok(())
}

/// `BEGIN; rls_set_session(..); <stmt>; COMMIT` on one handle of a pinned connection.
///
/// Queries pipeline all four statements: they are written together and cost one round trip.
/// `may_postgres` does not report a row count through its streaming API, so for `Execute` the
/// `BEGIN` and context are pipelined, then the statement and `COMMIT` follow (three round trips;
/// before this pool, four). Every stream is drained in order and stops at its first error, so an
/// error is never attributed to a later statement. A failed statement leaves the transaction
/// aborted, and the pipelined `COMMIT` then rolls it back.
fn contextual_unit(
    h: &Client,
    conn: &SlotConn,
    args: &SessionArgs,
    sql: &str,
    refs: &[&dyn ToSql],
    kind: Kind,
) -> Result<Out, LifeError> {
    let tier = "primary";
    let (begin, _) = conn
        .stmts
        .get_or_prepare(h, "BEGIN")
        .map_err(LifeError::from)?;
    let (set, _) = conn
        .stmts
        .get_or_prepare(h, RLS_SET_SESSION_SQL)
        .map_err(rls_error)?;
    let (commit, _) = conn
        .stmts
        .get_or_prepare(h, "COMMIT")
        .map_err(LifeError::from)?;
    let arg_refs = args.refs();
    let r = (|| -> Result<Out, LifeError> {
        let (st, _) = conn.stmts.get_or_prepare(h, sql).map_err(LifeError::from)?;
        let s_begin = h.query_raw(&begin, &[]).map_err(LifeError::from)?;
        let s_set = h.query_raw(&set, &arg_refs).map_err(LifeError::from)?;
        match kind {
            Kind::Execute => {
                let rb = drain(s_begin);
                let rs = drain(s_set);
                rb.map_err(LifeError::from)?;
                if let Err(e) = rs {
                    let _ = h.batch_execute("ROLLBACK");
                    return Err(rls_error(e));
                }
                let n = timed(tier, sql, || h.execute(&st, refs));
                match n {
                    Ok(n) => {
                        h.execute(&commit, &[]).map_err(LifeError::from)?;
                        Ok(Out::Count(n))
                    }
                    Err(e) => {
                        let _ = h.batch_execute("ROLLBACK");
                        Err(e)
                    }
                }
            }
            Kind::QueryOne | Kind::QueryAll => {
                let s_stmt = h.query_raw(&st, refs).map_err(LifeError::from);
                let s_commit = h.query_raw(&commit, &[]).map_err(LifeError::from);
                let rb = drain(s_begin);
                let rs = drain(s_set);
                let rows: Result<Vec<Row>, LifeError> = match s_stmt {
                    Ok(s) => timed(tier, sql, || s.collect()),
                    Err(e) => Err(e),
                };
                let rc = match s_commit {
                    Ok(s) => drain(s).map_err(LifeError::from),
                    Err(e) => Err(e),
                };
                rb.map_err(LifeError::from)?;
                rs.map_err(rls_error)?;
                let rows = rows?;
                rc?;
                if kind == Kind::QueryAll {
                    Ok(Out::All(rows))
                } else {
                    let mut it = rows.into_iter();
                    match (it.next(), it.next()) {
                        (Some(row), None) => Ok(Out::One(row)),
                        (None, _) => Err(LifeError::QueryError(
                            "no rows returned (expected one row)".to_string(),
                        )),
                        (Some(_), Some(_)) => Err(LifeError::QueryError(
                            "query returned more than one row (expected one row)".to_string(),
                        )),
                    }
                }
            }
        }
    })();
    if let Err(e) = &r {
        if is_stale_message(e) {
            conn.stmts.clear();
        }
    }
    r
}

fn is_stale_message(e: &LifeError) -> bool {
    matches!(e, LifeError::PostgresError(pe) if crate::pool::stmt_cache::is_stale_error(pe))
}

fn rls_error(error: may_postgres::Error) -> LifeError {
    if error.code().is_some_and(|code| code.code() == "42883") {
        LifeError::Pool("rls_set_session SQL function not found on database".to_string())
    } else {
        LifeError::Pool(format!("rls_set_session context injection failed: {error}"))
    }
}

// ---------------------------------------------------------------------------------------------
// Session arguments (computed once per context)

/// `rls_set_session` arguments in owned form, serialised once when the context is attached.
pub(crate) struct SessionArgs {
    params: [OwnedParam; 8],
}

impl SessionArgs {
    fn new(ctx: &SessionContext) -> Result<Self, LifeError> {
        let roles = serde_json::to_value(&ctx.roles)
            .map_err(|e| LifeError::Other(format!("failed to serialize session roles: {e}")))?;
        let permissions = serde_json::to_value(&ctx.permissions).map_err(|e| {
            LifeError::Other(format!("failed to serialize session permissions: {e}"))
        })?;
        Ok(Self {
            params: [
                OwnedParam::String(TextParam::some(ctx.tenant_id.clone())),
                OwnedParam::Uuid(Some(ctx.subject_id)),
                OwnedParam::Uuid(Some(ctx.organization_id)),
                OwnedParam::String(TextParam::some(ctx.session_id.clone())),
                OwnedParam::Json(Some(roles)),
                OwnedParam::Json(Some(permissions)),
                OwnedParam::String(TextParam(ctx.user_type.clone())),
                OwnedParam::String(TextParam(ctx.org_type.clone())),
            ],
        })
    }

    fn refs(&self) -> Vec<&dyn ToSql> {
        self.params.iter().map(OwnedParam::as_sql_ref).collect()
    }
}

// ---------------------------------------------------------------------------------------------
// LifeguardPool

/// Pool of `PostgreSQL` connections with **primary** slots for writes and optional **replica**
/// slots for reads when lag allows.
///
/// # Which constructor?
///
/// | Entry point | Use when |
/// |-------------|----------|
/// | [`LifeguardPool::new`] | Tests and embedders that want **defaults** (30s acquire timeout, 8 in flight per slot, 500ms WAL poll). |
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
    /// Open pools with default [`LifeguardPoolSettings`] (30s acquire timeout, 8 statements in
    /// flight per slot).
    ///
    /// For file/env-driven timeouts and queue depth, use [`Self::new_with_settings`] or
    /// [`Self::from_database_config`].
    ///
    /// # Errors
    ///
    /// Returns [`LifeError::Pool`] for invalid size/URL combinations, or [`LifeError::Other`] when a
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
    ///
    /// # Errors
    ///
    /// Returns [`LifeError::Pool`] for invalid size/URL combinations, or [`LifeError::Other`] when a
    /// connection slot fails.
    pub fn new_with_settings(
        primary_url: &str,
        primary_pool_size: usize,
        replica_urls: Vec<String>,
        replica_pool_size: usize,
        settings: &LifeguardPoolSettings,
    ) -> Result<Self, LifeError> {
        let replica_urls_trimmed: Vec<String> = replica_urls
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

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

        let primary = WorkerPool::new(
            primary_pool_size,
            |_| primary_url.to_string(),
            "primary",
            settings,
        )?;

        let has_replica_tier = !replica_urls_trimmed.is_empty() && replica_pool_size > 0;
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
    /// per-slot in-flight depth) plus explicit replica list and replica tier width.
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

    /// Slots connected to the primary URL (writes and fallback reads).
    #[must_use]
    pub fn primary_pool_size(&self) -> usize {
        self.primary.pool_size
    }

    /// Slots connected to replica URLs; `0` if this pool is primary-only.
    #[must_use]
    pub fn replica_pool_size(&self) -> usize {
        self.replicas.as_ref().map_or(0, |r| r.pool_size)
    }

    /// Same as [`Self::primary_pool_size`] (historical name for “main” pool width).
    #[must_use]
    pub fn pool_size(&self) -> usize {
        self.primary_pool_size()
    }

    /// `true` when replica slots exist and the lag monitor reports the replica is behind.
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

    /// Pin one primary slot for a multi-statement unit of work (for example `BEGIN` → ORM work →
    /// `COMMIT`). While the returned [`ExclusivePrimaryLifeExecutor`] is alive, no other statement
    /// runs on that connection; other callers use the remaining slots.
    ///
    /// # Errors
    ///
    /// Returns [`LifeError::Pool`] when every slot is being replaced.
    pub fn exclusive_primary_write_executor(
        &self,
    ) -> Result<ExclusivePrimaryLifeExecutor<'_>, LifeError> {
        let pin = Pin::acquire(&self.primary.inner)?;
        Ok(ExclusivePrimaryLifeExecutor {
            pool: self,
            handle: Mutex::new(Some(pin.conn.take_handle())),
            pin,
        })
    }

    /// Run a multi-statement unit of work on one pinned primary connection with
    /// transaction-local RLS context.
    ///
    /// The pool starts a transaction, calls `public.rls_set_session(...)` (pipelined with the
    /// `BEGIN`, one round trip), and then passes its [`ExclusivePrimaryLifeExecutor`] to
    /// `operation`. All ORM and raw executor calls in the closure therefore use the same
    /// connection and the same RLS context. A successful closure is committed; an error is rolled
    /// back. Unwinding also triggers a best-effort rollback before the slot is released.
    ///
    /// Use [`PooledLifeExecutor::with_session_context`] for a single contextual statement. Use
    /// this method when several statements must share one transaction. Context-free callers
    /// continue to use the normal pool API and pay no RLS cost.
    ///
    /// # Errors
    ///
    /// Returns the first database or application error. Context injection is fail-closed: the
    /// operation is not called if the helper is unavailable or rejects the context.
    pub fn with_session_transaction<T>(
        &self,
        context: &SessionContext,
        operation: impl FnOnce(&ExclusivePrimaryLifeExecutor<'_>) -> Result<T, LifeError>,
    ) -> Result<T, LifeError> {
        let args = SessionArgs::new(context)?;
        let executor = self.exclusive_primary_write_executor()?;
        executor.begin_with_context(&args)?;

        let mut transaction = PoolTransactionGuard::new(&executor);
        match operation(transaction.executor()) {
            Ok(value) => {
                transaction.commit()?;
                Ok(value)
            }
            Err(operation_error) => {
                if let Err(rollback_error) = transaction.rollback() {
                    log::error!(
                        "lifeguard pool: contextual transaction rollback failed: {rollback_error}"
                    );
                }
                Err(operation_error)
            }
        }
    }
}

/// Pins one primary [`LifeguardPool`] slot: every [`LifeExecutor`] call uses the same underlying
/// connection until this value is dropped (PRD U-4).
///
/// Both reads and writes go through the **primary** tier (no replica routing), which matches
/// PostgreSQL transaction semantics for `BEGIN`/`COMMIT` on a single connection.
pub struct ExclusivePrimaryLifeExecutor<'a> {
    pool: &'a LifeguardPool,
    handle: Mutex<Option<Client>>,
    pin: Pin,
}

impl ExclusivePrimaryLifeExecutor<'_> {
    fn tier(&self) -> &Arc<TierInner> {
        &self.pool.primary.inner
    }

    /// Run `f` on the pinned handle within the reply deadline.
    fn call<T: Send + 'static>(
        &self,
        f: impl FnOnce(&Client, &SlotConn) -> Result<T, LifeError> + Send + 'static,
    ) -> Result<T, LifeError> {
        let started = Instant::now();
        let handle = self
            .handle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .ok_or_else(|| {
                LifeError::Pool(
                    "ExclusivePrimaryLifeExecutor: the pinned connection was lost after a reply timeout"
                        .to_string(),
                )
            })?;
        let conn = Arc::clone(&self.pin.conn);
        #[cfg(feature = "tracing")]
        let ctx = may_tracing::current();
        let watched = Arc::clone(&self.pin.conn);
        let out = guarded(&self.pin.tier, self.pin.index, &watched, move || {
            #[cfg(feature = "tracing")]
            let _ctx = may_tracing::set_current(ctx);
            let r = f(&handle, &conn);
            (r, handle)
        });
        match out {
            Some((r, h)) => {
                *self
                    .handle
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(h);
                r
            }
            None => {
                // The handle stays with the wedged statement; the unit cannot continue.
                Err(self.tier().on_reply_timeout(self.pin.index, started, true))
            }
        }
    }

    fn run_values(
        &self,
        kind: Kind,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<Out, LifeError> {
        self.run_owned(kind, query, values_to_owned(values)?)
    }

    fn run_owned(
        &self,
        kind: Kind,
        query: &str,
        params: Vec<OwnedParam>,
    ) -> Result<Out, LifeError> {
        let sql: Arc<str> = Arc::from(query);
        self.call(move |h, conn| {
            let refs: Vec<&dyn ToSql> = params.iter().map(OwnedParam::as_sql_ref).collect();
            exec_kind("primary", h, conn, &sql, &refs, kind)
        })
    }

    /// `BEGIN` and `rls_set_session(..)` pipelined (one round trip).
    fn begin_with_context(&self, args: &SessionArgs) -> Result<(), LifeError> {
        let params: Vec<OwnedParam> = args.params.to_vec();
        self.call(move |h, conn| {
            let (begin, _) = conn
                .stmts
                .get_or_prepare(h, "BEGIN")
                .map_err(LifeError::from)?;
            let (set, _) = conn
                .stmts
                .get_or_prepare(h, RLS_SET_SESSION_SQL)
                .map_err(rls_error)?;
            let refs: Vec<&dyn ToSql> = params.iter().map(OwnedParam::as_sql_ref).collect();
            let sb = h.query_raw(&begin, &[]).map_err(LifeError::from)?;
            let ss = h.query_raw(&set, &refs).map_err(LifeError::from);
            let rb = drain(sb);
            let rs = match ss {
                Ok(s) => drain(s).map_err(rls_error),
                Err(e) => Err(e),
            };
            rb.map_err(LifeError::from)?;
            if let Err(e) = rs {
                let _ = h.batch_execute("ROLLBACK");
                return Err(e);
            }
            Ok(())
        })
    }

    fn simple(&self, sql: &'static str) -> Result<(), LifeError> {
        self.call(move |h, _| h.batch_execute(sql).map_err(LifeError::from))
    }
}

struct PoolTransactionGuard<'executor, 'pool> {
    executor: &'executor ExclusivePrimaryLifeExecutor<'pool>,
    active: bool,
}

impl<'executor, 'pool> PoolTransactionGuard<'executor, 'pool> {
    fn new(executor: &'executor ExclusivePrimaryLifeExecutor<'pool>) -> Self {
        Self {
            executor,
            active: true,
        }
    }

    fn executor(&self) -> &ExclusivePrimaryLifeExecutor<'pool> {
        self.executor
    }

    fn commit(&mut self) -> Result<(), LifeError> {
        self.active = false;
        self.executor.simple("COMMIT")
    }

    fn rollback(&mut self) -> Result<(), LifeError> {
        self.active = false;
        self.executor.simple("ROLLBACK")
    }
}

impl Drop for PoolTransactionGuard<'_, '_> {
    fn drop(&mut self) {
        if self.active {
            if let Err(error) = self.executor.simple("ROLLBACK") {
                log::error!(
                    "lifeguard pool: contextual transaction cleanup rollback failed: {error}"
                );
            }
        }
    }
}

impl Drop for ExclusivePrimaryLifeExecutor<'_> {
    fn drop(&mut self) {
        if let Some(h) = self
            .handle
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            self.pin.conn.put_handle(h);
        }
    }
}

impl fmt::Debug for ExclusivePrimaryLifeExecutor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExclusivePrimaryLifeExecutor")
            .field("slot", &self.pin.index)
            .finish_non_exhaustive()
    }
}

impl LifeExecutor for ExclusivePrimaryLifeExecutor<'_> {
    fn execute(&self, query: &str, params: &[&dyn ToSql]) -> Result<u64, LifeError> {
        if params.is_empty() {
            return self.execute_values(query, &sea_query::Values(Vec::new()));
        }
        Err(LifeError::Pool(
            "ExclusivePrimaryLifeExecutor: use execute_values(query, &sea_query::Values) or ORM APIs; dynamic &dyn ToSql cannot cross the pool boundary".to_string(),
        ))
    }

    fn query_one(&self, query: &str, params: &[&dyn ToSql]) -> Result<Row, LifeError> {
        if params.is_empty() {
            return self.query_one_values(query, &sea_query::Values(Vec::new()));
        }
        Err(LifeError::Pool(
            "ExclusivePrimaryLifeExecutor: use query_one_values(query, &sea_query::Values) or ORM APIs; dynamic &dyn ToSql cannot cross the pool boundary".to_string(),
        ))
    }

    fn query_all(&self, query: &str, params: &[&dyn ToSql]) -> Result<Vec<Row>, LifeError> {
        if params.is_empty() {
            return self.query_all_values(query, &sea_query::Values(Vec::new()));
        }
        Err(LifeError::Pool(
            "ExclusivePrimaryLifeExecutor: use query_all_values(query, &sea_query::Values) or ORM APIs; dynamic &dyn ToSql cannot cross the pool boundary".to_string(),
        ))
    }

    fn execute_values(&self, query: &str, values: &sea_query::Values) -> Result<u64, LifeError> {
        match self.run_values(Kind::Execute, query, values)? {
            Out::Count(n) => Ok(n),
            _ => Err(internal_kind_error()),
        }
    }

    fn query_one_values(&self, query: &str, values: &sea_query::Values) -> Result<Row, LifeError> {
        match self.run_values(Kind::QueryOne, query, values)? {
            Out::One(r) => Ok(r),
            _ => Err(internal_kind_error()),
        }
    }

    fn query_all_values(
        &self,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<Vec<Row>, LifeError> {
        match self.run_values(Kind::QueryAll, query, values)? {
            Out::All(r) => Ok(r),
            _ => Err(internal_kind_error()),
        }
    }

    fn __lg_execute_owned(&self, query: &str, params: Vec<OwnedParam>) -> Result<u64, LifeError> {
        match self.run_owned(Kind::Execute, query, params)? {
            Out::Count(n) => Ok(n),
            _ => Err(internal_kind_error()),
        }
    }

    fn __lg_query_all_owned(
        &self,
        query: &str,
        params: Vec<OwnedParam>,
    ) -> Result<Vec<Row>, LifeError> {
        match self.run_owned(Kind::QueryAll, query, params)? {
            Out::All(r) => Ok(r),
            _ => Err(internal_kind_error()),
        }
    }
}

fn internal_kind_error() -> LifeError {
    LifeError::Pool("internal pool error: result kind mismatch".to_string())
}

fn values_to_owned(values: &sea_query::Values) -> Result<Vec<OwnedParam>, LifeError> {
    values.0.iter().map(OwnedParam::try_from).collect()
}

/// [`LifeExecutor`] that runs statements through a [`LifeguardPool`].
///
/// Use [`LifeExecutor::execute_values`], [`LifeExecutor::query_one_values`], and
/// [`LifeExecutor::query_all_values`] (or ORM methods built on them). Raw
/// `execute` / `query_*` with non-empty `&[&dyn ToSql]` are rejected: the statement may run on a
/// separate coroutine under the reply deadline, so parameters must be owned.
///
/// **Read routing:** by default, reads may go to a configured replica when WAL lag allows; see
/// [`ReadPreference`] and [`Self::with_read_preference`]. Writes always use the primary tier.
///
/// **RLS integration (Story 5):** optionally carries a [`SessionContext`]. Each contextual
/// statement pins a slot and runs `BEGIN`, `public.rls_set_session($1, ..., $8)`, the
/// application statement and `COMMIT` (pipelined). The transaction-local context cannot leak into
/// another statement. When `session_context` is `None` (the default), statements run in
/// autocommit on shared, pipelined slots.
#[derive(Clone)]
pub struct PooledLifeExecutor {
    pool: Arc<LifeguardPool>,
    read_preference: ReadPreference,
    session_context: Option<SessionContext>,
    session_args: Option<Arc<SessionArgs>>,
}

impl PooledLifeExecutor {
    #[must_use]
    pub fn new(pool: Arc<LifeguardPool>) -> Self {
        Self {
            pool,
            read_preference: ReadPreference::default(),
            session_context: None,
            session_args: None,
        }
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

    /// Attach a [`SessionContext`] for RLS session injection.
    ///
    /// When a context is attached, every statement executed through this executor runs in a
    /// short transaction that first calls `public.rls_set_session($1, ..., $8)`; it commits only
    /// after both succeed. The arguments are serialised once, here.
    ///
    /// Returns `self` for method chaining.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use lifeguard::{LifeguardPool, LifeExecutor, PooledLifeExecutor, SessionContext};
    /// use std::sync::Arc;
    ///
    /// # fn take_pool() -> Arc<LifeguardPool> { todo!() }
    /// let pool = take_pool();
    /// let executor = PooledLifeExecutor::new(pool).with_session_context(SessionContext {
    ///     tenant_id: "hauliage".to_string(),
    ///     subject_id: uuid::Uuid::new_v4(),
    ///     organization_id: uuid::Uuid::new_v4(),
    ///     session_id: "session-123".to_string(),
    ///     roles: vec!["admin".to_string()],
    ///     permissions: vec!["read".to_string()],
    ///     user_type: Some("admin".to_string()),
    ///     org_type: None,
    /// });
    /// ```
    #[must_use]
    pub fn with_session_context(mut self, ctx: SessionContext) -> Self {
        // Serialisation of two `Vec<String>` cannot fail; keep the context and fall back to
        // per-statement serialisation (and its error) if it ever does.
        self.session_args = SessionArgs::new(&ctx).ok().map(Arc::new);
        self.session_context = Some(ctx);
        self
    }

    fn session(&self) -> Result<Option<Arc<SessionArgs>>, LifeError> {
        match (&self.session_args, &self.session_context) {
            (Some(a), _) => Ok(Some(Arc::clone(a))),
            (None, Some(ctx)) => SessionArgs::new(ctx).map(|a| Some(Arc::new(a))),
            (None, None) => Ok(None),
        }
    }

    fn run(
        &self,
        tier: &WorkerPool,
        kind: Kind,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<Out, LifeError> {
        self.run_owned(tier, kind, query, values_to_owned(values)?)
    }

    fn run_owned(
        &self,
        tier: &WorkerPool,
        kind: Kind,
        query: &str,
        params: Vec<OwnedParam>,
    ) -> Result<Out, LifeError> {
        let session = self.session()?;
        // Contextual statements always run on the primary: the transaction and its RLS context
        // must see the primary's data, as before.
        let tier = if session.is_some() {
            &self.pool.primary
        } else {
            tier
        };
        tier.run(kind, Arc::from(query), params, session)
    }
}

impl LifeExecutor for PooledLifeExecutor {
    fn execute(&self, query: &str, params: &[&dyn ToSql]) -> Result<u64, LifeError> {
        if params.is_empty() {
            return self.execute_values(query, &sea_query::Values(Vec::new()));
        }
        Err(LifeError::Pool(
            "PooledLifeExecutor: use execute_values(query, &sea_query::Values) or ORM APIs; dynamic &dyn ToSql cannot cross the pool boundary".to_string(),
        ))
    }

    fn query_one(&self, query: &str, params: &[&dyn ToSql]) -> Result<Row, LifeError> {
        if params.is_empty() {
            return self.query_one_values(query, &sea_query::Values(Vec::new()));
        }
        Err(LifeError::Pool(
            "PooledLifeExecutor: use query_one_values(query, &sea_query::Values) or ORM APIs; dynamic &dyn ToSql cannot cross the pool boundary".to_string(),
        ))
    }

    fn query_all(&self, query: &str, params: &[&dyn ToSql]) -> Result<Vec<Row>, LifeError> {
        if params.is_empty() {
            return self.query_all_values(query, &sea_query::Values(Vec::new()));
        }
        Err(LifeError::Pool(
            "PooledLifeExecutor: use query_all_values(query, &sea_query::Values) or ORM APIs; dynamic &dyn ToSql cannot cross the pool boundary".to_string(),
        ))
    }

    fn execute_values(&self, query: &str, values: &sea_query::Values) -> Result<u64, LifeError> {
        match self.run(&self.pool.primary, Kind::Execute, query, values)? {
            Out::Count(n) => Ok(n),
            _ => Err(internal_kind_error()),
        }
    }

    fn query_one_values(&self, query: &str, values: &sea_query::Values) -> Result<Row, LifeError> {
        let tier = self.pool.read_pool_for(self.read_preference);
        match self.run(tier, Kind::QueryOne, query, values)? {
            Out::One(r) => Ok(r),
            _ => Err(internal_kind_error()),
        }
    }

    fn query_all_values(
        &self,
        query: &str,
        values: &sea_query::Values,
    ) -> Result<Vec<Row>, LifeError> {
        let tier = self.pool.read_pool_for(self.read_preference);
        match self.run(tier, Kind::QueryAll, query, values)? {
            Out::All(r) => Ok(r),
            _ => Err(internal_kind_error()),
        }
    }
    fn __lg_execute_owned(&self, query: &str, params: Vec<OwnedParam>) -> Result<u64, LifeError> {
        match self.run_owned(&self.pool.primary, Kind::Execute, query, params)? {
            Out::Count(n) => Ok(n),
            _ => Err(internal_kind_error()),
        }
    }

    /// Batch statements with `RETURNING` are writes: they always use the primary.
    fn __lg_query_all_owned(
        &self,
        query: &str,
        params: Vec<OwnedParam>,
    ) -> Result<Vec<Row>, LifeError> {
        match self.run_owned(&self.pool.primary, Kind::QueryAll, query, params)? {
            Out::All(r) => Ok(r),
            _ => Err(internal_kind_error()),
        }
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
        assert_eq!(
            a, b,
            "salt must not depend on time; limit must be stable per slot"
        );
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod session_args_tests {
    use super::*;

    fn context() -> SessionContext {
        SessionContext {
            tenant_id: "tenant-a".to_string(),
            subject_id: uuid::Uuid::new_v4(),
            organization_id: uuid::Uuid::new_v4(),
            session_id: "session-test".to_string(),
            roles: vec!["admin".to_string()],
            permissions: vec!["read".to_string(), "write".to_string()],
            user_type: Some("admin".to_string()),
            org_type: None,
        }
    }

    #[test]
    fn session_args_follow_rls_set_session_order() {
        let ctx = context();
        let a = SessionArgs::new(&ctx).unwrap();
        assert_eq!(a.refs().len(), 8);
        match &a.params[0] {
            OwnedParam::String(TextParam(Some(s))) => assert_eq!(s, "tenant-a"),
            other => panic!("tenant_id: {other:?}"),
        }
        match &a.params[1] {
            OwnedParam::Uuid(Some(u)) => assert_eq!(*u, ctx.subject_id),
            other => panic!("subject_id: {other:?}"),
        }
        match &a.params[5] {
            OwnedParam::Json(Some(j)) => assert_eq!(j, &serde_json::json!(["read", "write"])),
            other => panic!("permissions: {other:?}"),
        }
        match &a.params[7] {
            OwnedParam::String(TextParam(None)) => {}
            other => panic!("org_type must be typed NULL: {other:?}"),
        }
    }

    #[test]
    fn session_context_types_are_send_and_clone() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        fn assert_clone<T: Clone>() {}
        assert_send::<SessionContext>();
        assert_clone::<SessionContext>();
        assert_send::<PooledLifeExecutor>();
        assert_sync::<PooledLifeExecutor>();
        assert_clone::<PooledLifeExecutor>();
        assert_sync::<LifeguardPool>();
        assert_send::<LifeguardPool>();
    }

    #[test]
    fn executor_keeps_context_and_precomputes_args() {
        // Construct the executor's session fields without a live pool.
        let ctx = context();
        let args = SessionArgs::new(&ctx).map(Arc::new).ok();
        assert!(args.is_some());
    }
}
