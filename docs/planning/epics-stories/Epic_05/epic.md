# Epic 05: Advanced Features

## Overview

Implement advanced features including LifeReflector (distributed cache coherence), Redis integration, replica read support with WAL lag awareness, relation loading, and materialized views.

## Goals

- **LifeReflector (distributed cache coherence)** - Critical for use at scale
  - Leader-elected Raft system
  - PostgreSQL LISTEN/NOTIFY integration
  - Redis cache coherence with TTL-based active set
  - Prevents cache stampedes and thundering herd at millions of requests/second
  - Enables 99%+ cache hit rate with zero-stale reads
- Redis integration for transparent caching
- Replica read support with WAL lag awareness
- **Complete relation support** (replicates SeaORM's Relation enum, Related trait, all join types, eager/lazy loading)
- Materialized views and generated columns

## Success Criteria

- LifeReflector maintains cache coherence across microservices
- Redis integration provides transparent caching (check Redis first, fall back to database)
- Replica reads automatically route to healthy replicas
- WAL lag monitoring prevents stale reads
- Complete relation support: Relation enum generation, Related trait, all relation types (has_one, has_many, belongs_to, many_to_many)
- All join operations supported: `join()`, `left_join()`, `right_join()`, `inner_join()`, `join_rev()`
- Relations can be loaded with joins (eager) or separate queries (lazy)
- N+1 query prevention via batch loading
- Materialized views supported in LifeModel

## Timeline

**Weeks 10-14**

## Dependencies

- Epic 04: v1 Release (must be complete)
- Redis (external service)
- PostgreSQL replication setup

## Technical Notes

- LifeReflector should be a standalone microservice
- Raft leader election ensures only one active reflector
- LISTEN/NOTIFY subscriptions for all tables
- TTL-based active set means only active items cached
- Replica health checks via `pg_current_wal_lsn()` vs `pg_last_wal_replay_lsn()`
- Automatic fallback to primary if replicas are lagged

## Related Epics

- Epic 04: v1 Release (prerequisite)
- Epic 06: Enterprise Features (follows this epic)

