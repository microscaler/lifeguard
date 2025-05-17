# Performance Tuning

## Pool Size

The number of active database connections (`max_connections`) is critical for throughput.

- Start with `10–20` for most workloads.
- Increase only if:
    - You have CPU headroom
    - Postgres isn't queuing or rejecting connections
    - Coroutines are waiting excessively

## Batch Insert Size

Batching large inserts is far more efficient than single-row inserts.

- Default batch size: `500`
- Test scaling between `100–1000`
- Group inserts by entity type (e.g., owners, then pets)

## Coroutine Runtime

Lifeguard uses the [`may`](https://github.com/Xudong-Huang/may) runtime for coroutine concurrency.

- Do not spawn blocking system threads
- Use `may::go!` liberally—spawning costs are tiny
- Pool queue pressure is a signal of saturation

## PostgreSQL Settings

Consider tuning:

- `max_connections`
- `work_mem`
- `shared_buffers`
- `wal_level = minimal` (for ingest-only systems)
