# Story 05: Replica Read Support with WAL Lag Awareness

## Description

Implement intelligent read routing to PostgreSQL replicas based on WAL lag. Reads should route to replicas when healthy, fall back to primary when lagged.

## Acceptance Criteria

- [ ] WAL lag monitoring (`pg_current_wal_lsn()` vs `pg_last_wal_replay_lsn()`)
- [ ] Replica health checks (lag < threshold)
- [ ] Automatic routing: healthy replicas for reads, primary for writes
- [ ] Fallback to primary if replicas are lagged
- [ ] Configurable lag threshold
- [ ] Unit tests demonstrate replica routing

## Technical Details

- WAL lag check:
  ```sql
  SELECT pg_current_wal_lsn() - pg_last_wal_replay_lsn() AS lag;
  ```
- Health threshold: configurable (e.g., 1MB lag = unhealthy)
- Read routing:
  - Check replica health
  - If healthy: read from replica
  - If lagged: read from primary
- Write routing: always to primary
- Connection pool: separate pools for primary and replicas

## Dependencies

- Epic 01: Foundation (LifeguardPool)
- PostgreSQL replication setup

## Notes

- This relieves load on primary node
- WAL lag awareness prevents stale reads
- Critical for high-read workloads

