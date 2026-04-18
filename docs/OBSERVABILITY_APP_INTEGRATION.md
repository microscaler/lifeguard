# Observability: Lifeguard with a host app (e.g. BRRTRouter)

This document describes how **OpenTelemetry tracing**, the **`tracing` subscriber stack**, and **Lifeguard’s may-channel logging** fit together on the **microscaler** platform. Follow these rules so you never register two global `TracerProvider`s and so Lifeguard spans share the same trace context as the HTTP router.

**Master cross-repo plan:** [`microscaler-observability/docs/PRD.md`](../../microscaler-observability/docs/PRD.md).

## Rules (short)

1. **One `TracerProvider` per process** — Owned by **[`microscaler-observability`](https://github.com/microscaler/microscaler-observability)**. The host (typically **BRRTRouter**) calls **`brrtrouter::otel::init_logging_with_config`** once at startup; when **`OTEL_EXPORTER_OTLP_ENDPOINT`** is set, that path delegates to **`microscaler_observability::init`**, which installs the OTLP tracer, logger bridge, and W3C propagator. Do not have libraries call **`opentelemetry::global::set_tracer_provider`**.
2. **One `tracing` subscriber** — Installed by **`microscaler_observability`** on the OTLP path, or by BRRTRouter’s stdout-only path when OTLP is disabled. Use **`.try_init()`** once, or **`tracing::subscriber::set_default`** in tests.
3. **Lifeguard does not own OTel globals** — The `lifeguard` crate emits **`tracing::span!`** when the **`tracing`** Cargo feature is enabled. It must **not** call **`set_tracer_provider`**, **`set_logger_provider`**, **`set_meter_provider`**, or **`set_text_map_propagator`**. With the **`metrics`** feature, **`LifeguardMetrics::init`** keeps a **local** **`SdkMeterProvider`** for Prometheus scrape text only (no global meter provider).
4. **Lifeguard `channel_layer()` is optional** — It adds another **layer** on the **same** registry. It forwards `tracing` events into Lifeguard’s **may `mpsc` queue** (drained to stderr by default). It is **not** a second tracer or provider.

## Why this works

- OpenTelemetry’s global is effectively **one provider pipeline**. Multiple **`Tracer`** handles can be created **from that same provider** (different instrumentation scope names); that is not the same as two competing globals.
- **`OpenTelemetryLayer::new(tracer)`** maps `tracing` spans to OTel using that tracer’s provider.
- **`lifeguard::channel_layer()`** only **enqueues** structured lines for the may-channel logger; it does not create an OTel `TracerProvider`.

## BRRTRouter and `microscaler-observability` (reference host)

| Location | Role |
|----------|------|
| [`microscaler-observability`](../../microscaler-observability/) | **Owns** OTLP tracer + logger pipeline, propagator, and (with OTLP) the **`tracing`** subscriber stack. See **`init`**, **`ObservabilityConfig::from_env`**, **`ShutdownGuard`**. |
| [`BRRTRouter/src/otel.rs`](../../BRRTRouter/src/otel.rs) | **`init_logging_with_config`** — stdout-only path composes **`EnvFilter`**, sampling, redaction, **`fmt`**. OTLP path calls **`microscaler_observability::init`** and stores the shutdown guard; **`shutdown`** flushes telemetry. |
| [`BRRTRouter/src/server/http_server.rs`](../../BRRTRouter/src/server/http_server.rs) | **`ServerHandle::run_until_shutdown`** — Unix **SIGTERM** / **SIGINT**, then stop server, then **`otel::shutdown`**. |
| [`BRRTRouter/tests/tracing_util.rs`](../../BRRTRouter/tests/tracing_util.rs) | Tests use **`tracing::subscriber::set_default`** so the global subscriber is not contested. |

When you add **optional** Lifeguard may-channel duplication, add **`lifeguard::channel_layer()`** to the **same** subscriber chain the host uses — today that composition lives inside **`microscaler-observability`** / BRRTRouter’s init paths; coordinate with a single ordered **`.with(...)`** chain.

### Optional: Lifeguard may-channel path from BRRTRouter

If the service depends on **Lifeguard** with the **`tracing`** feature:

1. Add **`lifeguard = { path = "…", features = ["tracing"] }`** (or crates.io with the same feature).
2. Add **`lifeguard::channel_layer()`** to the registry **once**, in the same place other layers are composed (after **`EnvFilter`** if you want filtered events only).
3. Do **not** call any Lifeguard API to set a **`TracerProvider`**.

## Lifeguard API reference (tracing feature)

- **`lifeguard::channel_layer()`** / **`ChannelLayer`** — `tracing_subscriber::Layer` enqueueing to the global may log channel.
- **`lifeguard::metrics::tracing_helpers`** — Spans such as `lifeguard.execute_query`; they participate in whatever **`OpenTelemetryLayer`** (or other subscriber) the **host** installed.

## Tests

- Prefer **`tracing::subscriber::set_default`** in tests (as in BRRTRouter’s `TestTracing`) to avoid fighting a single global **`try_init()`**.
- Shut down the test **`TracerProvider`** in **`Drop`** if your SDK requires it (see **`tracing_util.rs`**).

## See also

- Rustdoc on **`lifeguard::logging`** (`src/logging/mod.rs`) for `log` bridge vs `tracing` layer.
- **[`microscaler-observability/docs/CLUSTER_OBSERVABILITY.md`](../../microscaler-observability/docs/CLUSTER_OBSERVABILITY.md)** — cluster OTLP endpoints and env vars.
- **Hauliage:** [`hauliage/docs/observability-migration.md`](../../hauliage/docs/observability-migration.md) — consumer-side **`main.rs`** and rollout notes.
