# Observability

Lifeguard is built for **OpenTelemetry-compatible** observability: optional **`tracing`** integration (spans and events that follow the ecosystem’s tracing model) and optional **Prometheus** metrics (`metrics` feature). You install **one** global `TracerProvider` and **one** `tracing` subscriber in the host process; Lifeguard does not take over globals—see [Host-owned OpenTelemetry](#host-owned-opentelemetry) below.

**Backends:** the same instrumentation works with **OpenTelemetry-native** stacks (OTLP → Grafana, Jaeger, Tempo, etc.) and with **Datadog** via the [OpenTelemetry Protocol](https://opentelemetry.io/docs/specs/otlp/)—for example Datadog Agent OTLP intake, or `otel-collector` forwarding to Datadog. Use your org’s standard collector/agent; Lifeguard emits **tracing** and **metrics** shapes that fit those pipelines.

**Deeper reference:** feature flags, Kind/Tilt dashboard refresh, and **metric tables** with labels — [docs/OBSERVABILITY.md](./docs/OBSERVABILITY.md). **App wiring** (Registry + `channel_layer`, no duplicate globals) — [docs/OBSERVABILITY_APP_INTEGRATION.md](./docs/OBSERVABILITY_APP_INTEGRATION.md).

---

## Prometheus metrics

When the `metrics` feature is enabled, typical series include (non-exhaustive; see [docs/OBSERVABILITY.md](./docs/OBSERVABILITY.md) for the full table):

- `lifeguard_pool_size` — Current pool size
- `lifeguard_active_connections` — Active connections
- `lifeguard_connection_wait_time` — Time waiting for connection
- `lifeguard_query_duration_seconds` — Query execution time
- `lifeguard_query_errors_total` — Query errors
- `lifeguard_cache_hits_total` — Cache hits
- `lifeguard_cache_misses_total` — Cache misses
- `lifeguard_replica_lag_bytes` — Replica lag (bytes)
- `lifeguard_replica_lag_seconds` — Replica lag (seconds)
- `lifeguard_replicas_healthy` — Number of healthy replicas

Pool-scoped series use a low-cardinality **`pool_tier`** label (`primary` / `replica`) where applicable.

## OpenTelemetry tracing

When the `tracing` feature is enabled:

- Distributed tracing for database operations
- Spans for: connection acquisition, query execution, cache operations
- Integration with existing OpenTelemetry infrastructure (via your process’s `tracing` + OTLP exporter)

### Host-owned OpenTelemetry

Lifeguard does **not** set a global OpenTelemetry `TracerProvider`. Your service (for example **BRRTRouter**) must install **one** provider and **one** `tracing_subscriber::Registry` stack. Optionally add **`lifeguard::channel_layer()`** to that same `.with(...)` chain so events also go through Lifeguard’s may-channel logger. See **[docs/OBSERVABILITY_APP_INTEGRATION.md](./docs/OBSERVABILITY_APP_INTEGRATION.md)** and the **`lifeguard::logging`** rustdoc.

## LifeReflector metrics

LifeReflector is a separate service ([microscaler/lifeguard-reflector](https://github.com/microscaler/lifeguard-reflector)) and documents its own `lifereflector_*` metrics in its README. Earlier design names, kept for reference:

- `reflector_notifications_total` — Notifications received
- `reflector_refreshes_total` — Cache refreshes
- `reflector_ignored_total` — Ignored notifications (inactive items)
- `reflector_active_keys` — Active cache keys
- `reflector_redis_latency_seconds` — Redis operation latency
- `reflector_pg_latency_seconds` — PostgreSQL operation latency
- `reflector_leader_changes_total` — Leader election events

---

[← README](./README.md) · [Operator guide & Kind/Grafana](./docs/OBSERVABILITY.md) · [App integration](./docs/OBSERVABILITY_APP_INTEGRATION.md)
