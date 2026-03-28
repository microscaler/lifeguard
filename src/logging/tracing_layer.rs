//! [`tracing_subscriber::Layer`] that enqueues [`super::LogRecord`] on the may channel.

use super::{enqueue, LogLevel, LogRecord};
use std::fmt;
use std::fmt::Write as _;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;

/// Layer that forwards `tracing` events through [`super::enqueue`].
#[derive(Debug, Default, Clone, Copy)]
pub struct ChannelLayer;

/// Build a [`ChannelLayer`] for use with `Registry::default().with(channel_layer())`.
pub fn channel_layer() -> ChannelLayer {
    ChannelLayer
}

#[derive(Default)]
struct EventFieldVisitor {
    message: Option<String>,
    kv: Vec<(String, String)>,
}

impl EventFieldVisitor {
    fn into_message(self) -> String {
        if let Some(m) = self.message {
            return m;
        }
        let mut out = String::new();
        for (k, v) in self.kv {
            if !out.is_empty() {
                out.push(' ');
            }
            let _ = write!(&mut out, "{k}={v}");
        }
        if out.is_empty() {
            return String::new();
        }
        out
    }

    fn record_pair(&mut self, field: &Field, display: String) {
        if field.name() == "message" {
            self.message = Some(display);
        } else {
            self.kv.push((field.name().to_string(), display));
        }
    }
}

impl Visit for EventFieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_pair(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_pair(field, format!("{value}"));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_pair(field, format!("{value}"));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_pair(field, format!("{value}"));
    }

    fn record_i128(&mut self, field: &Field, value: i128) {
        self.record_pair(field, format!("{value}"));
    }

    fn record_u128(&mut self, field: &Field, value: u128) {
        self.record_pair(field, format!("{value}"));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_pair(field, format!("{value}"));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.record_pair(field, format!("{value:?}"));
    }
}

fn snapshot_active_span<S>(ctx: &Context<'_, S>) -> Option<String>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    ctx.lookup_current().map(|s| {
        let m = s.metadata();
        format!("{}::{}", m.target(), m.name())
    })
}

impl<S> Layer<S> for ChannelLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut visitor = EventFieldVisitor::default();
        event.record(&mut visitor);
        let message = visitor.into_message();
        let meta = event.metadata();
        let target = meta.target().to_string();
        let level = LogLevel::from(*meta.level());
        let active_span = snapshot_active_span(&ctx);
        let mut rec = LogRecord::new(level, target, message);
        if let Some(sp) = active_span {
            rec = rec.with_active_span(sp);
        }
        enqueue(rec);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn tracing_event_through_layer() {
        tracing::subscriber::with_default(
            tracing_subscriber::registry().with(channel_layer()),
            || {
                tracing::info!(target: "lifeguard_tracing_layer_test", "hello_layer");
            },
        );
    }
}
