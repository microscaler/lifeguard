# `LifeExecutor`, `LifeguardPool`, and read routing

- **Status**: `verified`
- **Source docs**: [`docs/POOLING_OPERATIONS.md`](../../POOLING_OPERATIONS.md), [`docs/planning/PRD_CONNECTION_POOLING.md`](../../planning/PRD_CONNECTION_POOLING.md), [`ARCHITECTURE.md`](../../../ARCHITECTURE.md)
- **Code anchors**: [`src/pool/`](../../../src/pool/), [`src/executor/`](../../../src/executor/), [`src/lib.rs`](../../../src/lib.rs) re-exports
- **Last updated**: 2026-07-14

## What it is

- **`LifeExecutor`** abstracts execution over `may_postgres` (single client or pooled).
- **`LifeguardPool`** holds worker pools for **primary** and optional **replica** URLs, enforces acquire timeouts, optional idle liveness, and **WAL lag** aware routing for reads (`ReadPreference`, `WalLagPolicy`).
- **Writes** always go to the primary tier; **reads** may use replicas when healthy and allowed by preference.

## RLS session context

`MayPostgresExecutor`, transactions, and pooled workers can carry a
`SessionContext`. Context-aware execution calls the application-owned,
schema-qualified `public.rls_set_session(uuid, uuid, text, text, jsonb, text)`
helper before the application query. A missing helper is an error; the query is
not allowed to continue without its requested tenant context. Consumers must
install the helper in `public` and grant callers `USAGE` on that schema plus
`EXECUTE` on the function.

The current helper contract uses session-scoped GUCs so direct autocommit calls
can inject context before a separate query. Transaction-local semantics require
the setter and application statement to share an explicit transaction boundary.

## Operational docs

- Operator-facing tuning: [`docs/POOLING_OPERATIONS.md`](../../POOLING_OPERATIONS.md), [`docs/POOL_TCP_KEEPALIVE.md`](../../POOL_TCP_KEEPALIVE.md)
- Metrics: optional **`metrics`** feature — see [`OBSERVABILITY.md`](../../../OBSERVABILITY.md) and [`docs/OBSERVABILITY.md`](../../OBSERVABILITY.md)

## Gotchas

> **Drift:** Public API details change with PRD work — prefer **`cargo doc -p lifeguard`** for exact method names at your revision.

## Cross-references

- [`topics/observability-and-logging.md`](../topics/observability-and-logging.md)
- [`entities/transaction-boundaries.md`](./transaction-boundaries.md)
- [`reference/workspace-and-module-map.md`](../reference/workspace-and-module-map.md)
