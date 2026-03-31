//! Global logging through a single [`may::sync::mpsc`] queue.
//!
//! Producers call [`enqueue`] (or the [`lifeguard_log!`](macro@crate::logging::lifeguard_log) macro). One coroutine drains the
//! channel and writes lines to stderr so formatting stays sequential without locking on the
//! send path. [`flush_log_channel`] and [`ChannelLogger`]'s
//! [`log::Log::flush`] block until prior enqueued records have been written (see reentrancy note
//! there).
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

/// Item on the global may `mpsc` logging queue.
pub enum LogMsg {
    /// Normal log line.
    Record(LogRecord),
    /// Synchronous barrier: the drain sends `()` on `done` after all prior messages are handled.
    Flush(std::sync::mpsc::Sender<()>),
}

impl std::fmt::Debug for LogMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Record(r) => f.debug_tuple("Record").field(r).finish(),
            Self::Flush(_) => f.write_str("Flush(..)"),
        }
    }
}

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

/// Replace CR/LF so one logical record stays one physical line (mitigates log forging).
fn sanitize_single_line_field(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
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
    ///
    /// [`Self::message`] and [`Self::active_span`] are escaped so embedded `\r`/`\n` cannot split
    /// output into extra physical lines (log-forging resistance).
    pub fn format_line(&self) -> String {
        let message = sanitize_single_line_field(&self.message);
        let mut line = format!(
            "[{}][{}] {} {}",
            self.timestamp_ms,
            level_token(self.level),
            self.target,
            message
        );
        if let Some(ref sp) = self.active_span {
            let sp = sanitize_single_line_field(sp);
            line.push_str(&format!(" [span={sp}]"));
        }
        line
    }
}

static GLOBAL_TX: OnceLock<mpsc::Sender<LogMsg>> = OnceLock::new();

fn start_global_drain() -> mpsc::Sender<LogMsg> {
    let (tx, rx) = mpsc::channel::<LogMsg>();
    // Keep the drain coroutine alive for the process lifetime (do not join on drop).
    let handle = may::go!(move || {
        for msg in &rx {
            match msg {
                LogMsg::Record(rec) => {
                    eprintln!("{}", rec.format_line());
                }
                LogMsg::Flush(done_tx) => {
                    let _ = done_tx.send(());
                }
            }
        }
    });
    #[allow(clippy::mem_forget)] // Drain coroutine must keep running after init returns.
    std::mem::forget(handle);
    tx
}

/// Sender used by all producers; clones share the same underlying queue.
pub fn global_log_sender() -> &'static mpsc::Sender<LogMsg> {
    GLOBAL_TX.get_or_init(start_global_drain)
}

/// Send one record to the global drain coroutine. Ignores send errors (e.g. drain stopped).
pub fn enqueue(record: LogRecord) {
    let _ = global_log_sender().send(LogMsg::Record(record));
}

/// Block until every record already queued before this call has been processed by the drain.
///
/// If the drain is stopped or the flush message cannot be sent, returns promptly without
/// blocking indefinitely.
///
/// **Reentrancy:** Do not call this from code that runs on the drain path (for example inside a
/// custom writer invoked while the drain prints a line), or the process may deadlock.
pub fn flush_log_channel() {
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    if global_log_sender().send(LogMsg::Flush(done_tx)).is_err() {
        return;
    }
    let _ = done_rx.recv();
}

/// Same as [`enqueue`] but returns whether the send succeeded.
pub fn try_enqueue(record: LogRecord) -> Result<(), SendError<LogRecord>> {
    global_log_sender()
        .send(LogMsg::Record(record))
        .map_err(|SendError(msg)| match msg {
            LogMsg::Record(r) => SendError(r),
            LogMsg::Flush(done_tx) => {
                drop(done_tx);
                SendError(LogRecord::new(
                    LogLevel::Error,
                    "lifeguard::logging",
                    "internal logging channel state error",
                ))
            }
        })
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
    fn format_line_escapes_newlines_in_message() {
        let r = LogRecord::new(
            LogLevel::Warn,
            "my::target",
            "a\n[999][INFO] fake next line",
        );
        let line = r.format_line();
        assert!(
            !line.contains('\n') && !line.contains('\r'),
            "must be one physical line: {line:?}"
        );
        assert!(line.contains("\\n"), "line: {line:?}");
        assert!(line.contains("fake next line"));
    }

    #[test]
    fn format_line_escapes_cr_lf_in_message_and_span() {
        let r = LogRecord::new(LogLevel::Info, "t", "x\ry").with_active_span("s\np");
        let line = r.format_line();
        assert!(
            !line.contains('\n') && !line.contains('\r'),
            "line: {line:?}"
        );
        assert!(
            line.contains("\\r") && line.contains("\\n"),
            "line: {line:?}"
        );
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

    /// Mirrors production drain handling of [`LogMsg`] so `flush` barriers are tested without
    /// relying on the process-global [`OnceLock`](std::sync::OnceLock) sender.
    #[test]
    fn log_msg_flush_waits_until_prior_records_processed() {
        let (tx, rx) = mpsc::channel::<LogMsg>();
        let out = Arc::new(Mutex::new(Vec::<String>::new()));
        let out_c = out.clone();
        let handle = may::go!(move || {
            for msg in &rx {
                match msg {
                    LogMsg::Record(rec) => {
                        if let Ok(mut g) = out_c.lock() {
                            g.push(rec.format_line());
                        }
                    }
                    LogMsg::Flush(done_tx) => {
                        let _ = done_tx.send(());
                    }
                }
            }
        });

        assert!(tx
            .send(LogMsg::Record(LogRecord::new(LogLevel::Info, "t", "one")))
            .is_ok());
        assert!(tx
            .send(LogMsg::Record(LogRecord::new(LogLevel::Info, "t", "two")))
            .is_ok());

        let (done_tx, done_rx) = std::sync::mpsc::channel();
        assert!(tx.send(LogMsg::Flush(done_tx)).is_ok());
        #[allow(clippy::unwrap_used)]
        {
            done_rx.recv().unwrap();
        }

        let g = out.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(g.len(), 2, "flush must run after both records: {g:?}");

        drop(tx);
        let _ = handle.join();
    }
}
