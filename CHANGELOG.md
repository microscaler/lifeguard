# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed — BREAKING

- **`LifeRecord` fields are `ActiveValue<T>`, not `Option<T>`.**

  A change-set records *intentions* about a row. Each column answers two
  independent questions — will we write it, and what will we write — and
  `Option<T>` has one slot for two answers, so "leave this column alone" and
  "write NULL to it" collided on `None` and no statement builder could tell
  them apart. The field now carries the state directly:

  | State | INSERT | UPDATE |
  | --- | --- | --- |
  | `NotSet` | omitted (column DEFAULT applies) | omitted |
  | `Unchanged(v)` | written (it is a real value) | omitted |
  | `Set(v)` | written | written |
  | `SetNull` | written as NULL (beats the DEFAULT) | written as NULL |
  | `Expr(e)` | rejected, naming the field | written as the expression |

  Two rules follow, and both are load-bearing:

  - **Calling a setter always has an effect.** `set_x(None)` writes SQL NULL;
    to leave a column alone, do not call its setter. New `set_x_null()` on
    every nullable column — the only way to express NULL for a `T` +
    `#[nullable]` field, whose setter takes `T`. Non-nullable columns get no
    such method, so "clear this" fails to compile.
  - **A loaded value is not a pending write.** `from_model` marks every column
    `Unchanged`, so an `UPDATE` from an edited row touches only the columns
    that were set and cannot clobber a concurrent edit to the rest of the row.

- **`Record::overwrite(&model)`** (new) stages every column, writing the model
  back wholesale. This is what a unit-of-work flush wants, and saying so at the
  call site distinguishes it from the read-modify-write that `from_model` now
  expresses.

- **`update()` with no staged columns is an error**, naming the record and
  pointing at `overwrite`. Previously this produced `UPDATE ... WHERE`, which
  the database rejects with a syntax error that points nowhere useful.

- **`insert()` rejects a staged expression per field**, naming the setter
  involved, rather than reporting a generic failure for the whole record.

- **`ActiveValue` is now the typed, per-field enum.** The former untyped
  `ActiveValue` (a `Set`/`NotSet`/`Unset` view over `sea_query::Value`) is
  renamed `ColumnValue`, and `ActiveModelTrait::into_active_value` becomes
  `into_column_value`. The internal `__update_exprs` map and the record's
  `null_columns` side table are gone: one field, one source of truth.

  **Migrating.** Code that goes through the generated setters needs no change —
  in hauliage and sesame-idam that was every call site. Two behaviour changes
  are worth checking:

  1. `set_x(None)` now writes NULL where it previously did nothing. Several
     call sites in both repos were already relying on the intended meaning:
     clearing an expiry after verification, clearing a draft payload after
     publishing, clearing an org's VAT or registration number when the field
     is submitted blank. All were silent no-ops; all now work.
  2. A `from_model(&m).update()` that sets nothing is now an error rather than
     a malformed statement. Unit-of-work flushes should use `overwrite`.

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
