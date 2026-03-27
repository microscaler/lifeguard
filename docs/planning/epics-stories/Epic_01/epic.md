# Epic 01: Foundation

## Overview

Establish the foundational architecture for Lifeguard by removing incompatible dependencies and building the core database connection infrastructure using `may_postgres`.

## Goals

- Remove SeaORM and Tokio dependencies (incompatible with `may` coroutines)
- Integrate `may_postgres` as the native database client
- Implement `LifeExecutor` trait for database operations
- Redesign `LifeguardPool` for `may_postgres` connections
- Add basic metrics and observability
- Implement transaction support (replicates SeaORM's transaction API)
- Implement raw SQL helpers (replicates SeaORM's `find_by_statement()`, `execute_unprepared()`)

## Success Criteria

- Zero SeaORM/Tokio dependencies in core Lifeguard code
- `may_postgres` successfully integrated and tested
- `LifeExecutor` trait defined and implemented
- `LifeguardPool` manages persistent `may_postgres` connections
- Basic Prometheus metrics and OpenTelemetry tracing in place
- Connection pooling works with semaphore-based acquisition
- Transaction support: `pool.begin()`, `transaction.commit()`, `transaction.rollback()`
- Raw SQL helpers: `find_by_statement()`, `execute_unprepared()`

## Timeline

**Weeks 1-3**

## Dependencies

- `may_postgres` crate (external dependency)
- `may` coroutine runtime (external dependency)

## Technical Notes

- Connection pool must use persistent connections (no on-demand creation)
- Semaphore-based concurrency control to prevent connection storms
- Health monitoring for automatic reconnection of failed connections
- Metrics should track: connection pool size, active connections, wait times, query durations

## Related Epics

- Epic 02: ORM Core (depends on this epic)

