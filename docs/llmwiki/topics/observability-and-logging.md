# Observability: metrics, tracing, channel logging

- **Status**: `verified`
- **Source docs**: [`OBSERVABILITY.md`](../../../OBSERVABILITY.md), [`docs/OBSERVABILITY.md`](../../OBSERVABILITY.md), [`docs/OBSERVABILITY_APP_INTEGRATION.md`](../../OBSERVABILITY_APP_INTEGRATION.md)
- **Code anchors**: `lifeguard/src/metrics/`, `lifeguard/src/logging/`
- **Last updated**: 2026-04-17

## What it is

Optional **`metrics`** (Prometheus) and **`tracing`** features integrate with standard OTLP pipelines. **Lifeguard does not install global subscribers** — the host application owns the OTel/`tracing` wiring (see app integration doc).

**Channel logging** (`logging` module) uses `may` mpsc for structured logs; see rustdoc for flush semantics.

## Cross-references

- [`entities/life-executor-pool-and-routing.md`](../entities/life-executor-pool-and-routing.md) (`pool_tier` labels)
- Grafana: [`grafana/README.md`](../../../grafana/README.md) if using bundled dashboards
