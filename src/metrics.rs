//! Metrics Module - Epic 01 Story 05
//!
//! Provides Prometheus metrics and OpenTelemetry tracing for Lifeguard.
//!
//! This module exposes observability features that can be enabled via feature flags:
//! - `metrics`: Enables Prometheus metrics collection
//! - `tracing`: Enables OpenTelemetry distributed tracing
//!
//! ## Metrics
//!
//! The following Prometheus metrics are exposed:
//! - `lifeguard_pool_size` (gauge): Total pool slots (primary + replica)
//! - `lifeguard_pool_workers` (gauge, `pool_tier`): Slots per tier (`primary` | `replica`)
//! - `lifeguard_active_connections` (gauge): Active connections (total)
//! - `lifeguard_connection_wait_time_seconds` (histogram, optional `pool_tier`): Pool queue dwell (enqueue → worker start); direct `connect` handshake
//! - `lifeguard_query_duration_seconds` (histogram, optional `pool_tier`): Query execution time
//! - `lifeguard_query_errors_total` (counter, optional `pool_tier`): Query errors
//! - `lifeguard_wal_monitor_replica_routing_disabled` (gauge): 1 if WAL lag monitor gave up on replica connect
//! - `lifeguard_pool_acquire_timeout_total` (counter, `pool_tier`): `PoolAcquireTimeout` dispatches
//! - `lifeguard_pool_reply_timeout_total` (counter, `pool_tier`): `PoolReplyTimeout` dispatches (enqueued job, no worker reply in budget)
//! - `lifeguard_pool_slot_heal_total` (counter, `pool_tier`): Connectivity-class slot heal reconnects
//! - `lifeguard_pool_connection_rotated_total` (counter, `pool_tier`): `max_connection_lifetime` rotations
//!
//! ## Tracing
//!
//! OpenTelemetry spans are created for:
//! - `lifeguard.acquire_connection`: Connection acquisition
//! - `lifeguard.execute_query`: Query execution
//! - `lifeguard.release_connection`: Connection release

#[cfg(feature = "metrics")]
use opentelemetry::{
    metrics::{Counter, Gauge, Histogram, MeterProvider},
    KeyValue,
};
#[cfg(feature = "metrics")]
use opentelemetry_sdk::metrics::SdkMeterProvider;
#[cfg(feature = "metrics")]
// Note: once_cell::sync::Lazy is deprecated in favor of std::sync::LazyLock,
// but LazyLock requires Rust 1.80+. Using once_cell for compatibility.
#[allow(deprecated)]
use std::sync::{Arc, LazyLock};

/// Lifeguard metrics collector
///
/// This struct holds all Prometheus metrics for Lifeguard. It's initialized
/// lazily on first access and can be accessed via the `METRICS` static.
#[cfg(feature = "metrics")]
pub struct LifeguardMetrics {
    /// Keeps the Prometheus reader + OTEL meter instruments alive (not installed as a global).
    #[allow(dead_code)]
    meter_provider: SdkMeterProvider,
    /// Registry that holds the OTEL→Prometheus collector (keep alive for `gather()` / HTTP scrape).
    pub registry: Arc<prometheus::Registry>,
    /// Total pool slots (primary + replica); unlabeled for backward-compatible dashboards.
    pub pool_size: Gauge<u64>,
    /// Worker slots per `pool_tier` label (`primary` \| `replica`).
    pub pool_workers: Gauge<u64>,
    /// Active connections gauge
    pub active_connections: Gauge<u64>,
    /// Connection wait time histogram (seconds)
    pub connection_wait_time: Histogram<f64>,
    /// Query duration histogram (seconds)
    pub query_duration: Histogram<f64>,
    /// Query errors counter
    pub query_errors: Counter<u64>,
    /// 1 when [`crate::pool::wal::WalLagMonitor`] gave up connecting (replica reads use primary only)
    pub wal_monitor_replica_routing_disabled: Gauge<u64>,
    pub pool_acquire_timeout_total: Counter<u64>,
    pub pool_reply_timeout_total: Counter<u64>,
    pub pool_slot_heal_total: Counter<u64>,
    pub pool_connection_rotated_total: Counter<u64>,
}

#[cfg(feature = "metrics")]
impl LifeguardMetrics {
    /// Initialize metrics collector
    ///
    /// Creates all `Prometheus` metrics and sets up the exporter.
    ///
    /// # Panics
    ///
    /// This function will panic if the Prometheus exporter fails to initialize.
    /// This should only happen if there's a configuration error or system resource issue.
    ///
    /// Note: This uses `expect()` because metrics initialization failure at startup
    /// is a critical system error that should be caught during development/testing.
    /// In production, this should be handled by the application's startup error handling.
    #[must_use]
    pub fn init() -> Self {
        // Prometheus exporter + local `SdkMeterProvider` only — do **not** call
        // `global::set_meter_provider` (owned by `microscaler-observability`).
        let registry = Arc::new(prometheus::Registry::new());
        let reg_for_provider = (*registry).clone();
        #[allow(clippy::expect_used)] // Critical system error - fail fast at startup
        let exporter = opentelemetry_prometheus::exporter()
            .with_registry(reg_for_provider)
            .build()
            .expect("failed to build prometheus exporter");
        let meter_provider = SdkMeterProvider::builder().with_reader(exporter).build();
        let meter = meter_provider.meter("lifeguard");

        let pool_size = meter
            .u64_gauge("lifeguard_pool_size")
            .with_description("Total pool worker slots (primary + replica)")
            .build();

        let pool_workers = meter
            .u64_gauge("lifeguard_pool_workers")
            .with_description("Worker slots per pool tier (pool_tier label)")
            .build();

        let active_connections = meter
            .u64_gauge("lifeguard_active_connections")
            .with_description("Active connections")
            .build();

        let connection_wait_time = meter
            .f64_histogram("lifeguard_connection_wait_time_seconds")
            .with_description(
                "Pool: time from successful job enqueue to worker start (queue dwell); direct connect: handshake wait",
            )
            .build();

        let query_duration = meter
            .f64_histogram("lifeguard_query_duration_seconds")
            .with_description("Query execution time")
            .build();

        let query_errors = meter
            .u64_counter("lifeguard_query_errors_total")
            .with_description("Total query errors")
            .build();

        let wal_monitor_replica_routing_disabled = meter
            .u64_gauge("lifeguard_wal_monitor_replica_routing_disabled")
            .with_description("1 if WAL lag monitor gave up connecting to replica")
            .build();

        let pool_acquire_timeout_total = meter
            .u64_counter("lifeguard_pool_acquire_timeout_total")
            .with_description("Pool acquire timeouts waiting for a worker slot")
            .build();

        let pool_reply_timeout_total = meter
            .u64_counter("lifeguard_pool_reply_timeout_total")
            .with_description("Dispatched jobs whose worker reply missed the reply deadline (wedged worker or overlong statement)")
            .build();

        let pool_slot_heal_total = meter
            .u64_counter("lifeguard_pool_slot_heal_total")
            .with_description("Slot heal reconnects after connectivity errors")
            .build();

        let pool_connection_rotated_total = meter
            .u64_counter("lifeguard_pool_connection_rotated_total")
            .with_description("Connections rotated due to max_connection_lifetime policy")
            .build();

        Self {
            meter_provider,
            registry,
            pool_size,
            pool_workers,
            active_connections,
            connection_wait_time,
            query_duration,
            query_errors,
            wal_monitor_replica_routing_disabled,
            pool_acquire_timeout_total,
            pool_reply_timeout_total,
            pool_slot_heal_total,
            pool_connection_rotated_total,
        }
    }

    fn tier_kv(tier: &str) -> [KeyValue; 1] {
        [KeyValue::new("pool_tier", tier.to_string())]
    }

    /// Record query execution duration. Use `pool_tier` for pooled queries (`primary` / `replica`).
    pub fn record_query_duration(&self, duration: std::time::Duration, pool_tier: Option<&str>) {
        match pool_tier {
            Some(t) => self
                .query_duration
                .record(duration.as_secs_f64(), &Self::tier_kv(t)),
            None => self.query_duration.record(duration.as_secs_f64(), &[]),
        }
    }

    /// Record query error. Use `pool_tier` for pooled paths.
    pub fn record_query_error(&self, pool_tier: Option<&str>) {
        match pool_tier {
            Some(t) => self.query_errors.add(1, &Self::tier_kv(t)),
            None => self.query_errors.add(1, &[]),
        }
    }

    /// Record time waiting for a pool slot or direct connection setup.
    pub fn record_connection_wait(&self, duration: std::time::Duration, pool_tier: Option<&str>) {
        match pool_tier {
            Some(t) => self
                .connection_wait_time
                .record(duration.as_secs_f64(), &Self::tier_kv(t)),
            None => self
                .connection_wait_time
                .record(duration.as_secs_f64(), &[]),
        }
    }

    /// Update total pool size (sum of tiers).
    pub fn set_pool_size(&self, size: u64) {
        self.pool_size.record(size, &[]);
    }

    /// Per-tier worker counts (low-cardinality: `primary` and `replica` only).
    pub fn set_pool_workers_by_tier(&self, primary_slots: u64, replica_slots: u64) {
        self.pool_workers
            .record(primary_slots, &Self::tier_kv("primary"));
        self.pool_workers
            .record(replica_slots, &Self::tier_kv("replica"));
    }

    /// Update active connections count
    pub fn set_active_connections(&self, count: u64) {
        self.active_connections.record(count, &[]);
    }

    pub fn set_wal_monitor_replica_routing_disabled(&self, v: u64) {
        self.wal_monitor_replica_routing_disabled.record(v, &[]);
    }

    pub fn record_pool_acquire_timeout(&self, tier: &str) {
        self.pool_acquire_timeout_total.add(1, &Self::tier_kv(tier));
    }

    pub fn record_pool_reply_timeout(&self, tier: &str) {
        self.pool_reply_timeout_total.add(1, &Self::tier_kv(tier));
    }

    pub fn record_pool_slot_heal(&self, tier: &str) {
        self.pool_slot_heal_total.add(1, &Self::tier_kv(tier));
    }

    pub fn record_pool_connection_rotated(&self, tier: &str) {
        self.pool_connection_rotated_total
            .add(1, &Self::tier_kv(tier));
    }
}

/// OpenMetrics text for Lifeguard `lifeguard_*` series (for appending to BRRTRouter `/metrics`).
#[cfg(feature = "metrics")]
pub fn prometheus_scrape_text() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = METRICS.registry.gather();
    let mut buf = Vec::new();
    if let Err(e) = encoder.encode(&metric_families, &mut buf) {
        return format!("# lifeguard metrics encode error: {e}\n");
    }
    String::from_utf8_lossy(&buf).into_owned()
}

#[cfg(feature = "metrics")]
#[allow(clippy::declare_interior_mutable_const)]
pub static METRICS: LazyLock<LifeguardMetrics> = LazyLock::new(LifeguardMetrics::init);

/// No-op metrics implementation when metrics feature is disabled
#[cfg(not(feature = "metrics"))]
pub struct LifeguardMetrics;

#[cfg(not(feature = "metrics"))]
impl LifeguardMetrics {
    pub fn init() -> Self {
        Self
    }

    pub fn record_query_duration(&self, _duration: std::time::Duration, _pool_tier: Option<&str>) {}
    pub fn record_query_error(&self, _pool_tier: Option<&str>) {}
    pub fn record_connection_wait(&self, _duration: std::time::Duration, _pool_tier: Option<&str>) {
    }
    pub fn set_pool_size(&self, _size: u64) {}
    pub fn set_pool_workers_by_tier(&self, _primary_slots: u64, _replica_slots: u64) {}
    pub fn set_active_connections(&self, _count: u64) {}
    pub fn set_wal_monitor_replica_routing_disabled(&self, _v: u64) {}
    pub fn record_pool_acquire_timeout(&self, _tier: &str) {}
    pub fn record_pool_reply_timeout(&self, _tier: &str) {}
    pub fn record_pool_slot_heal(&self, _tier: &str) {}
    pub fn record_pool_connection_rotated(&self, _tier: &str) {}
}

#[cfg(not(feature = "metrics"))]
pub static METRICS: LifeguardMetrics = LifeguardMetrics;

/// Tracing helpers for database operations.
///
/// The spans are never *entered* by lifeguard: it runs on `may` coroutines,
/// which can resume on another OS thread after a yield, and
/// tracing-subscriber keeps the "current span" per thread. A span entered on
/// one thread and exited on another leaves a closed span on the first
/// thread's stack; the next span created there is cloned from it and
/// tracing-subscriber panics ("tried to clone a span that already closed").
/// Creating and dropping the span keeps the OTEL export - start, end,
/// fields - and never touches the per-thread stack. This is
/// [may_tracing ADR-0001](https://github.com/microscaler/may_tracing/blob/main/docs/ADR/ADR-0001-no-entered-guard-across-a-yield.md).
///
/// Whether a span is *linked to the caller's current span* (nested under the
/// request span in a trace) is a runtime switch, [`set_span_nesting`] /
/// `LIFEGUARD_SPAN_NESTING`, **on by default** since the switch reads the
/// **coroutine's** context (`may_tracing::current()`) and never the thread's
/// span stack: it is safe on `may` whatever the host does. A host that never
/// sets a context (a plain-thread or Tokio service without `may_tracing`)
/// simply gets root spans. `LIFEGUARD_SPAN_NESTING=0|false|off` or
/// `set_span_nesting(false)` forces root spans for every `lifeguard.*` span.
///
/// Pool worker threads (`lifeguard-pool-<tier>-<slot>`) are OS threads, not
/// coroutines; nothing sets a context there, so worker-side spans
/// (`lifeguard.pool_slot_heal`, keepalive, rotation) are pool-scoped roots.
#[cfg(feature = "tracing")]
pub mod tracing_helpers {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::OnceLock;
    use tracing::Span;

    static NESTING: AtomicBool = AtomicBool::new(true);
    static ENV_READ: OnceLock<()> = OnceLock::new();

    /// Link the `lifeguard.*` spans to the coroutine's current span
    /// (`may_tracing::current()`; `true`, the default) or create them as root
    /// spans (`false`). `LIFEGUARD_SPAN_NESTING=0|false|off|no` in the
    /// environment turns it off at first use unless this was called first.
    pub fn set_span_nesting(nested: bool) {
        let _ = ENV_READ.set(());
        NESTING.store(nested, Ordering::Relaxed);
    }

    /// Current setting (after applying the environment variable once).
    pub fn span_nesting() -> bool {
        ENV_READ.get_or_init(|| {
            if let Ok(v) = std::env::var("LIFEGUARD_SPAN_NESTING") {
                let v = v.trim().to_ascii_lowercase();
                NESTING.store(
                    !matches!(v.as_str(), "0" | "false" | "off" | "no"),
                    Ordering::Relaxed,
                );
            }
        });
        NESTING.load(Ordering::Relaxed)
    }

    /// A `lifeguard.*` span: a child of the coroutine's current span when
    /// nesting is on (`may_tracing::child_span!`, which yields a root span when
    /// there is no context), an explicit root otherwise. Never entered. Fields
    /// are forwarded as token trees so `%x` / `?x` work.
    macro_rules! lifeguard_span {
        ($name:literal $(, $($fields:tt)*)?) => {
            if span_nesting() {
                may_tracing::child_span!(tracing::Level::INFO, $name $(, $($fields)*)?)
            } else {
                tracing::span!(parent: None, tracing::Level::INFO, $name $(, $($fields)*)?)
            }
        };
    }

    /// Create a span for connection acquisition
    pub fn acquire_connection_span() -> Span {
        lifeguard_span!("lifeguard.acquire_connection")
    }

    /// Create a span for query execution
    pub fn execute_query_span(query: &str) -> Span {
        lifeguard_span!("lifeguard.execute_query", query = %query)
    }

    /// Create a span for connection release
    pub fn release_connection_span() -> Span {
        lifeguard_span!("lifeguard.release_connection")
    }

    /// Create a span for beginning a transaction
    pub fn begin_transaction_span() -> Span {
        lifeguard_span!("lifeguard.begin_transaction")
    }

    /// Create a span for committing a transaction
    pub fn commit_transaction_span() -> Span {
        lifeguard_span!("lifeguard.commit_transaction")
    }

    /// Create a span for rolling back a transaction
    pub fn rollback_transaction_span() -> Span {
        lifeguard_span!("lifeguard.rollback_transaction")
    }

    /// Create a span for connection health check
    pub fn health_check_span() -> Span {
        lifeguard_span!("lifeguard.health_check")
    }

    /// Slot replaced after connectivity-class error (PRD R5.2 / R8.2).
    pub fn pool_slot_heal_span() -> Span {
        lifeguard_span!("lifeguard.pool_slot_heal")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};
        use tracing::span::{Attributes, Id};
        use tracing_subscriber::layer::{Context, Layer};
        use tracing_subscriber::prelude::*;
        use tracing_subscriber::registry::LookupSpan;

        /// Records explicit parents only (never the thread's contextual span).
        #[derive(Clone, Default)]
        struct Parents(Arc<Mutex<HashMap<u64, Option<u64>>>>);
        impl<S: tracing::Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Parents {
            fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, _: Context<'_, S>) {
                if let Ok(mut m) = self.0.lock() {
                    m.insert(id.into_u64(), attrs.parent().map(Id::into_u64));
                }
            }
        }
        fn parent_of(p: &Parents, s: &Span) -> Option<u64> {
            let id = s.id().map(|i| i.into_u64())?;
            p.0.lock().ok()?.get(&id).copied().flatten()
        }

        #[test]
        fn nesting_is_settable() {
            set_span_nesting(false);
            assert!(!span_nesting());
            set_span_nesting(true);
            assert!(span_nesting());
        }

        #[test]
        fn query_span_nests_under_coroutine_context() {
            let parents = Parents::default();
            let _s = tracing::subscriber::set_default(
                tracing_subscriber::registry().with(parents.clone()),
            );
            set_span_nesting(true);
            let req = tracing::info_span!(parent: None, "http_request");
            let req_id = req.id().map(|i| i.into_u64());
            // under a context: children of it
            let (q, a, h) = may_tracing::with_span(req.clone(), || {
                (
                    execute_query_span("select 1"),
                    acquire_connection_span(),
                    pool_slot_heal_span(),
                )
            });
            assert_eq!(parent_of(&parents, &q), req_id);
            assert_eq!(parent_of(&parents, &a), req_id);
            assert_eq!(parent_of(&parents, &h), req_id);
            // no context: roots
            let q = execute_query_span("select 2");
            assert_eq!(parent_of(&parents, &q), None);
            // nesting off: roots even under a context
            set_span_nesting(false);
            let q = may_tracing::with_span(req, || execute_query_span("select 3"));
            assert_eq!(parent_of(&parents, &q), None);
            set_span_nesting(true);
        }
    }
}

/// No-op tracing helpers when tracing feature is disabled
#[cfg(not(feature = "tracing"))]
pub mod tracing_helpers {
    pub fn set_span_nesting(_nested: bool) {}
    pub fn span_nesting() -> bool {
        false
    }
    pub fn acquire_connection_span() {}
    pub fn execute_query_span(_query: &str) {}
    pub fn release_connection_span() {}
    pub fn begin_transaction_span() {}
    pub fn commit_transaction_span() {}
    pub fn rollback_transaction_span() {}
    pub fn health_check_span() {}
    pub fn pool_slot_heal_span() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        // Use the singleton so global meter provider is initialized once (matches production).
        let metrics: &LifeguardMetrics = &METRICS;
        // Just verify it doesn't panic
        metrics.record_query_duration(std::time::Duration::from_millis(100), None);
        metrics.record_query_error(None);
        metrics.record_connection_wait(std::time::Duration::from_millis(50), None);
        metrics.set_pool_size(10);
        metrics.set_pool_workers_by_tier(4, 2);
        metrics.set_active_connections(5);
        metrics.set_wal_monitor_replica_routing_disabled(0);
        metrics.record_pool_acquire_timeout("primary");
        metrics.record_pool_slot_heal("replica");
        metrics.record_pool_connection_rotated("primary");
    }

    #[test]
    #[cfg(feature = "tracing")]
    fn test_tracing_spans() {
        let _span1 = tracing_helpers::acquire_connection_span();
        let _span2 = tracing_helpers::execute_query_span("SELECT 1");
        let _span3 = tracing_helpers::release_connection_span();
        let _span4 = tracing_helpers::pool_slot_heal_span();
        // Just verify they don't panic
    }
}
