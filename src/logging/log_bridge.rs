//! Bridge [`log`] records into the may-channel [`super::enqueue`] path.

use super::{enqueue, flush_log_channel, LogLevel, LogRecord};

/// Forwards `log::info!` and friends through [`super::global_log_sender`].
///
/// Install with [`init_log_bridge`] or [`log::set_logger`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ChannelLogger;

/// Global instance suitable for [`log::set_logger`].
pub static CHANNEL_LOG_BRIDGE: ChannelLogger = ChannelLogger;

impl log::Log for ChannelLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let level = LogLevel::from(record.metadata().level());
        let target = record.metadata().target().to_string();
        let message = format!("{}", record.args());
        enqueue(LogRecord::new(level, target, message));
    }

    /// Waits until records already enqueued before this call have been written by the global drain.
    ///
    /// Do not call from the drain path (e.g. inside output hooks run while the drainer prints), or
    /// a deadlock may occur.
    fn flush(&self) {
        flush_log_channel();
    }
}

/// Set [`CHANNEL_LOG_BRIDGE`] as the process logger and raise the max level to `Trace`.
///
/// Call once at startup. If another logger is already set, returns [`log::SetLoggerError`].
pub fn init_log_bridge() -> Result<(), log::SetLoggerError> {
    log::set_logger(&CHANNEL_LOG_BRIDGE)?;
    log::set_max_level(log::LevelFilter::Trace);
    Ok(())
}
