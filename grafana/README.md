# Grafana JSON in this directory

Some files under `grafana/` and `config/k8s/observability/` were hand-written before metric names stabilized.

**Canonical metric names and semantics** are defined in [`../src/metrics.rs`](../src/metrics.rs) (rustdoc table at the top). Example: `lifeguard_query_duration_seconds`, `lifeguard_pool_acquire_timeout_total`, `lifeguard_pool_workers` (label `pool_tier`).

**Hauliage** ships maintained dashboards that track the current instruments:

- `hauliage/k8s/observability/dashboards/hauliage-lifeguard.json` (UID `hauliage-lifeguard`)

Prefer that dashboard for production-style clusters; update JSON here when you change Lifeguard’s Prometheus instruments.
