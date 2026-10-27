//! Latency samples and the report rows the suite prints.

use serde::Serialize;
use std::time::Duration;

/// One measured scenario variant.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// Scenario group, e.g. `pk_select`.
    pub scenario: String,
    /// Implementation under test, e.g. `orm_pool`, `raw_may_prepared`, `raw_tokio`.
    pub variant: String,
    /// Concurrency (callers in flight) for this row.
    pub concurrency: usize,
    pub ops: usize,
    pub mean_us: f64,
    pub p50_us: f64,
    pub p95_us: f64,
    pub p99_us: f64,
    pub max_us: f64,
    /// Completed operations per second across all callers.
    pub ops_per_s: f64,
    /// Free-form note (rows per op, payload size, ...).
    pub note: String,
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p / 100.0).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Summarise latency samples (microseconds) measured over `wall`.
pub fn summarize(
    scenario: &str,
    variant: &str,
    concurrency: usize,
    mut samples_us: Vec<f64>,
    wall: Duration,
    note: impl Into<String>,
) -> Row {
    samples_us.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = samples_us.len();
    let mean = if n == 0 {
        0.0
    } else {
        samples_us.iter().sum::<f64>() / n as f64
    };
    let secs = wall.as_secs_f64();
    Row {
        scenario: scenario.to_string(),
        variant: variant.to_string(),
        concurrency,
        ops: n,
        mean_us: mean,
        p50_us: pct(&samples_us, 50.0),
        p95_us: pct(&samples_us, 95.0),
        p99_us: pct(&samples_us, 99.0),
        max_us: samples_us.last().copied().unwrap_or(0.0),
        ops_per_s: if secs > 0.0 { n as f64 / secs } else { 0.0 },
        note: note.into(),
    }
}

/// Markdown table of `rows`.
pub fn markdown(rows: &[Row]) -> String {
    let mut out = String::from(
        "| scenario | variant | conc | ops | p50 µs | p95 µs | p99 µs | mean µs | ops/s | note |\n|---|---|---:|---:|---:|---:|---:|---:|---:|---|\n",
    );
    for r in rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {:.1} | {:.1} | {:.1} | {:.1} | {:.0} | {} |\n",
            r.scenario,
            r.variant,
            r.concurrency,
            r.ops,
            r.p50_us,
            r.p95_us,
            r.p99_us,
            r.mean_us,
            r.ops_per_s,
            r.note
        ));
    }
    out
}
