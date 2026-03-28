//! Global logging through a single [`may::sync::mpsc`] queue.
//!
//! Producers call [`enqueue`] (or the [`lifeguard_log!`] macro). One coroutine drains the
//! channel and writes lines to stderr so formatting stays sequential without locking on the
//! send path.
//!
//! ## `log` crate
//!
//! Use [`ChannelLogger`] and [`init_log_bridge`] to forward `log::info!` (etc.) through the
//! same channel.
//!
//! ## `tracing` crate
//!
//! With the **`tracing`** Cargo feature, use [`channel_layer`] on a [`tracing_subscriber::Registry`]
//! stack. Span metadata active at event time is captured into [`LogRecord::active_span`].
//!
//! ## OpenTelemetry and host apps (e.g. BRRTRouter)
//!
//! **This crate does not call [`opentelemetry::global::set_tracer_provider`].** The application
//! (or framework) must install **one** global `TracerProvider` and **one** `tracing` subscriber.
//!
//! Practical pattern:
//!
//! 1. Host constructs a single OTel `TracerProvider`, sets the global provider **once**.
//! 2. Host builds `Registry::default().with(...).with(...).try_init()` (or `set_default` in tests).
//! 3. Add [`OpenTelemetryLayer`](https://docs.rs/tracing-opentelemetry) from your tracer in that
//!    same `.with(...)` chain.
//! 4. Optionally add [`channel_layer()`] in the **same** chain so selected `tracing` events also
//!    flow through the may-channel drain (stderr by default).
//!
//! Lifeguard DB spans from [`crate::metrics::tracing_helpers`] use `tracing::span!` and therefore
//! join whatever subscriber the host installed (including OTel export), as long as the host does
//! not replace the subscriber after init.
//!
//! Full narrative and BRRTRouter file pointers: see repository doc
//! **`docs/OBSERVABILITY_APP_INTEGRATION.md`**.

mod log_bridge;

#[cfg(feature = "tracing")]
mod tracing_layer;

use may::sync::mpsc;
use std::sync::mpsc::SendError;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub use log_bridge::{init_log_bridge, ChannelLogger, CHANNEL_LOG_BRIDGE};

#[cfg(feature = "tracing")]
pub use tracing_layer::{channel_layer, ChannelLayer};

/// Severity for [`LogRecord`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<log::Level> for LogLevel {
    fn from(level: log::Level) -> Self {
        match level {
            log::Level::Error => LogLevel::Error,
            log::Level::Warn => LogLevel::Warn,
            log::Level::Info => LogLevel::Info,
            log::Level::Debug => LogLevel::Debug,
            log::Level::Trace => LogLevel::Trace,
        }
    }
}

#[cfg(feature = "tracing")]
impl From<tracing::Level> for LogLevel {
    fn from(level: tracing::Level) -> Self {
        match level {
            tracing::Level::ERROR => LogLevel::Error,
            tracing::Level::WARN => LogLevel::Warn,
            tracing::Level::INFO => LogLevel::Info,
            tracing::Level::DEBUG => LogLevel::Debug,
            tracing::Level::TRACE => LogLevel::Trace,
        }
    }
}

fn level_token(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "ERROR",
        LogLevel::Warn => "WARN",
        LogLevel::Info => "INFO",
        LogLevel::Debug => "DEBUG",
        LogLevel::Trace => "TRACE",
    }
}

/// One log line routed through the global channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogRecord {
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    pub timestamp_ms: u128,
    /// Span description captured at enqueue time (e.g. from `tracing`), not in the drainer.
    pub active_span: Option<String>,
}

impl LogRecord {
    /// Build a record with the current wall time (no active span).
    pub fn new(level: LogLevel, target: impl Into<String>, message: impl Into<String>) -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        Self {
            level,
            target: target.into(),
            message: message.into(),
            timestamp_ms,
            active_span: None,
        }
    }

    /// Attach span snapshot (from `tracing` at event time).
    #[must_use]
    pub fn with_active_span(mut self, span: impl Into<String>) -> Self {
        self.active_span = Some(span.into());
        self
    }

    /// Single-line text for sinks (stderr, tests, custom drains).
    pub fn format_line(&self) -> String {
        let mut line = format!(
            "[{}][{}] {} {}",
            self.timestamp_ms,
            level_token(self.level),
            self.target,
            self.message
        );
        if let Some(ref sp) = self.active_span {
            line.push_str(&format!(" [span={sp}]"));
        }
        line
    }
}

static GLOBAL_TX: OnceLock<mpsc::Sender<LogRecord>> = OnceLock::new();

fn start_global_drain() -> mpsc::Sender<LogRecord> {
    let (tx, rx) = mpsc::channel::<LogRecord>();
    // Keep the drain coroutine alive for the process lifetime (do not join on drop).
    let handle = may::go!(move || {
        for rec in &rx {
            eprintln!("{}", rec.format_line());
        }
    });
    #[allow(clippy::mem_forget)] // Drain coroutine must keep running after init returns.
    std::mem::forget(handle);
    tx
}

/// Sender used by all producers; clones share the same underlying queue.
pub fn global_log_sender() -> &'static mpsc::Sender<LogRecord> {
    GLOBAL_TX.get_or_init(start_global_drain)
}

/// Send one record to the global drain coroutine. Ignores send errors (e.g. drain stopped).
pub fn enqueue(record: LogRecord) {
    let _ = global_log_sender().send(record);
}

/// Same as [`enqueue`] but returns whether the send succeeded.
pub fn try_enqueue(record: LogRecord) -> Result<(), SendError<LogRecord>> {
    global_log_sender().send(record)
}

#[cfg(test)]
pub(crate) fn drain_into(
    rx: mpsc::Receiver<LogRecord>,
    sink: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
) {
    for rec in &rx {
        if let Ok(mut g) = sink.lock() {
            g.push(rec.format_line());
        }
    }
}

/// Enqueue a formatted log line on the global `may` channel.
#[macro_export]
macro_rules! lifeguard_log {
    ($lvl:ident, $($arg:tt)*) => {
        $crate::logging::enqueue($crate::logging::LogRecord::new(
            $crate::logging::LogLevel::$lvl,
            module_path!(),
            format!($($arg)*),
        ))
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[allow(clippy::unwrap_used)]
    #[test]
    fn format_line_includes_target_and_message() {
        let r = LogRecord::new(LogLevel::Warn, "my::target", "hello");
        let line = r.format_line();
        assert!(line.contains("WARN"));
        assert!(line.contains("my::target"));
        assert!(line.contains("hello"));
    }

    #[test]
    fn format_line_includes_active_span() {
        let r = LogRecord::new(LogLevel::Info, "t", "m").with_active_span("my::mod::root");
        assert!(r.format_line().contains("[span=my::mod::root]"));
    }

    #[test]
    fn may_channel_drains_in_coroutine() {
        let (tx, rx) = mpsc::channel::<LogRecord>();
        let out = Arc::new(Mutex::new(Vec::<String>::new()));
        let out_c = out.clone();
        let handle = may::go!(move || {
            drain_into(rx, out_c);
        });
        let _ = tx.send(LogRecord::new(LogLevel::Info, "t", "one"));
        let _ = tx.send(LogRecord::new(LogLevel::Info, "t", "two"));
        drop(tx);
        let _ = handle.join();
        let g = out.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(g.len(), 2);
        assert!(g[0].contains("one"));
        assert!(g[1].contains("two"));
    }
}
