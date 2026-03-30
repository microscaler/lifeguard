# Lifeguard connection pool — operations & tuning

Audience: operators and service owners integrating [`LifeguardPool`](../src/pool/pooled.rs). For the full requirement list, see [PRD_CONNECTION_POOLING.md](./planning/PRD_CONNECTION_POOLING.md). For TCP keepalive URI parameters, see [POOL_TCP_KEEPALIVE.md](./POOL_TCP_KEEPALIVE.md). For metrics and Grafana, see [OBSERVABILITY.md](./OBSERVABILITY.md).

## Non-goals (use external tools)

- **PgBouncer-style multiplexing** (transaction/statement pool modes, fleet-wide queueing) is **out of scope** for the in-process pool. Run **[PgBouncer](https://www.pgbouncer.org/)** (or a cloud proxy) **beside** the app when you need connection multiplication across many clients.
- The pool does **not** implement a **prepared statement cache** at the pool layer; rely on driver/server behavior.
- **Global query cancel** / pool-level `statement_timeout` as core policy are non-goals; use server `statement_timeout` and application policy.

## Configuration surface

- **File:** `config/config.toml` → `[database]` (see [`DatabaseConfig`](../src/pool/config.rs) rustdoc).
- **Environment:** `LIFEGUARD__DATABASE__*` nested keys (e.g. `LIFEGUARD__DATABASE__POOL_TIMEOUT_SECONDS`). Same field names as TOML, merged after the file.
- **Programmatic:** [`LifeguardPool::new_with_settings`] with [`LifeguardPoolSettings`] for tests and embedders that bypass file/env.

## Tuning: `max_connection_lifetime` vs Postgres & network

| Knob | Role |
|------|------|
| **`max_connection_lifetime_seconds`** (+ **`max_connection_lifetime_jitter_ms`**) | Closes **client** sessions in pool workers after wall-clock age (with jitter per slot). Aligns with credential rotation and avoiding unbounded session age. **`0`** disables. |
| Postgres **`idle_session_timeout`** | Server closes **idle** sessions; set **longer** than your expected idle between queries **or** rely on pool idle liveness / traffic so slots stay warm. |
| Postgres **`tcp_keepalives_*`** / URL params | OS/driver keepalive; see [POOL_TCP_KEEPALIVE.md](./POOL_TCP_KEEPALIVE.md). Complements pool **`idle_liveness_interval_ms`**. |
| Firewall / LB idle TCP | Half-open connections may not be detected until a query runs; use keepalive + optional **`idle_liveness_interval_ms`**. |

**Rule of thumb:** Pool **max lifetime** addresses **client-side** age and rotation; server **idle** and **firewall** limits must still be satisfied by keepalive, liveness probes, or traffic patterns.

## WAL lag monitor: retries vs give-up

- **Initial connect to the replica** uses exponential backoff (PRD R7.1).
- **`wal_lag_monitor_max_connect_retries`:** **`0`** (default) = **no cap** — the monitor thread keeps retrying forever. **`N > 0`** = after **`N`** failed attempts, the monitor **gives up** (PRD R7.3): logs a **warning**, sets metric **`lifeguard_wal_monitor_replica_routing_disabled`**, and **`is_replica_routing_disabled()`** becomes true so **reads use the primary** until process restart.
- **Lag policy** (bytes / apply lag) is separate: see PRD §5.7 and [`WalLagPolicy`](../src/pool/wal.rs).

## Observability

Prometheus metric names and tracing spans are listed in [OBSERVABILITY.md](./OBSERVABILITY.md). Replica lag on the **database** side (e.g. `pg_stat_replication`) should still be monitored in Grafana; the in-app monitor is for **routing** only.

## Migration notes (embedders)

- Prefer **`LifeguardPool::from_database_config`** when using **`DatabaseConfig::load`** so file + `LIFEGUARD__DATABASE__*` stay aligned.
- New **`[database]`** fields are optional with serde defaults; older TOML files without them keep prior behavior (see [CHANGELOG.md](../CHANGELOG.md) `[Unreleased]` / release entries).
- **`LifeguardPool::new(url, …)`** remains for simple call sites; defaults are documented on [`LifeguardPool::new`](../src/pool/pooled.rs).

## NFR verification

| NFR | Evidence |
|-----|----------|
| **NFR1** | `cargo check -p lifeguard`, `cargo check --manifest-path examples/perf-idam/Cargo.toml`, `cargo check --manifest-path examples/entities/Cargo.toml` succeed on supported toolchains. |
| **NFR2** | Time-based pool/WAL behavior uses short `Duration`s in tests or configurable ms fields; integration tests use testcontainers with bounded waits. |
| **NFR3** | This document + [`lib.rs`](../src/lib.rs) pool section + PRD cross-links. |
| **NFR4** | Idle liveness and WAL polling run on **idle** paths or background threads; hot dispatch path does not add per-query `SELECT 1` unless idle probe interval is set. |
