# Dependency Chain Analysis & Epic Restructuring

**Version:** 1.0  
**Date:** 2025-01-XX  
**Purpose:** Analyze bottom-up dependency chain and restructure EPICS accordingly

---

## Bottom-Up Dependency Chain

The implementation must be built in this exact order:

```
Layer 6: BRRTRouter
    ↓ depends on
Layer 5: LifeguardPool
    ↓ depends on
Layer 4: LifeExecutor
    ↓ depends on
Layer 3: LifeModel + LifeRecord (ORM Layer)
    ↓ depends on
Layer 2: LifeQuery / SeaQuery (SQL Builder)
    ↓ depends on
Layer 1: may_postgres
    ↓ connects to
Foundation: PostgreSQL
```

### Detailed Dependency Analysis

#### Layer 1: may_postgres → PostgreSQL
**Foundation Layer - No Dependencies**

- Direct connection to PostgreSQL
- Coroutine-native Postgres client
- Synchronous API (no async)
- Provides: `Client`, `Transaction`, `Row`, `Error`

**What to Build:**
- Integration tests with `may_postgres`
- Connection establishment
- Basic query execution
- Error handling
- Row parsing

**Dependencies:** None (external crate `may_postgres`)

---

#### Layer 2: LifeQuery / SeaQuery → may_postgres
**SQL Builder Layer - Depends on Layer 1**

- Wraps SeaQuery for SQL generation
- Converts SeaQuery statements to SQL strings
- Executes via `may_postgres::Client`
- Provides: SQL building, parameter binding

**What to Build:**
- SeaQuery integration wrapper
- SQL string generation (PostgresQueryBuilder)
- Parameter binding for `may_postgres`
- Type-safe query construction
- Basic SELECT, INSERT, UPDATE, DELETE

**Dependencies:** 
- ✅ Layer 1 (may_postgres) - MUST be complete
- SeaQuery crate (external dependency)

**Cannot Start Until:** may_postgres integration is working

---

#### Layer 3: LifeModel + LifeRecord → LifeQuery
**ORM Layer - Depends on Layer 2**

- Uses LifeQuery to build SQL
- Generates structs from database rows
- Provides type-safe CRUD operations
- Procedural macros for code generation

**What to Build:**
- `#[derive(LifeModel)]` macro
- `#[derive(LifeRecord)]` macro
- Row-to-struct mapping
- Query builder methods (`find()`, `find_by_id()`, etc.)
- Insert/update/delete operations
- Type-safe column references

**Dependencies:**
- ✅ Layer 2 (LifeQuery) - MUST be complete
- ✅ Layer 1 (may_postgres) - MUST be complete

**Cannot Start Until:** LifeQuery can generate and execute SQL

---

#### Layer 4: LifeExecutor → ORM Layer
**Execution Abstraction - Depends on Layer 3**

- Abstraction over `may_postgres` for ORM and migrations
- Provides unified execution interface
- Handles connection management
- Error translation

**What to Build:**
- `LifeExecutor` trait definition
- Implementation for `may_postgres::Client`
- Error type (`LifeError`)
- Transaction support
- Connection abstraction

**Dependencies:**
- ✅ Layer 3 (ORM Layer) - MUST be complete to test
- ✅ Layer 2 (LifeQuery) - MUST be complete
- ✅ Layer 1 (may_postgres) - MUST be complete

**Note:** LifeExecutor can be **designed** early, but **fully implemented** requires ORM layer to test against.

---

#### Layer 5: LifeguardPool → LifeExecutor
**Connection Pool - Depends on Layer 4**

- Manages persistent connection slots
- Semaphore-based acquisition
- Health monitoring
- Replica support (later)

**What to Build:**
- Persistent connection slot management
- Semaphore-based acquisition
- Connection health monitoring
- Auto-reconnection logic
- Pool metrics
- Replica connection management (v2)

**Dependencies:**
- ✅ Layer 4 (LifeExecutor) - MUST be complete
- ✅ Layer 3 (ORM Layer) - For testing
- ✅ Layer 2 (LifeQuery) - For testing
- ✅ Layer 1 (may_postgres) - Foundation

**Cannot Start Until:** LifeExecutor is working

---

#### Layer 6: BRRTRouter → LifeguardPool / LifeExecutor
**Integration Layer - Depends on Layer 5**

- Integration with BRRTRouter framework
- Demonstrates real-world usage
- Performance benchmarks

**What to Build:**
- BRRTRouter integration examples
- Performance benchmarks
- Real-world usage patterns

**Dependencies:**
- ✅ Layer 5 (LifeguardPool) - MUST be complete
- ✅ All previous layers

---

## Current Epic Structure Issues

### Problem 1: LifeExecutor Before ORM Layer

**Current Epic 01** tries to build:
- LifeExecutor ✅ (correct level)
- LifeguardPool ❌ (too early - needs LifeExecutor + ORM for testing)

**Issue:** LifeguardPool needs LifeExecutor, but LifeExecutor needs ORM layer to be fully testable.

### Problem 2: ORM Layer Before SQL Builder

**Current Epic 02** tries to build:
- LifeModel/LifeRecord ✅ (correct)
- SeaQuery integration ❌ (should be in separate epic before ORM)

**Issue:** LifeModel/LifeRecord depend on LifeQuery, but LifeQuery isn't built first.

### Problem 3: Missing LifeQuery Epic

**Current Structure:** No dedicated epic for LifeQuery/SeaQuery integration.

**Issue:** LifeQuery is a critical layer that must be built before ORM.

---

## Corrected Epic Structure

### Epic 01: may_postgres Foundation
**Layer 1 - Foundation**

**Goal:** Integrate and validate `may_postgres` as the database client.

**Stories:**
1. Remove SeaORM/Tokio dependencies
2. Integrate `may_postgres` crate
3. Basic connection establishment
4. Execute raw SQL queries
5. Row parsing and type conversion
6. Error handling and translation
7. Transaction support (basic)
8. Connection health checks

**Dependencies:** None (external `may_postgres` crate)

**Timeline:** Weeks 1-2

---

### Epic 02: LifeQuery / SeaQuery Integration
**Layer 2 - SQL Builder**

**Goal:** Build SQL builder layer using SeaQuery, executing via `may_postgres`.

**Stories:**
1. SeaQuery crate integration
2. PostgresQueryBuilder wrapper
3. SQL string generation (SELECT, INSERT, UPDATE, DELETE)
4. Parameter binding for `may_postgres`
5. Type-safe query construction
6. Filter operations (eq, ne, gt, etc.)
7. Join operations (inner, left, right)
8. Aggregates (COUNT, SUM, AVG)
9. Ordering and grouping
10. Subqueries and CTEs

**Dependencies:**
- ✅ Epic 01 (may_postgres) - MUST be complete

**Timeline:** Weeks 2-3

---

### Epic 03: LifeModel & LifeRecord (ORM Core)
**Layer 3 - ORM Layer**

**Goal:** Build ORM layer with LifeModel and LifeRecord derive macros.

**Stories:**
1. `#[derive(LifeModel)]` macro - basic structure
2. `#[derive(LifeModel)]` macro - field mapping and types
3. `#[derive(LifeModel)]` macro - column metadata
4. `#[derive(LifeRecord)]` macro - insert operations
5. `#[derive(LifeRecord)]` macro - update operations
6. `LifeModel::find()` query builder
7. `LifeModel::find_by_id()` method
8. `LifeModel::find_one()` method
9. `LifeRecord::insert()` method
10. `LifeRecord::update()` method
11. `LifeRecord::delete()` method
12. Batch operations (insert_many, update_many)
13. Upsert support
14. Pagination helpers
15. Entity hooks & lifecycle events
16. Validators
17. Soft deletes
18. Auto-managed timestamps

**Dependencies:**
- ✅ Epic 02 (LifeQuery) - MUST be complete
- ✅ Epic 01 (may_postgres) - MUST be complete

**Timeline:** Weeks 3-6

---

### Epic 04: LifeExecutor & LifeguardPool
**Layer 4 & 5 - Execution & Pooling**

**Goal:** Build execution abstraction and connection pool.

**Stories:**
1. `LifeExecutor` trait definition
2. `LifeExecutor` implementation for `may_postgres::Client`
3. `LifeError` error type
4. Transaction support via LifeExecutor
5. Raw SQL helpers via LifeExecutor
6. LifeguardPool - persistent connection slots
7. LifeguardPool - semaphore-based acquisition
8. LifeguardPool - health monitoring
9. LifeguardPool - auto-reconnection
10. LifeguardPool - metrics integration
11. LifeguardPool - configuration system
12. Integration testing with ORM layer

**Dependencies:**
- ✅ Epic 03 (ORM Layer) - For testing LifeExecutor
- ✅ Epic 02 (LifeQuery) - For testing
- ✅ Epic 01 (may_postgres) - Foundation

**Timeline:** Weeks 6-8

**Note:** LifeExecutor can be **designed** in parallel with Epic 03, but **full implementation** requires ORM layer.

---

### Epic 05: LifeMigration System
**Layer 4 - Migrations (Uses LifeExecutor)**

**Goal:** Build migration system using LifeExecutor.

**Stories:**
1. `LifeMigration` trait definition
2. Migration runner implementation
3. Migration history tracking
4. CLI tooling (`lifeguard migrate`)
5. Core PostgreSQL schema operations
6. Programmatic migrations
7. Data seeding in migrations
8. Advanced migration operations

**Dependencies:**
- ✅ Epic 04 (LifeExecutor) - MUST be complete
- ✅ Epic 01 (may_postgres) - Foundation

**Timeline:** Weeks 8-9

**Note:** Can start in parallel with Epic 04 once LifeExecutor is available.

---

### Epic 06: v1 Release & BRRTRouter Integration
**Layer 6 - Integration & Release**

**Goal:** Complete v1 feature set, integrate with BRRTRouter, release.

**Stories:**
1. Complete PostgreSQL feature support (v1)
2. Testkit infrastructure
3. Comprehensive documentation
4. BRRTRouter integration
5. Performance benchmarks
6. v1.0.0 release preparation

**Dependencies:**
- ✅ Epic 04 (LifeguardPool) - MUST be complete
- ✅ Epic 05 (Migrations) - MUST be complete
- ✅ Epic 03 (ORM) - MUST be complete

**Timeline:** Weeks 9-10

---

### Epic 07: Advanced Features (LifeReflector, Redis, Replicas)
**Layer 5+ - Advanced Capabilities**

**Goal:** Build distributed cache coherence, Redis integration, replica support.

**Stories:**
1. Redis integration - read-through cache
2. Redis integration - write-through cache
3. LifeReflector - leader-elected Raft system
4. LifeReflector - Postgres LISTEN/NOTIFY integration
5. LifeReflector - Redis cache refresh logic
6. LifeReflector - TTL-based active set management
7. Replica support - WAL lag monitoring
8. Replica support - dynamic routing
9. Replica support - read preferences
10. Replica support - strong consistency mode
11. Complete relation support
12. Materialized views

**Dependencies:**
- ✅ Epic 06 (v1 Release) - MUST be complete
- Redis (external service)
- PostgreSQL replication setup

**Timeline:** Weeks 10-14

---

### Epic 08: Enterprise Features
**Advanced Postgres Features**

**Goal:** PostGIS, partitioning, triggers, codegen.

**Stories:**
1. PostGIS support
2. Partitioning (RANGE, HASH, LIST)
3. Triggers and stored procedures
4. Schema introspection tools
5. Code generation from database
6. Model inspector CLI

**Dependencies:**
- ✅ Epic 07 (Advanced Features) - MUST be complete

**Timeline:** Weeks 15-20

---

## Key Insights

### 1. LifeQuery Must Come Before ORM

**Current Problem:** Epic 02 tries to build ORM before SQL builder is ready.

**Solution:** Epic 02 should be dedicated to LifeQuery/SeaQuery integration. Epic 03 builds ORM on top.

### 2. LifeExecutor Design vs. Implementation

**Insight:** LifeExecutor trait can be **designed** early (even in Epic 01), but **full implementation** requires ORM layer for testing.

**Solution:** Design LifeExecutor in Epic 01, implement fully in Epic 04.

### 3. LifeguardPool Needs Complete Stack

**Insight:** LifeguardPool needs LifeExecutor + ORM layer to be properly tested.

**Solution:** Build LifeguardPool in Epic 04 after LifeExecutor is complete.

### 4. Migrations Can Be Parallel

**Insight:** LifeMigration only needs LifeExecutor, not the full ORM.

**Solution:** Epic 05 can start once LifeExecutor is available (parallel with Epic 04 completion).

---

## Epic Dependency Graph

```
Epic 01: may_postgres Foundation
    ↓
Epic 02: LifeQuery Integration
    ↓
Epic 03: LifeModel & LifeRecord (ORM)
    ↓
Epic 04: LifeExecutor & LifeguardPool ──┐
    ↓                                    │
Epic 05: LifeMigration ─────────────────┘ (parallel once LifeExecutor ready)
    ↓
Epic 06: v1 Release & BRRTRouter
    ↓
Epic 07: Advanced Features (LifeReflector, Redis, Replicas)
    ↓
Epic 08: Enterprise Features
```

---

## Summary

### Critical Changes Needed

1. **Split Epic 01:** Separate may_postgres integration from LifeExecutor/LifeguardPool
2. **Create Epic 02:** Dedicated LifeQuery/SeaQuery integration epic
3. **Restructure Epic 03:** Focus solely on LifeModel/LifeRecord (depends on Epic 02)
4. **Create Epic 04:** LifeExecutor + LifeguardPool (depends on Epic 03)
5. **Restructure Epic 05:** LifeMigration (depends on Epic 04's LifeExecutor)
6. **Rename Epics:** Epic 04 → Epic 06, Epic 05 → Epic 07, Epic 06 → Epic 08

### Build Order Validation

✅ **Correct Order:**
1. may_postgres (Epic 01)
2. LifeQuery (Epic 02)
3. LifeModel/LifeRecord (Epic 03)
4. LifeExecutor (Epic 04)
5. LifeguardPool (Epic 04)
6. LifeMigration (Epic 05)
7. BRRTRouter integration (Epic 06)
8. Advanced features (Epic 07)
9. Enterprise features (Epic 08)

This ensures each layer can be built and tested before the next layer depends on it.
