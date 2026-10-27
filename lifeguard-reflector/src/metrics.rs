//! `lifereflector_*` Prometheus metrics and the `/metrics` + `/healthz` endpoint.
//!
//! Handles are created once and registered in a private registry; the hot path only touches
//! atomics. Label cardinality is bounded by the number of tables that publish notifications.

use prometheus::{
    Encoder, Histogram, HistogramOpts, IntCounter, IntCounterVec, IntGauge, Opts, Registry,
    TextEncoder,
};
use std::io::{Read, Write};
use std::sync::OnceLock;

/// All reflector metrics.
pub struct Metrics {
    registry: Registry,
    /// Notifications received, by table.
    pub notifications: IntCounterVec,
    /// Payloads that did not parse.
    pub invalid_payloads: IntCounter,
    /// Events folded into an earlier event for the same key within a batch.
    pub coalesced: IntCounter,
    /// Keys sent to the cache for invalidation.
    pub keys_invalidated: IntCounter,
    /// Invalidated keys that were actually cached.
    pub keys_present: IntCounter,
    /// Batches flushed, by result (`ok`, `error`).
    pub flushes: IntCounterVec,
    /// Keys per flushed batch.
    pub batch_size: Histogram,
    /// Flush round-trip time.
    pub flush_seconds: Histogram,
    /// Trigger time to invalidation done.
    pub lag_seconds: Histogram,
    /// Reconnects, by target (`postgres`, `redis`).
    pub reconnects: IntCounterVec,
    /// Catch-up runs, by mode (`flush`, `ranged`, `full`).
    pub catch_ups: IntCounterVec,
    /// Keys removed by catch-up.
    pub catch_up_keys: IntCounter,
    /// Pending batches abandoned for a catch-up because the cache stayed down.
    pub overflows: IntCounter,
    /// 1 while this instance holds the advisory lock.
    pub leader: IntGauge,
    /// Keys waiting to be flushed.
    pub pending: IntGauge,
    /// Last processed trigger timestamp (ms since epoch).
    pub watermark_ms: IntGauge,
}

fn counter_vec(r: &Registry, name: &str, help: &str, labels: &[&str]) -> IntCounterVec {
    let c = IntCounterVec::new(Opts::new(name, help), labels).expect("valid metric");
    r.register(Box::new(c.clone())).expect("unique metric");
    c
}

fn counter(r: &Registry, name: &str, help: &str) -> IntCounter {
    let c = IntCounter::new(name, help).expect("valid metric");
    r.register(Box::new(c.clone())).expect("unique metric");
    c
}

fn gauge(r: &Registry, name: &str, help: &str) -> IntGauge {
    let g = IntGauge::new(name, help).expect("valid metric");
    r.register(Box::new(g.clone())).expect("unique metric");
    g
}

fn histogram(r: &Registry, name: &str, help: &str, buckets: Vec<f64>) -> Histogram {
    let h = Histogram::with_opts(HistogramOpts::new(name, help).buckets(buckets))
        .expect("valid metric");
    r.register(Box::new(h.clone())).expect("unique metric");
    h
}

const LATENCY_BUCKETS: [f64; 14] = [
    0.000_25, 0.000_5, 0.001, 0.002, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 5.0, 30.0,
];

impl Metrics {
    fn new() -> Self {
        let r = Registry::new();
        Self {
            notifications: counter_vec(
                &r,
                "lifereflector_notifications_total",
                "Change notifications received",
                &["table"],
            ),
            invalid_payloads: counter(
                &r,
                "lifereflector_invalid_payloads_total",
                "Notifications whose payload did not parse",
            ),
            coalesced: counter(
                &r,
                "lifereflector_coalesced_total",
                "Notifications folded into an earlier one for the same key in a batch",
            ),
            keys_invalidated: counter(
                &r,
                "lifereflector_keys_invalidated_total",
                "Keys sent to the cache for invalidation",
            ),
            keys_present: counter(
                &r,
                "lifereflector_keys_present_total",
                "Invalidated keys that were cached at the time",
            ),
            flushes: counter_vec(
                &r,
                "lifereflector_flushes_total",
                "Invalidation batches flushed",
                &["result"],
            ),
            batch_size: histogram(
                &r,
                "lifereflector_batch_keys",
                "Keys per invalidation batch",
                vec![
                    1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0, 512.0, 1024.0, 4096.0,
                ],
            ),
            flush_seconds: histogram(
                &r,
                "lifereflector_flush_seconds",
                "Invalidation batch round trip",
                LATENCY_BUCKETS.to_vec(),
            ),
            lag_seconds: histogram(
                &r,
                "lifereflector_lag_seconds",
                "Trigger time to invalidation complete (oldest event in the batch)",
                LATENCY_BUCKETS.to_vec(),
            ),
            reconnects: counter_vec(
                &r,
                "lifereflector_reconnects_total",
                "Connections re-established",
                &["target"],
            ),
            catch_ups: counter_vec(
                &r,
                "lifereflector_catch_ups_total",
                "Catch-up runs after a (re)connect or overflow",
                &["mode"],
            ),
            catch_up_keys: counter(
                &r,
                "lifereflector_catch_up_keys_total",
                "Keys removed by catch-up",
            ),
            overflows: counter(
                &r,
                "lifereflector_overflows_total",
                "Pending invalidations abandoned for a catch-up while the cache was down",
            ),
            leader: gauge(
                &r,
                "lifereflector_leader",
                "1 while this instance holds the advisory lock",
            ),
            pending: gauge(
                &r,
                "lifereflector_pending_keys",
                "Keys waiting to be flushed",
            ),
            watermark_ms: gauge(
                &r,
                "lifereflector_watermark_ms",
                "Trigger timestamp of the newest processed change (ms since epoch)",
            ),
            registry: r,
        }
    }

    /// Prometheus text exposition of every reflector metric.
    #[must_use]
    pub fn scrape_text(&self) -> String {
        let mut buf = Vec::new();
        let _ = TextEncoder::new().encode(&self.registry.gather(), &mut buf);
        String::from_utf8(buf).unwrap_or_default()
    }
}

/// Process-wide metrics.
pub fn metrics() -> &'static Metrics {
    static M: OnceLock<Metrics> = OnceLock::new();
    M.get_or_init(Metrics::new)
}

/// Serve `/metrics` and `/healthz` on `addr` from a coroutine.
///
/// # Errors
///
/// The address cannot be bound.
pub fn serve(addr: &str) -> std::io::Result<()> {
    let listener = may::net::TcpListener::bind(addr)?;
    may::go!(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            may::go!(move || {
                let mut buf = [0u8; 1024];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let (status, body) = if req.starts_with("GET /metrics") {
                    ("200 OK", metrics().scrape_text())
                } else if req.starts_with("GET /healthz") {
                    ("200 OK", "ok\n".to_owned())
                } else {
                    ("404 Not Found", String::new())
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 {status}\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    Ok(())
}
