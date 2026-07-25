# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Fixed

- **`LifeRecord`: staged SQL `NULL` is now actually written (data-loss bug).**
  A record field is `Option<T>` and statements only include fields that were
  touched, so `None` was ambiguous — "never touched" and "set me to NULL" were
  indistinguishable. `update()` emitted **no SET clause** for `set_x(None)`, so
  the old value survived while the call reported success; when a NULL was the
  *only* staged change the generated statement was `UPDATE ... WHERE`, a syntax
  error. Records now track explicitly staged NULLs (`__null_columns`) and emit
  a correctly **typed** null per column (`Value::String(None)` bound to an
  INTEGER or UUID column is a parameter-type error, not a NULL).

  - `set_x(None)` on an `Option<T>` field means SQL `NULL`. Calling a setter
    always has an effect; to leave a column alone, do not call its setter.
  - New `set_x_null()` on every nullable column — the only way to express NULL
    for a `T` + `#[nullable]` field, whose setter takes `T`. Non-nullable
    columns get no such method, so "clear this" fails to compile.
  - On INSERT an explicit NULL beats the column DEFAULT; an untouched column
    still takes the DEFAULT.
  - `take()` un-stages a NULL (it removes the column from the change-set);
    `reset()` clears staged NULLs; `set_x_expr()` and a staged NULL are
    mutually exclusive, last write wins; a staged NULL counts as dirty, and
    `null_columns()` reports them.

  **Reach beyond `update()`.** Every mutation in the crate is built by the
  derive's `insert` / `update` / `delete`; there is no second statement
  builder, so the fix covers the ORM rather than only hand-written SQL:

  - **Soft delete restore.** `delete()` on a `#[soft_delete]` entity was never
    affected (it builds its own `UPDATE ... SET deleted_at = now()`), but there
    is no `restore()` API — un-deleting means clearing `deleted_at`, which was
    the operation that vanished. Soft-deleted rows could not be brought back
    through the ORM at all, and since `find()` filters on `deleted_at IS NULL`
    they stayed invisible to every generated query.
  - **Identity map / session flush.** `flush_dirty` persists via a
    caller-supplied `Record::update()` and so inherited the bug; a change-set
    whose only pending change was a NULL is now correctly dirty and flushes.
  - **Row Level Security.** A clear is now a real write, so `WITH CHECK`
    policies adjudicate it. Previously the SET clause was dropped, the policy
    never saw the write, and the statement reported success — the application
    believed it had cleared a column the database would have refused to let it
    clear. `USING` row scoping is unchanged: a clear still cannot reach a row
    the policy hides.

  **Migration:** code that called `set_x(None)` expecting "leave unchanged" now
  writes NULL. In the full-record rebuild pattern (`from_model` → set → update)
  the value written equals the value read, so behaviour is unchanged. Two real
  call sites in this workspace were already *relying* on the intended
  semantics and silently doing nothing. Covered by
  `tests/db_integration/nullable_null_update.rs` (6 tests that fail without the
  fix, 5 that pin the "leave untouched columns alone" contract) and
  `tests/db_integration/nullable_orm_and_rls.rs` (soft-delete restore, session
  flush, and RLS under both `USING` and `WITH CHECK`).

### Added

- **Chrono (`DateTime<Utc>` / `Local`, `NaiveDateTime`, …):** Derive `type_conversion` and `LifeModel` / `LifeRecord` align `sea_query::Value` variants with PostgreSQL time types; `FromRow` uses direct `try_get` for tz-aware chrono types; soft-delete `UPDATE` emits typed “now” for `deleted_at` / `updated_at` by model field type. See [`docs/CHRONO_AND_POSTGRES_TYPES.md`](./docs/CHRONO_AND_POSTGRES_TYPES.md) and [`docs/COMPLETE_CHRONO_IMPLEMENTATION.md`](./docs/COMPLETE_CHRONO_IMPLEMENTATION.md). **Additive:** existing `NaiveDateTime` / `timestamp without time zone` usage is unchanged.

### Documentation

- **README / observability:** Pool section reflects shipped pool features, `pool_tier` metrics, and links to [`docs/POOLING_OPERATIONS.md`](./docs/POOLING_OPERATIONS.md), [`docs/planning/DESIGN_CONNECTION_POOLING.md`](./docs/planning/DESIGN_CONNECTION_POOLING.md), and [`docs/OBSERVABILITY.md`](./docs/OBSERVABILITY.md). [`docs/OBSERVABILITY.md`](./docs/OBSERVABILITY.md) metric table documents label columns.
- **Rustdoc:** `lib.rs` and [`src/pool/mod.rs`](./src/pool/mod.rs) add cross-links for WAL types, metrics, and GitHub ops/design docs.
- **Pool acquire default (PRD R1.3):** Default maximum wait for a worker slot is **30 seconds**, matching `DatabaseConfig::default().pool_timeout_seconds` and `LifeguardPoolSettings::default().acquire_timeout`. Documented on `LifeguardPool::new`, `LifeguardPoolSettings`, and `DatabaseConfig` in source.
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

- **`Value::SmallInt` / `TinyInt` (and small unsigned) → `ToSql`:** `converted_params` and pool [`OwnedParam`](./src/pool/owned_param.rs) now keep PostgreSQL **`INT2`** (`i16`) binds distinct from `INT4` (`i32`). Previously, `SMALLINT` columns could fail with “cannot convert between the Rust type `i32` and the Postgres type `int2`” on insert/update.
- **`Value::Json` → `ToSql`:** `converted_params` now binds `serde_json::Value` directly (JSON/JSONB), not a serialized string, so `JSONB` columns accept ORM inserts/updates.
- **Iteration D4 — typed SQL NULLs:** `String(None)`, `Bytes(None)`, and `Json(None)` no longer share the generic `Option<i32>` NULL placeholder in `converted_params`; pool `OwnedParam` uses `String`/`Bytes`/`Json(Option<…>)` for the same. Avoids OID / `accepts` mismatches on `TEXT`, `BYTEA`, and JSON/JSONB nullable columns.

### Changed

- **`OwnedParam::Json`:** is now `Json(Option<serde_json::Value>)` (SQL NULL = `None`) instead of routing JSON null through `GenericNull`. Update exhaustive matches if you branch on `OwnedParam` outside the crate.
- **`DatabaseConfig::load`:** Correctly reads `config/config.toml` `[database]` by deserializing a `database` key (nested TOML). Previously, `[database]` values were not applied to the flat struct, so defaults (e.g. 30s pool timeout) could mask TOML. Environment overrides use **`LIFEGUARD__DATABASE__*`** (e.g. `LIFEGUARD__DATABASE__POOL_TIMEOUT_SECONDS`) so they match the file layout (PRD R2.2).

### Changed

- **Metrics (`pool_tier` cardinality):** Gauge **`lifeguard_pool_workers`** records primary/replica slot counts with label **`pool_tier`**. Pool counters and histograms that were unlabeled now attach **`pool_tier`** where the work runs (`primary` \| `replica`). **`METRICS.record_query_duration`**, **`record_query_error`**, and **`record_connection_wait`** take an optional `pool_tier` (`None` for non-pooled paths such as direct [`connect`](./src/connection.rs)). **`record_pool_acquire_timeout`**, **`record_pool_slot_heal`**, and **`record_pool_connection_rotated`** take a tier string. Use **`set_pool_workers_by_tier`** when opening a pool (called from [`LifeguardPool::new_with_settings`](./src/pool/pooled.rs)).
- **Environment overrides** for database fields must use **`LIFEGUARD__DATABASE__<FIELD>`** (e.g. `LIFEGUARD__DATABASE__URL`). If you relied on root-style names such as `LIFEGUARD__POOL_TIMEOUT_SECONDS`, switch to the nested form.
