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
    global,
    metrics::{Counter, Gauge, Histogram},
    KeyValue,
};
#[cfg(feature = "metrics")]
use opentelemetry_sdk::metrics::SdkMeterProvider;
#[cfg(feature = "metrics")]
// Note: once_cell::sync::Lazy is deprecated in favor of std::sync::LazyLock,
// but LazyLock requires Rust 1.80+. Using once_cell for compatibility.
#[allow(deprecated)]
use std::sync::{Arc, LazyLock, Once};
#[cfg(feature = "metrics")]
static INIT_GLOBAL_METER_PROVIDER: Once = Once::new();

/// Lifeguard metrics collector
///
/// This struct holds all Prometheus metrics for Lifeguard. It's initialized
/// lazily on first access and can be accessed via the `METRICS` static.
#[cfg(feature = "metrics")]
pub struct LifeguardMetrics {
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
        // Wire the Prometheus exporter as the **global** OTEL meter provider so instruments
        // created via `global::meter` are not no-ops. Keep `registry` alive for text scrape.
        let registry = Arc::new(prometheus::Registry::new());
        let reg_for_provider = (*registry).clone();
        INIT_GLOBAL_METER_PROVIDER.call_once(|| {
            #[allow(clippy::expect_used)] // Critical system error - fail fast at startup
            let exporter = opentelemetry_prometheus::exporter()
                .with_registry(reg_for_provider)
                .build()
                .expect("failed to build prometheus exporter");
            let provider = SdkMeterProvider::builder().with_reader(exporter).build();
            global::set_meter_provider(provider);
        });
        let meter = global::meter("lifeguard");

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

        let pool_slot_heal_total = meter
            .u64_counter("lifeguard_pool_slot_heal_total")
            .with_description("Slot heal reconnects after connectivity errors")
            .build();

        let pool_connection_rotated_total = meter
            .u64_counter("lifeguard_pool_connection_rotated_total")
            .with_description("Connections rotated due to max_connection_lifetime policy")
            .build();

        Self {
            registry,
            pool_size,
            pool_workers,
            active_connections,
            connection_wait_time,
            query_duration,
            query_errors,
            wal_monitor_replica_routing_disabled,
            pool_acquire_timeout_total,
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
    pub fn record_pool_slot_heal(&self, _tier: &str) {}
    pub fn record_pool_connection_rotated(&self, _tier: &str) {}
}

#[cfg(not(feature = "metrics"))]
pub static METRICS: LifeguardMetrics = LifeguardMetrics;

/// Tracing helpers for database operations
#[cfg(feature = "tracing")]
pub mod tracing_helpers {
    use tracing::Span;

    /// Create a span for connection acquisition
    pub fn acquire_connection_span() -> Span {
        tracing::span!(tracing::Level::INFO, "lifeguard.acquire_connection")
    }

    /// Create a span for query execution
    pub fn execute_query_span(query: &str) -> Span {
        tracing::span!(
            tracing::Level::INFO,
            "lifeguard.execute_query",
            query = %query
        )
    }

    /// Create a span for connection release
    pub fn release_connection_span() -> Span {
        tracing::span!(tracing::Level::INFO, "lifeguard.release_connection")
    }

    /// Create a span for beginning a transaction
    pub fn begin_transaction_span() -> Span {
        tracing::span!(tracing::Level::INFO, "lifeguard.begin_transaction")
    }

    /// Create a span for committing a transaction
    pub fn commit_transaction_span() -> Span {
        tracing::span!(tracing::Level::INFO, "lifeguard.commit_transaction")
    }

    /// Create a span for rolling back a transaction
    pub fn rollback_transaction_span() -> Span {
        tracing::span!(tracing::Level::INFO, "lifeguard.rollback_transaction")
    }

    /// Create a span for connection health check
    pub fn health_check_span() -> Span {
        tracing::span!(tracing::Level::INFO, "lifeguard.health_check")
    }

    /// Slot replaced after connectivity-class error (PRD R5.2 / R8.2).
    pub fn pool_slot_heal_span() -> Span {
        tracing::span!(tracing::Level::INFO, "lifeguard.pool_slot_heal")
    }
}

/// No-op tracing helpers when tracing feature is disabled
#[cfg(not(feature = "tracing"))]
pub mod tracing_helpers {
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
        let metrics: &LifeguardMetrics = &*METRICS;
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
