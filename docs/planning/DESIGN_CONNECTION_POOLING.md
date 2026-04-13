# Design: Lifeguard connection pooling

**Status:** Active companion to [PRD_CONNECTION_POOLING.md](./PRD_CONNECTION_POOLING.md).  
**Code:** [`src/pool/pooled.rs`](../../src/pool/pooled.rs), [`src/pool/wal.rs`](../../src/pool/wal.rs), [`src/pool/config.rs`](../../src/pool/config.rs), [`src/pool/connectivity.rs`](../../src/pool/connectivity.rs).

## Architecture summary

- **Fixed-size worker tiers:** one OS thread per primary (and optional replica) slot; each owns one [`may_postgres::Client`].
- **Dispatch:** [`crossbeam_channel::bounded`](https://docs.rs/crossbeam-channel) per worker; producers use [`send_timeout`](https://docs.rs/crossbeam-channel/latest/crossbeam_channel/struct.Sender.html#method.send_timeout) bounded by [`LifeguardPoolSettings::acquire_timeout`](../../src/pool/config.rs). Failure → [`LifeError::PoolAcquireTimeout`](../../src/executor.rs).
- **Replies (Cooperative Yielding):** The underlying request flow originates from cooperative `may` coroutines mapping to `WorkerJob`s. Because HTTP service coroutines multiplex over a small set of OS scheduler threads, worker responses flow through a decoupled, unbounded `may::sync::mpsc::channel()` instead of OS-locking channels (like `std::sync::mpsc::sync_channel`). This protects the core `may` scheduler from halting/deadlocking when large concurrent web tier requests wait on PostgreSQL replies.
- **Reads:** [`LifeguardPool`](../../src/pool/pooled.rs) chooses **replica** workers when a replica tier exists and [`WalLagMonitor`](../../src/pool/wal.rs) reports acceptable lag; otherwise **primary**.

## Queue policy (PRD §9.2)

**Decision:** When a worker queue is full, callers **block with timeout** until [`LifeguardPoolSettings::acquire_timeout`](../../src/pool/config.rs) elapses, then fail with **`PoolAcquireTimeout`**. No unbounded wait; no drop-oldest in the current implementation.

## Replica routing & WAL monitor

- **Background thread** on the replica URL polls lag (bytes ± apply time); see [`WalLagPolicy`](../../src/pool/wal.rs).
- **Initial connect:** retry with backoff; optional **`wal_lag_monitor_max_connect_retries`** — `0` = unlimited retries, `N > 0` = give up after N failures (primary-only reads, observable via log + metric + [`is_replica_routing_disabled`](../../src/pool/wal.rs)).
- **Replica heal policy:** same connectivity heuristic as primary slots ([`connectivity.rs`](../../src/pool/connectivity.rs)); no stricter replica-only path in code today.

## Error taxonomy (connectivity vs application)

Slot **heal** runs only for errors classified in [`life_error_is_connectivity_heal_candidate`](../../src/pool/connectivity.rs) (roughly: SQLSTATE 08*, connection closed, certain I/O). Ordinary SQL/query errors do **not** trigger client replacement.

## Metric names (Prometheus)

| Metric | Type | Notes |
|--------|------|--------|
| `lifeguard_pool_size` | Gauge | Configured pool width |
| `lifeguard_active_connections` | Gauge | Same as pool size at init |
| `lifeguard_connection_wait_time_seconds` | Histogram | Wait to enqueue on worker |
| `lifeguard_query_duration_seconds` | Histogram | Query execution |
| `lifeguard_query_errors_total` | Counter | Query errors |
| `lifeguard_wal_monitor_replica_routing_disabled` | Gauge | 1 after monitor give-up |
| `lifeguard_pool_acquire_timeout_total` | Counter | `PoolAcquireTimeout` |
| `lifeguard_pool_slot_heal_total` | Counter | Heal reconnects |
| `lifeguard_pool_connection_rotated_total` | Counter | Max-lifetime rotations |

## Open product choices (PRD §9) — current stance

| Topic | Decision |
|-------|----------|
| **`LifeguardPool::new` vs richer config struct** | **`new`** + **`new_with_settings`** + **`from_database_config`** cover current needs; a single `PoolConfig` struct can be a future refactor with a deprecation window. |
| **Dynamic pool size** | **Fixed** worker count; “shrink” is **not** implemented — rotation is **in-slot** (`Client` replace). |
| **Minimum idle / shrink** | Not implemented; idle policy is **between-job** checks + optional idle `SELECT 1` + max lifetime. |

## Related

- [POOLING_OPERATIONS.md](../POOLING_OPERATIONS.md) — operator tuning.
- [PRD_CONNECTION_POOLING.md](./PRD_CONNECTION_POOLING.md) — requirements and checklists.
