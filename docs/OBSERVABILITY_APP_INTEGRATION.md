# Observability: Lifeguard with a host app (e.g. BRRTRouter)

This document describes how **OpenTelemetry tracing**, the **`tracing` subscriber stack**, and **Lifeguard’s may-channel logging** fit together. Follow these rules so you never register two global `TracerProvider`s and so Lifeguard spans share the same trace context as the router.

## Rules (short)

1. **One `TracerProvider` per process** — Build a single OpenTelemetry SDK provider (processors, sampler, resource, OTLP, etc.). Call `opentelemetry::global::set_tracer_provider` **once** at application startup (or use the 0.30+ equivalent your stack documents). Do not have libraries call this.
2. **One `tracing` subscriber** — Compose **`tracing_subscriber::Registry`** with **layers** (filter, fmt, `OpenTelemetryLayer`, Lifeguard’s `channel_layer`, etc.). Use **`.try_init()`** or **`tracing::subscriber::set_default`** (tests) once for that stack.
3. **Lifeguard does not own OTel globals** — The `lifeguard` crate emits `tracing::span!` when the **`tracing`** Cargo feature is enabled. It must **not** call `set_tracer_provider` or otherwise install a second provider.
4. **Lifeguard `channel_layer()` is optional** — It adds another **layer** on the **same** registry. It forwards `tracing` events into Lifeguard’s **may `mpsc` queue** (drained to stderr by default). It is **not** a second tracer or provider.

## Why this works

- OpenTelemetry’s global is effectively **one provider pipeline**. Multiple **`Tracer`** handles can be created **from that same provider** (different instrumentation scope names); that is not the same as two competing globals.
- **`OpenTelemetryLayer::new(tracer)`** maps `tracing` spans to OTel using that tracer’s provider.
- **`lifeguard::channel_layer()`** only **enqueues** structured lines for the may-channel logger; it does not create an OTel `TracerProvider`.

## BRRTRouter (reference host)

BRRTRouter owns process startup and observability wiring:

| Location | Role |
|----------|------|
| [`BRRTRouter/src/otel.rs`](../../BRRTRouter/src/otel.rs) | **`init_logging_with_config`** builds `tracing_subscriber::registry()` and layers (`EnvFilter`, sampling, redaction, `fmt`), then **`.try_init()`**. This is the **single place** to extend when you add OTLP (see below). |
| [`BRRTRouter/tests/tracing_util.rs`](../../BRRTRouter/tests/tracing_util.rs) | Tests use **`tracing::subscriber::set_default`** with `Registry::default().with(OpenTelemetryLayer::...)` so the global subscriber is not contested. Same **one provider / one subscriber** idea, scoped to the test. |

When you add production OTel export, do it **inside** the same initialization path as `init_logging_with_config` (or a small helper it calls): create the provider, set the global provider once, then add **`OpenTelemetryLayer::new(provider.tracer("…"))`** to the **same** `.with(...)` chain **before or after** other layers as required by `tracing-subscriber` (order affects filtering and formatting).

### Optional: Lifeguard may-channel path from BRRTRouter

If the service depends on **Lifeguard** with the **`tracing`** feature:

1. Add a dependency, e.g. `lifeguard = { path = "…", features = ["tracing"] }` (or crates.io with the same feature).
2. In **`otel.rs`**, when building the registry, add:

   ```rust
   .with(lifeguard::channel_layer())
   ```

   Place it where you want events duplicated into the may-channel drain (often after `EnvFilter` so filtered-out events are not enqueued). Be aware **fmt** and **channel_layer** both handle events — you may get console + may-channel lines; adjust layers if you want only one text sink.

3. Do **not** call any Lifeguard API to set a `TracerProvider`.

## Lifeguard API reference (tracing feature)

- **`lifeguard::channel_layer()`** / **`ChannelLayer`** — `tracing_subscriber::Layer` enqueueing to the global may log channel.
- **`lifeguard::metrics::tracing_helpers`** — Spans such as `lifeguard.execute_query`; they participate in whatever `OpenTelemetryLayer` (or other subscriber) the **host** installed.

## Tests

- Prefer **`tracing::subscriber::set_default`** in tests (as in BRRTRouter’s `TestTracing`) to avoid fighting a single global `try_init()`.
- Shut down the test `TracerProvider` in `Drop` if your SDK requires it (see `tracing_util.rs`).

## See also

- Rustdoc on **`lifeguard::logging`** (`src/logging/mod.rs`) for `log` bridge vs `tracing` layer.
- BRRTRouter crate docs: **`brrtrouter::otel`** module documentation.
