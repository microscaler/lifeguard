# Story 04: Redesign LifeguardPool for may_postgres

## Description

Redesign `LifeguardPool` to work with `may_postgres` connections instead of SeaORM connections. The pool must use persistent connections with semaphore-based acquisition.

## Acceptance Criteria

- [ ] `LifeguardPool` manages persistent `may_postgres` connections
- [ ] Semaphore-based concurrency control prevents connection storms
- [ ] Connections are pre-allocated at pool creation (not on-demand)
- [ ] Health monitoring detects and reconnects failed connections
- [ ] Pool size is configurable (min/max connections)
- [ ] Unit tests demonstrate connection acquisition and release
- [ ] Load tests show pool handles concurrent requests efficiently

## Technical Details

- Pool should maintain a fixed set of connections (no dynamic allocation)
- Use `may::sync::Semaphore` for concurrency control
- Each connection slot tracks: `in_use: bool`, `last_used: Instant`, `connection: may_postgres::Connection`
- Health check: ping database periodically, reconnect on failure
- Configuration: `min_connections`, `max_connections`, `connection_timeout`, `idle_timeout`

## Dependencies

- Story 02: Integrate may_postgres as Database Client
- Story 03: Implement LifeExecutor Trait

## Notes

- This is critical for Pricewhisperers scale (100-500 connections handling millions of requests)
- Connection reuse is essential (no connection churn)
- Health monitoring prevents dead connections from blocking the pool

