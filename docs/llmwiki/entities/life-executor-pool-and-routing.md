# `LifeExecutor`, `LifeguardPool`, and read routing

- **Status**: `verified`
- **Source docs**: [`docs/POOLING_OPERATIONS.md`](../../POOLING_OPERATIONS.md), [`docs/planning/PRD_CONNECTION_POOLING.md`](../../planning/PRD_CONNECTION_POOLING.md), [`ARCHITECTURE.md`](../../../ARCHITECTURE.md)
- **Code anchors**: [`src/pool/`](../../../src/pool/), [`src/executor/`](../../../src/executor/), [`src/lib.rs`](../../../src/lib.rs) re-exports
- **Last updated**: 2026-04-17

## What it is

- **`LifeExecutor`** abstracts execution over `may_postgres` (single client or pooled).
- **`LifeguardPool`** holds worker pools for **primary** and optional **replica** URLs, enforces acquire timeouts, optional idle liveness, and **WAL lag** aware routing for reads (`ReadPreference`, `WalLagPolicy`).
- **Writes** always go to the primary tier; **reads** may use replicas when healthy and allowed by preference.

## Operational docs

- Operator-facing tuning: [`docs/POOLING_OPERATIONS.md`](../../POOLING_OPERATIONS.md), [`docs/POOL_TCP_KEEPALIVE.md`](../../POOL_TCP_KEEPALIVE.md)
- Metrics: optional **`metrics`** feature — see [`OBSERVABILITY.md`](../../../OBSERVABILITY.md) and [`docs/OBSERVABILITY.md`](../../OBSERVABILITY.md)

## Gotchas

> **Drift:** Public API details change with PRD work — prefer **`cargo doc -p lifeguard`** for exact method names at your revision.

## Cross-references

- [`topics/observability-and-logging.md`](../topics/observability-and-logging.md)
- [`reference/workspace-and-module-map.md`](../reference/workspace-and-module-map.md)
