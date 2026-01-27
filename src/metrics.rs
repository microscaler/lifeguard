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
//! - `lifeguard_pool_size` (gauge): Current pool size
//! - `lifeguard_active_connections` (gauge): Active connections
//! - `lifeguard_connection_wait_time_seconds` (histogram): Time waiting for connection
//! - `lifeguard_query_duration_seconds` (histogram): Query execution time
//! - `lifeguard_query_errors_total` (counter): Query errors
//!
//! ## Tracing
//!
//! OpenTelemetry spans are created for:
//! - `lifeguard.acquire_connection`: Connection acquisition
//! - `lifeguard.execute_query`: Query execution
//! - `lifeguard.release_connection`: Connection release

#[cfg(feature = "metrics")]
// Note: once_cell::sync::Lazy is deprecated in favor of std::sync::LazyLock,
// but LazyLock requires Rust 1.80+. Using once_cell for compatibility.
#[allow(deprecated)]
use std::sync::LazyLock;
#[cfg(feature = "metrics")]
use opentelemetry::{
    global,
    metrics::{Counter, Gauge, Histogram},
};
#[cfg(feature = "metrics")]
use opentelemetry_prometheus::PrometheusExporter;

/// Lifeguard metrics collector
///
/// This struct holds all Prometheus metrics for Lifeguard. It's initialized
/// lazily on first access and can be accessed via the `METRICS` static.
#[cfg(feature = "metrics")]
pub struct LifeguardMetrics {
    /// Prometheus exporter for scraping metrics
    pub exporter: PrometheusExporter,
    /// Pool size gauge
    pub pool_size: Gauge<u64>,
    /// Active connections gauge
    pub active_connections: Gauge<u64>,
    /// Connection wait time histogram (seconds)
    pub connection_wait_time: Histogram<f64>,
    /// Query duration histogram (seconds)
    pub query_duration: Histogram<f64>,
    /// Query errors counter
    pub query_errors: Counter<u64>,
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
        // Note: Using expect() here is intentional - metrics initialization failure
        // is a critical system error that should fail fast at startup.
        // The application should handle this at a higher level.
        #[allow(clippy::expect_used)] // Critical system error - fail fast at startup
        let exporter = opentelemetry_prometheus::exporter()
            .build()
            .expect("failed to build prometheus exporter");
        let meter = global::meter("lifeguard");

        let pool_size = meter
            .u64_gauge("lifeguard_pool_size")
            .with_description("Current pool size")
            .build();

        let active_connections = meter
            .u64_gauge("lifeguard_active_connections")
            .with_description("Active connections")
            .build();

        let connection_wait_time = meter
            .f64_histogram("lifeguard_connection_wait_time_seconds")
            .with_description("Time waiting for connection")
            .build();

        let query_duration = meter
            .f64_histogram("lifeguard_query_duration_seconds")
            .with_description("Query execution time")
            .build();

        let query_errors = meter
            .u64_counter("lifeguard_query_errors_total")
            .with_description("Total query errors")
            .build();

        Self {
            exporter,
            pool_size,
            active_connections,
            connection_wait_time,
            query_duration,
            query_errors,
        }
    }

    /// Record query execution duration
    pub fn record_query_duration(&self, duration: std::time::Duration) {
        self.query_duration.record(duration.as_secs_f64(), &[]);
    }

    /// Record query error
    pub fn record_query_error(&self) {
        self.query_errors.add(1, &[]);
    }

    /// Record connection wait time
    pub fn record_connection_wait(&self, duration: std::time::Duration) {
        self.connection_wait_time.record(duration.as_secs_f64(), &[]);
    }

    /// Update pool size
    pub fn set_pool_size(&self, size: u64) {
        self.pool_size.record(size, &[]);
    }

    /// Update active connections count
    pub fn set_active_connections(&self, count: u64) {
        self.active_connections.record(count, &[]);
    }
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

    pub fn record_query_duration(&self, _duration: std::time::Duration) {}
    pub fn record_query_error(&self) {}
    pub fn record_connection_wait(&self, _duration: std::time::Duration) {}
    pub fn set_pool_size(&self, _size: u64) {}
    pub fn set_active_connections(&self, _count: u64) {}
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization() {
        let metrics = LifeguardMetrics::init();
        // Just verify it doesn't panic
        metrics.record_query_duration(std::time::Duration::from_millis(100));
        metrics.record_query_error();
        metrics.record_connection_wait(std::time::Duration::from_millis(50));
        metrics.set_pool_size(10);
        metrics.set_active_connections(5);
    }

    #[test]
    #[cfg(feature = "tracing")]
    fn test_tracing_spans() {
        let _span1 = tracing_helpers::acquire_connection_span();
        let _span2 = tracing_helpers::execute_query_span("SELECT 1");
        let _span3 = tracing_helpers::release_connection_span();
        // Just verify they don't panic
    }
}
