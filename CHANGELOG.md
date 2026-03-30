# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Documentation

- **PRD G9 / NFR3:** [`docs/POOLING_OPERATIONS.md`](./docs/POOLING_OPERATIONS.md) — operator tuning (lifetime vs `idle_session_timeout`, keepalive pointers), non-goals (PgBouncer), WAL retry vs `wal_lag_monitor_max_connect_retries`, migration notes, NFR evidence table.
- **Design doc:** [`docs/planning/DESIGN_CONNECTION_POOLING.md`](./docs/planning/DESIGN_CONNECTION_POOLING.md) — queue policy, metric names, connectivity heal pointer, PRD §9 decisions.

### Added

- **WAL monitor give-up (PRD R7.3):** `DatabaseConfig::wal_lag_monitor_max_connect_retries` / `LifeguardPoolSettings::wal_lag_monitor_max_connect_retries` — **`0`** = unlimited connect retries (default). When **`> 0`**, the monitor stops after that many failed replica connects, logs a warning, sets gauge **`lifeguard_wal_monitor_replica_routing_disabled`**, and `WalLagMonitor::is_replica_routing_disabled` / `LifeguardPool::is_replica_routing_disabled` become `true` (reads use primary).
- **Pool metrics + heal span (PRD R8.1 / R8.2):** Counters `lifeguard_pool_acquire_timeout_total`, `lifeguard_pool_slot_heal_total`, `lifeguard_pool_connection_rotated_total`; tracing span **`lifeguard.pool_slot_heal`** on successful slot heal.
- **Connection max lifetime (PRD R3.1 / R3.2):** `max_connection_lifetime_seconds` + `max_connection_lifetime_jitter_ms` — per-slot `Client` rotation after wall-clock age (with jitter) on fixed worker threads; **`0`** disables.
- **WAL lag policy (PRD R7.2):** `DatabaseConfig::wal_lag_max_bytes` / `wal_lag_max_apply_lag_seconds` and `LifeguardPoolSettings::wal_lag_max_bytes` / `wal_lag_max_apply_lag` — [`WalLagMonitor`](./src/pool/wal.rs) uses byte lag (receive vs replay LSN on the standby) and optionally **apply lag** in seconds (`clock_timestamp() - pg_last_xact_replay_timestamp()`). **`0`** disables each criterion; if both are disabled, the effective byte threshold remains **1 MiB** (historical default). Env: `LIFEGUARD__DATABASE__WAL_LAG_MAX_BYTES`, `LIFEGUARD__DATABASE__WAL_LAG_MAX_APPLY_LAG_SECONDS`. Public [`WalLagPolicy`](./src/pool/wal.rs) re-exported from the crate root.
- **Idle liveness probes (PRD R4.2):** Optional `DatabaseConfig::idle_liveness_interval_ms` / `LifeguardPoolSettings::idle_liveness_interval` — idle workers run `SELECT 1` on an interval so half-open TCP sessions are detected and healed via the existing slot-heal path. **`0`** / **`None`** disables probes (default). File/env values are clamped to **1s–1h**; use `LifeguardPoolSettings` directly for sub-second intervals in tests.
- **TCP keepalive operator doc (PRD R4.1):** `docs/POOL_TCP_KEEPALIVE.md` and `connection::connect` rustdoc describe libpq URI parameters (`keepalives`, `keepalives_idle`, etc.).
- **Pool slot heal (PRD §5.5):** Worker threads replace the `may_postgres::Client` after connectivity-class `Postgres` errors (SQLSTATE 08\*, shutdown codes, closed connection, transport `io` kinds). One reconnect attempt per job; application SQL errors do not trigger heal. See `src/pool/connectivity.rs`.

### Fixed

- **`DatabaseConfig::load`:** Correctly reads `config/config.toml` `[database]` by deserializing a `database` key (nested TOML). Previously, `[database]` values were not applied to the flat struct, so defaults (e.g. 30s pool timeout) could mask TOML. Environment overrides use **`LIFEGUARD__DATABASE__*`** (e.g. `LIFEGUARD__DATABASE__POOL_TIMEOUT_SECONDS`) so they match the file layout (PRD R2.2).

### Changed

- **Environment overrides** for database fields must use **`LIFEGUARD__DATABASE__<FIELD>`** (e.g. `LIFEGUARD__DATABASE__URL`). If you relied on root-style names such as `LIFEGUARD__POOL_TIMEOUT_SECONDS`, switch to the nested form.

### Documentation

- **Pool acquire default (PRD R1.3):** Default maximum wait for a worker slot is **30 seconds**, matching `DatabaseConfig::default().pool_timeout_seconds` and `LifeguardPoolSettings::default().acquire_timeout`. Documented on `LifeguardPool::new`, `LifeguardPoolSettings`, and `DatabaseConfig` in source.
