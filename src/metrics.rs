use once_cell::sync::Lazy;
use opentelemetry::{
    global,
    metrics::{Counter, Histogram},
};
use opentelemetry_prometheus::PrometheusExporter;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

pub static METRICS: Lazy<LifeguardMetrics> = Lazy::new(LifeguardMetrics::init);

pub struct LifeguardMetrics {
    pub exporter: PrometheusExporter,
    pub queries_total: Counter<u64>,
    pub query_duration: Histogram<f64>,
    pub coroutine_wait_duration: Histogram<f64>,
    pub queue_depth: Arc<AtomicUsize>,
}

impl LifeguardMetrics {
    pub fn init() -> Self {
        let exporter = opentelemetry_prometheus::exporter().build().expect("failed to build prometheus exporter");
        let meter = global::meter("lifeguard");

        let queries_total = meter.u64_counter("lifeguard_queries_total")
            .with_description("Total queries executed").build();

        let query_duration = meter.f64_histogram("lifeguard_query_duration_seconds")
            .with_description("Duration of queries").build();

        let coroutine_wait_duration = meter.f64_histogram("lifeguard_coroutine_wait_seconds")
            .with_description("Time coroutines waited for query results").build();

        let queue_depth = Arc::new(AtomicUsize::new(0));
        let depth_clone = Arc::clone(&queue_depth);

        meter.u64_observable_gauge("lifeguard_pool_queue_depth")
            .with_description("Number of tasks waiting in the DB pool queue")
            .with_callback(move |observer| {
                observer.observe(depth_clone.load(Ordering::Relaxed) as u64, &[]);
            });

        Self {
            exporter,
            queries_total,
            query_duration,
            coroutine_wait_duration,
            queue_depth,
        }
    }

    pub fn record_query(&self, elapsed: std::time::Duration) {
        self.queries_total.add(1, &[]);
        self.query_duration.record(elapsed.as_secs_f64(), &[]);
    }

    pub fn observe_wait(&self, duration: std::time::Duration) {
        self.coroutine_wait_duration.record(duration.as_secs_f64(), &[]);
    }
}
