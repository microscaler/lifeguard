# Lifeguard Migration Audit: Strip & Replace Analysis

**Version:** 1.0  
**Date:** 2025-01-XX  
**Purpose:** Comprehensive audit of components to remove and replace based on final PRD requirements

---

## Executive Summary

The current Lifeguard implementation uses a **bridge architecture** that wraps SeaORM/Tokio inside coroutines. This must be **completely replaced** with a native coroutine architecture using `may_postgres`. This document tabulates every component that must be stripped out and what replaces it.

---

## Audit Table: Strip & Replace

| Component | Current Implementation | Must Strip Out | Replacement | Why Strip | Benefits of Replacement |
|-----------|----------------------|----------------|-------------|-----------|------------------------|
| **Database Client** | `sea-orm` with `sqlx-postgres` | ✅ Entire SeaORM dependency chain | `may_postgres` | SeaORM is async-only, incompatible with coroutines. Creates async overhead defeating coroutine performance benefits. | Zero async overhead, native coroutine I/O, 2-5× faster on hot paths, 10×+ faster on small queries |
| **Async Runtime** | `tokio` runtime in worker threads | ✅ Tokio runtime creation in workers | Pure coroutine execution | Tokio adds future polling overhead, context switching, and memory allocation. Defeats purpose of coroutines. | Deterministic scheduling, lower memory footprint, predictable latency, no future allocation overhead |
| **Connection Pool Architecture** | Worker threads with Tokio runtimes, channels for job queuing | ✅ `DbPoolManager` worker loop pattern | Persistent connection slots with semaphore | Current design creates connections per job (expensive) or uses async runtime (overhead). Not persistent pooled connections. | Pre-allocated connections, aggressive reuse, bounded concurrency, no connection churn, handles millions of requests with 100-500 connections |
| **Database Connection Type** | `sea_orm::DatabaseConnection` | ✅ All SeaORM connection types | `may_postgres::Client` | SeaORM connections are async-only, require Tokio. Cannot be used in coroutine context. | Synchronous, coroutine-native, no Send/Sync constraints, direct Postgres protocol |
| **Executor Abstraction** | SeaORM's `ConnectionTrait` | ✅ `ConnectionTrait` implementation | `LifeExecutor` trait | SeaORM trait is async-only, tied to async ecosystem. Not usable with coroutines. | Simple sync trait, works with `may_postgres`, usable by ORM and migrations, clean abstraction |
| **ORM Layer** | SeaORM entities (`EntityTrait`, `Model`, `ActiveModel`) | ✅ All SeaORM entity code | `LifeModel` and `LifeRecord` derive macros | SeaORM entities require async runtime, cannot work with coroutines. Exposes SeaORM types to users. | Coroutine-native, no async, clean API, zero SeaORM exposure, type-safe, compile-time validated |
| **Query Execution** | `async fn` methods via Tokio | ✅ All async query paths | Synchronous methods using `may_postgres` | Async methods require Tokio runtime, add overhead. Cannot be called from coroutines. | Direct coroutine execution, no async tax, predictable performance, simpler code paths |
| **Transaction Management** | SeaORM async transactions | ✅ SeaORM transaction code | `LifeTransaction` using `may_postgres` | SeaORM transactions are async-only. Cannot be used in coroutine context. | Synchronous transactions, coroutine-safe, no async overhead |
| **Migration System** | SeaORM migrations via CLI | ✅ SeaORM migration dependency | `LifeMigration` trait + runner | SeaORM migrations require async runtime. Need Lifeguard-native system. | Coroutine-native, uses `LifeExecutor`, clean API, no SeaORM exposure |
| **Error Types** | `sea_orm::DbErr` | ✅ SeaORM error types | `LifeError` enum | SeaORM errors expose async runtime details. Need Lifeguard-branded errors. | Clean error API, no SeaORM exposure, coroutine-appropriate error handling |
| **Connection Pool Manager** | `DbPoolManager` with worker threads | ✅ Entire `DbPoolManager` implementation | `LifeguardPool` with persistent slots | Current manager spawns Tokio runtimes, uses channels. Not persistent connections. | Persistent connection slots, semaphore-based acquisition, health monitoring, automatic reconnection |
| **Worker Loop Pattern** | `run_worker_loop` with async | ✅ Worker loop with Tokio | Connection slot management | Worker loop pattern creates async overhead. Need direct connection slot management. | Direct connection reuse, no worker overhead, simpler architecture |
| **Job Queue System** | `crossbeam-channel` for jobs | ✅ Job queue abstraction | Direct connection acquisition | Job queue adds indirection. Need direct connection slot access. | Lower latency, simpler code, direct connection access |
| **Channel-Based Communication** | `crossbeam-channel` for results | ✅ Channel result passing | Direct return values | Channels add overhead and complexity. Coroutines can return directly. | Simpler code, lower latency, direct return paths |
| **Test Infrastructure** | SeaORM entity tests | ✅ SeaORM entity test code | `LifeModel`/`LifeRecord` test helpers | Tests use SeaORM entities. Need tests for new ORM layer. | Tests for coroutine-native ORM, no async test overhead, faster test execution |
| **Macro Helpers** | `lifeguard_execute!`, `lifeguard_query!` | ✅ All SeaORM-wrapping macros | `LifeModel`/`LifeRecord` macros | Current macros wrap SeaORM. Need ORM-layer macros. | Direct ORM macros, no SeaORM wrapping, cleaner API |
| **Connection Interface** | `LifeguardConnection` implementing `ConnectionTrait` | ✅ `LifeguardConnection` | Direct `may_postgres::Client` access | Current connection wraps SeaORM. Need direct `may_postgres` access. | Direct database access, no wrapping overhead, simpler interface |
| **Dependencies** | `sea-orm`, `tokio`, `async-trait` | ✅ All async dependencies | `may_postgres`, `sea-query` | Async dependencies incompatible with coroutines. Need coroutine-native stack. | Smaller dependency tree, no async runtime, faster builds, lower memory footprint |
| **Caching Layer** | None | N/A | `LifeReflector` + Redis integration | No caching exists. Need distributed cache coherence. | Oracle Coherence-level functionality, zero-stale reads, cluster-wide consistency, TTL-based active sets |
| **Replica Support** | None | N/A | WAL lag monitoring + replica routing | No replica support. Need read replica routing with health checks. | Offload read traffic, reduce primary load, automatic health monitoring, WAL-based consistency |
| **Read Preferences** | None | N/A | `ReadPreference` enum (primary/replica/mixed/strong) | No read preference system. Need configurable read routing. | Flexible read strategies, performance optimization, consistency guarantees |
| **Connection Health Monitoring** | None | N/A | Per-slot health tracking + reconnection | No health monitoring. Need automatic connection recovery. | Automatic failure recovery, pool health metrics, degraded mode handling |
| **Metrics Integration** | Basic OpenTelemetry | ⚠️ Keep but extend | Extended metrics for new architecture | Current metrics exist but need extension for new components. | Pool health, replica lag, cache hit rates, connection lifecycle |
| **Configuration System** | Basic TOML config | ⚠️ Keep but extend | Extended config for pool, replicas, cache | Current config exists but needs extension. | Replica URLs, cache TTLs, read preferences, pool sizing |
| **Test Helpers** | `test_helpers.rs` | ⚠️ Keep but adapt | Adapt for `may_postgres` | Current helpers exist but need adaptation. | Test infrastructure for new architecture |

---

## Detailed Component Analysis

### 1. Database Client Layer

**Current:**
- `sea-orm` with `sqlx-postgres` backend
- Async-only API
- Requires Tokio runtime

**Replacement:**
- `may_postgres` crate
- Synchronous, coroutine-native API
- Direct Postgres protocol

**Impact:**
- **High** - Core dependency change affecting entire codebase
- **Benefits:** 2-5× performance improvement, zero async overhead

---

### 2. Connection Pool Architecture

**Current:**
- `DbPoolManager` spawns worker threads
- Each worker runs Tokio runtime
- Channels for job queuing
- Connections created per job or shared via async runtime

**Replacement:**
- `LifeguardPool` with persistent connection slots
- Semaphore-based acquisition
- Pre-allocated connections at startup
- Direct connection reuse

**Impact:**
- **Critical** - Complete architectural change
- **Benefits:** Handles millions of requests with limited connections, no connection churn, predictable performance

---

### 3. ORM Layer

**Current:**
- SeaORM entities (`EntityTrait`, `Model`, `ActiveModel`)
- Async methods
- SeaORM types exposed to users

**Replacement:**
- `LifeModel` derive macro (immutable rows)
- `LifeRecord` derive macro (mutations)
- Synchronous methods
- Zero SeaORM exposure

**Impact:**
- **Critical** - Complete ORM rewrite
- **Benefits:** Coroutine-native, clean API, no async, type-safe

---

### 4. Executor Abstraction

**Current:**
- SeaORM's `ConnectionTrait` (async)
- Tied to async ecosystem

**Replacement:**
- `LifeExecutor` trait (synchronous)
- Works with `may_postgres`
- Used by ORM and migrations

**Impact:**
- **High** - Core abstraction change
- **Benefits:** Clean abstraction, coroutine-compatible, reusable

---

### 5. Migration System

**Current:**
- SeaORM migrations via `sea-orm-cli`
- Async migration trait
- SeaORM dependency

**Replacement:**
- `LifeMigration` trait (synchronous)
- Lifeguard migration runner
- Uses `LifeExecutor`
- CLI: `lifeguard migrate`

**Impact:**
- **Medium** - New system, but can borrow SeaORM migration patterns
- **Benefits:** Coroutine-native, no SeaORM dependency, clean API

---

### 6. Caching & LifeReflector

**Current:**
- No caching layer
- No distributed coherence

**Replacement:**
- Redis integration for read-through/write-through caching
- `LifeReflector` microservice (leader-elected Raft)
- TTL-based active set management
- Postgres LISTEN/NOTIFY integration

**Impact:**
- **New Feature** - Completely new capability
- **Benefits:** Oracle Coherence-level functionality, zero-stale reads, cluster-wide consistency

---

### 7. Replica Support

**Current:**
- No replica support
- No read routing

**Replacement:**
- WAL lag monitoring (`pg_current_wal_lsn` vs `pg_last_wal_replay_lsn`)
- Dynamic replica health checks
- Read preference modes (primary/replica/mixed/strong)
- Automatic fallback to primary on lag

**Impact:**
- **New Feature** - Advanced capability
- **Benefits:** Offload read traffic, reduce primary load, automatic health monitoring

---

## Migration Strategy

### Phase 1: Foundation (Weeks 1-3)
1. Remove SeaORM/Tokio dependencies
2. Integrate `may_postgres`
3. Implement `LifeExecutor` trait
4. Build `LifeguardPool` with persistent connections
5. Basic metrics

### Phase 2: ORM Core (Weeks 3-6)
6. Implement `LifeModel` derive macro
7. Implement `LifeRecord` derive macro
8. Basic CRUD operations
9. SeaQuery integration

### Phase 3: Advanced Features (Weeks 6-10)
10. `LifeMigration` system
11. Redis caching integration
12. Replica support with WAL monitoring
13. LifeReflector microservice (separate crate)

---

## Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| Breaking changes to existing code | High | Provide migration guide, maintain compatibility layer during transition |
| `may_postgres` maturity | Medium | Evaluate thoroughly, have fallback plan |
| Macro complexity | Medium | Start simple, iterate, comprehensive testing |
| Performance regression | Low | Benchmark continuously, validate improvements |
| Missing features | Medium | Prioritize by usage, implement incrementally |

---

## Success Metrics

- ✅ Zero async runtime dependencies in core
- ✅ 2-5× performance improvement on hot paths
- ✅ Zero SeaORM types in public API
- ✅ Persistent connection pool operational
- ✅ LifeModel/LifeRecord working
- ✅ LifeReflector providing cache coherence
- ✅ Replica routing with health monitoring

---

## Conclusion

This audit identifies **17 major components** that must be stripped out and replaced, plus **3 new features** (caching, replicas, LifeReflector) that must be built. The migration is substantial but necessary to achieve the coroutine-native architecture described in the PRD.

The benefits are significant:
- **Performance:** 2-5× faster, 10×+ on small queries
- **Architecture:** Clean, coroutine-native, no async overhead
- **Features:** Distributed cache coherence, replica support, advanced Postgres features
- **Developer Experience:** Simpler API, no async/await, clear semantics
