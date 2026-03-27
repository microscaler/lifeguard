# Story 07: WAL-Based Replica Health Monitoring

## Description

Implement WAL-based replica health monitoring that tracks replica lag using PostgreSQL WAL positions. This enables safe replica reads and automatic failover.

## Acceptance Criteria

- [ ] `pg_current_wal_lsn()` query on primary
- [ ] `pg_last_wal_replay_lsn()` query on replicas
- [ ] Lag calculation (bytes and seconds)
- [ ] Replica health status tracking
- [ ] Automatic replica routing based on health
- [ ] Metrics: `lifeguard_replica_lag_bytes`, `lifeguard_replica_lag_seconds`, `lifeguard_replicas_healthy`
- [ ] Unit tests demonstrate replica health monitoring

## Technical Details

- WAL position queries:
  ```sql
  -- Primary
  SELECT pg_current_wal_lsn();
  
  -- Replica
  SELECT pg_last_wal_replay_lsn();
  ```
- Lag calculation:
  ```rust
  let lag_bytes = current_lsn - replay_lsn;
  let lag_seconds = calculate_lag_seconds(lag_bytes, replication_rate);
  ```
- Health thresholds:
  - `lag_seconds < 1.0` → healthy
  - `lag_bytes < 1_000_000` (1MB) → healthy
- Replica routing:
  - If healthy → use replica for reads
  - If unhealthy → fallback to primary
- Periodic health checks (every 1-5 seconds)

## Dependencies

- Epic 04: LifeExecutor & LifeguardPool (LifeguardPool must be complete)
- Epic 01 Story 05: Basic Metrics and Observability

## Notes

- Critical for safe replica reads
- Should match Epic 05 Story 05 requirements
- Consider caching health status to avoid query overhead

