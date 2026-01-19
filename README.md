<p align="center">
  <img src="/docs/images/Lifeguard2.png" alt="Lifeguard logo" />
</p>

# 🛟 Lifeguard: Coroutine-Driven Database Runtime for Rust

**Lifeguard** is a **complete, production-grade ORM and data access platform** built from the ground up for Rust's `may` coroutine runtime. This is not a wrapper or adapter—it's a **parallel universe ORM** that provides SeaORM-like functionality but is architected natively for coroutines.

---

## 🔥 Why Lifeguard?

**Lifeguard** is a **complete, production-grade ORM and data access platform** built from the ground up for Rust's `may` coroutine runtime. This is not a wrapper or adapter—it's a **parallel universe ORM** that provides SeaORM-like functionality but is architected natively for coroutines.

**The Problem:** Existing Rust ORMs (SeaORM, Diesel, SQLx) are built for async/await and Tokio. The `may` coroutine runtime uses stackful coroutines, not async futures. These are **fundamentally incompatible architectures**—you cannot bridge them without significant performance penalties.

**The Solution:** Build a complete ORM from scratch using `may_postgres` (coroutine-native PostgreSQL client). No async runtime. No Tokio. Pure coroutine I/O.

**Why This Matters:**
- **BRRTRouter** (the coroutine API framework) needs blistering fast database access for high-throughput applications
- High-performance microservices need predictable, low-latency database access without async overhead
- Applications with extreme scale requirements (millions of requests/second) need efficient connection pooling when database connections are limited
- Coroutines offer deterministic scheduling, lower memory overhead, and predictable latency
- But without a proper ORM, developers are forced to choose: async ORM (overhead) or raw SQL (no safety)

**Lifeguard solves this** by providing a complete data platform that matches SeaORM's feature set but is built for coroutines, plus **distributed cache coherence** (LifeReflector) that no other ORM provides.

---

## 🚀 What We're Building

### Core ORM: LifeModel & LifeRecord

A complete ORM system with two primary abstractions:

**LifeModel** (Immutable Database Rows)
- Represents database rows as immutable Rust structs
- Generated via `#[derive(LifeModel)]` procedural macro
- Provides type-safe query builders
- Automatic row-to-struct mapping
- Complete SeaORM API parity

**LifeRecord** (Mutable Change Sets)
- Separate abstraction for inserts and updates
- Generated via `#[derive(LifeRecord)]` procedural macro
- Type-safe mutation builders
- Automatic SQL generation via SeaQuery
- Change tracking (dirty fields)

```rust
#[derive(LifeModel)]
#[table = "users"]
struct User {
    #[primary_key]
    id: i64,
    email: String,
    is_active: bool,
}

#[derive(LifeRecord)]
struct NewUser {
    email: String,
}

// Usage - no async/await needed!
let user = NewUser { email: "test@example.com".into() }
    .insert(&pool)?;

let found = User::find_by_id(&pool, user.id)?;
let users = User::find()
    .filter(User::Email.eq("test@example.com"))
    .all(&pool)?;
```

### Connection Pool: LifeguardPool

A sophisticated connection pool designed for extreme scale:

- **Persistent connections:** Pre-allocated, long-lived Postgres connections (no on-demand creation)
- **Semaphore-based concurrency:** Bounded acquisition prevents connection storms
- **Health monitoring:** Automatic detection and reconnection of failed connections
- **Aggressive reuse:** Every connection handles thousands of requests per second
- **Coroutine-native:** No async runtime, pure coroutine I/O

**For High-Scale Applications:** When database connections are limited (e.g., 100-500 connections) but traffic is extreme (millions of requests per second), connection pooling becomes critical. LifeguardPool makes a small number of connections (the 300 Spartans) handle massive traffic (the Persian Empire) through aggressive connection reuse and intelligent pooling tactics.

### The Killer Feature: LifeReflector

**Distributed cache coherence system**—this is Lifeguard's unique advantage:

A **standalone microservice** that maintains cluster-wide cache coherence:

- **Leader-elected Raft system:** Only one active reflector at a time (no duplicate work)
- **Postgres LISTEN/NOTIFY integration:** Subscribes to database change events
- **Intelligent cache refresh:** Only updates keys that exist in Redis (TTL-based active set)
- **Zero-stale reads:** Redis always reflects current database state
- **Horizontal scaling:** All microservices benefit from single reflector

**How It Works:**
1. LifeRecord writes to Postgres → triggers `NOTIFY`
2. LifeReflector (leader) receives notification
3. Checks if key exists in Redis (active item)
4. If exists → refreshes from database → updates Redis
5. If not → ignores (inactive item, TTL expired)
6. All microservices read from Redis → always fresh data

**Result:** Oracle Coherence-level cache consistency with Postgres + Redis, but lighter, faster, and open source.

### Transparent Caching System

Lifeguard provides transparent caching that uses PostgreSQL as the gods designed it:

- **Check Redis first:** Sub-millisecond reads if cached
- **Read from replicas:** When healthy (WAL lag < threshold)
- **Write to primary:** Always (as PostgreSQL was designed)
- **LifeReflector keeps cache fresh:** Automatic coherence across microservices

Your application code doesn't need to know about Redis, replicas, or cache coherence. It just calls `User::find_by_id(&pool, 42)?` and Lifeguard handles the rest.

### Replica Read Support

Advanced read routing with WAL lag awareness:

- **WAL position tracking:** Monitors `pg_current_wal_lsn()` vs `pg_last_wal_replay_lsn()`
- **Dynamic health checks:** Automatically detects replica lag
- **Intelligent routing:** Routes reads to replicas only when healthy
- **Automatic fallback:** Falls back to primary if replicas are stale
- **Strong consistency mode:** Optional causal read-your-writes consistency

**Read Preference Modes:**
- `primary` - Always read from primary
- `replica` - Use replicas when healthy
- `mixed` - Automatic selection (Redis → replica → primary)
- `strong` - Causal consistency (wait for replica to catch up)

### Complete Feature Set

**ORM Features (SeaORM Parity):**
- ✅ Complete CRUD operations
- ✅ Type-safe query builders
- ✅ Relations (has_one, has_many, belongs_to, many_to_many)
- ✅ Migrations (programmatic, data seeding, advanced operations)
- ✅ Transactions
- ✅ Raw SQL helpers
- ✅ Batch operations
- ✅ Upsert support
- ✅ Pagination helpers
- ✅ Entity hooks & lifecycle events
- ✅ Validators
- ✅ Soft deletes
- ✅ Auto-managed timestamps

**Competitive Features:**
- ✅ Schema inference (Diesel `table!` macro equivalent)
- ✅ Session/Unit of Work pattern (SQLAlchemy)
- ✅ Scopes (ActiveRecord)
- ✅ Model Managers (Django)
- ✅ F() Expressions (Django)
- ✅ Advanced eager loading strategies (SQLAlchemy)

**Unique Features (No Other ORM Has):**
- ✅ **LifeReflector** - Distributed cache coherence
- ✅ **Coroutine-native** - No async overhead
- ✅ **WAL-based replica routing** - Automatic health monitoring
- ✅ **TTL-based active set** - Adaptive caching

---

## 🏗️ Architecture Overview

### New Architecture (Post-Rebuild)

```mermaid
graph TD
    App[Application Code] --> Pool[LifeguardPool]
    Pool --> Executor[LifeExecutor]
    Executor --> may_postgres[may_postgres]
    may_postgres --> PostgreSQL[PostgreSQL]
    
    App --> LifeModel[LifeModel / LifeRecord]
    LifeModel --> SeaQuery[SeaQuery SQL Builder]
    SeaQuery --> Executor
    
    App --> Redis[Redis Cache]
    Redis --> LifeReflector[LifeReflector Service]
    PostgreSQL -- NOTIFY --> LifeReflector
    LifeReflector --> Redis
    
    style App fill:#add8e6,stroke:#333,stroke-width:2px
    style Pool fill:#90ee90,stroke:#333,stroke-width:2px
    style Executor fill:#90ee90,stroke:#333,stroke-width:2px
    style LifeModel fill:#90ee90,stroke:#333,stroke-width:2px
    style SeaQuery fill:#90ee90,stroke:#333,stroke-width:2px
    style may_postgres fill:#90ee90,stroke:#333,stroke-width:2px
    style PostgreSQL fill:#c0c0c0,stroke:#333,stroke-width:2px
    style Redis fill:#ffcccb,stroke:#333,stroke-width:2px
    style LifeReflector fill:#add8e6,stroke:#333,stroke-width:2px
```

**Key Components:**
- **LifeguardPool**: Persistent connection pool with semaphore-based acquisition
- **LifeExecutor**: Database execution abstraction over `may_postgres`
- **LifeModel/LifeRecord**: Complete ORM layer (replaces SeaORM)
- **SeaQuery**: SQL building (borrowed, compatible with coroutines)
- **may_postgres**: Coroutine-native PostgreSQL client (foundation)
- **LifeReflector**: Distributed cache coherence microservice
- **Redis**: Transparent caching layer

```mermaid
graph TD
    subgraph Frontend["Frontend / Clients"]
        Web[Web App]
        Mobile[Mobile App]
        API[API Clients]
    end
    
    subgraph BFF["BFF Layer<br/>Built with BRRTRouter"]
        BFF_Service[Backend for Frontend<br/>API Gateway / Router]
    end
    
    subgraph Backend["Backend Microservices<br/>Your Business Logic"]
        MS1[User Service]
        MS2[Product Service]
        MS3[Order Service]
        MSN[Service N<br/>Your Domain]
    end
    
    subgraph Lifeguard
        Pool[LifeguardPool]
        Executor[LifeExecutor]
        LifeModel[LifeModel / LifeRecord]
        SeaQuery[SeaQuery]
    end
    
    subgraph Data Layer
        may_postgres[may_postgres]
        PostgreSQL[(PostgreSQL)]
        Redis[(Redis Cache)]
    end
    
    subgraph LifeReflector
        Reflector[LifeReflector Leader]
    end
    
    Web --> BFF_Service
    Mobile --> BFF_Service
    API --> BFF_Service
    
    BFF_Service --> MS1
    BFF_Service --> MS2
    BFF_Service --> MS3
    BFF_Service --> MSN
    
    MS1 --> Pool
    MS2 --> Pool
    MS3 --> Pool
    MSN --> Pool
    
    Pool --> Executor
    Executor --> LifeModel
    LifeModel --> SeaQuery
    SeaQuery --> Executor
    Executor --> may_postgres
    may_postgres --> PostgreSQL
    
    MS1 --> Redis
    MS2 --> Redis
    MS3 --> Redis
    MSN --> Redis
    PostgreSQL -- NOTIFY --> Reflector
    Reflector --> Redis
    
    style Frontend fill:#e1f5ff
    style Web fill:#e1f5ff
    style Mobile fill:#e1f5ff
    style API fill:#e1f5ff
    style BFF fill:#add8e6
    style BFF_Service fill:#add8e6
    style Backend fill:#d4edda
    style MS1 fill:#d4edda
    style MS2 fill:#d4edda
    style MS3 fill:#d4edda
    style MSN fill:#d4edda
    style Pool fill:#90ee90
    style Executor fill:#90ee90
    style LifeModel fill:#90ee90
    style may_postgres fill:#90ee90
    style PostgreSQL fill:#c0c0c0
    style Redis fill:#ffcccb
    style Reflector fill:#add8e6
```

### Connection Pool Architecture

```mermaid
graph TD
    subgraph LifeguardPool["LifeguardPool<br/>The 300 Spartans"]
        S[Semaphore<br/>max_connections tokens<br/>100-500 limit]
        subgraph Slots["Connection Slots<br/>Persistent & Reused"]
            C1[Slot 1<br/>in_use: false<br/>ready]
            C2[Slot 2<br/>in_use: true<br/>executing query]
            C3[Slot 3<br/>in_use: false<br/>ready]
            CN[Slot N<br/>in_use: false<br/>ready]
        end
    end
    
    subgraph Traffic["Incoming Traffic<br/>The Persian Empire"]
        R1[Request 1]
        R2[Request 2]
        R3[Request 3]
        RN[Request N<br/>millions/sec]
    end
    
    Traffic -->|acquire| S
    S -->|find free| Slots
    Slots -->|mark in_use| C2
    C2 -->|execute query| PG[PostgreSQL<br/>The Pass]
    PG -->|result| C2
    C2 -->|release| S
    C2 -->|mark free| Slots
    C2 -->|ready for| Traffic
    
    style LifeguardPool fill:#fff4e1
    style S fill:#fff4e1
    style Slots fill:#e1ffe1
    style PG fill:#e1ffe1
    style Traffic fill:#ffe1e1
    
    Note[100 connections<br/>handle millions of requests<br/>through aggressive reuse]
    LifeguardPool --> Note
```

### LifeReflector Cache Coherence

```mermaid
sequenceDiagram
    participant LifeRecord
    participant Postgres
    participant LifeReflector
    participant Redis
    participant LifeModel

    LifeRecord->>Postgres: Write (INSERT/UPDATE/DELETE)
    Postgres->>Postgres: Commit Transaction
    Postgres->>LifeReflector: NOTIFY table_changes, '{"id": 42}'
    LifeReflector->>Redis: EXISTS lifeguard:model:table:42?
    alt Key Exists (Active Item)
        LifeReflector->>Postgres: SELECT * FROM table WHERE id = 42 (from Primary)
        Postgres-->>LifeReflector: Fresh Data
        LifeReflector->>Redis: SETEX lifeguard:model:table:42 <TTL> <Serialized Data>
    else Key Not Exists (Inactive)
        LifeReflector->>LifeReflector: Ignore (item not cached, TTL expired)
    end
    LifeModel->>Redis: Read (GET lifeguard:model:table:42)
    alt Cache Hit
        Redis-->>LifeModel: Cached Data (Fresh)
    else Cache Miss
        LifeModel->>Postgres: Read (SELECT * FROM table WHERE id = 42)
        Postgres-->>LifeModel: Data
        LifeModel->>Redis: SETEX lifeguard:model:table:42 <TTL> <Serialized Data>
    end
```





---

## 💻 Getting Started

**Note:** Lifeguard is currently being rebuilt from scratch. The following examples show the target API (not yet implemented).

### Installation

```toml
[dependencies]
lifeguard = { git = "https://github.com/microscaler/lifeguard" }
```

### Basic Usage

```rust
use lifeguard::*;

// Create connection pool
let pool = LifeguardPool::new("postgresql://user:pass@localhost/dbname")?;

// Define model
#[derive(LifeModel)]
#[table = "users"]
struct User {
    #[primary_key]
    id: i64,
    email: String,
    is_active: bool,
}

// Query (no async/await needed!)
let user = User::find_by_id(&pool, 1)?;
let users = User::find()
    .filter(User::Email.eq("test@example.com"))
    .all(&pool)?;

// Insert
#[derive(LifeRecord)]
struct NewUser {
    email: String,
}

let user = NewUser { email: "test@example.com".into() }
    .insert(&pool)?;
```

### With Caching (Transparent)

```rust
// Caching is transparent - no code changes needed!
// Lifeguard automatically:
// 1. Checks Redis first
// 2. Falls back to database if cache miss
// 3. Caches result for future reads
// 4. LifeReflector keeps cache fresh

let user = User::find_by_id(&pool, 1)?; // May come from Redis or DB
```

### With Replicas (Automatic Health Monitoring)

```rust
// Configure read preferences
let pool = LifeguardPool::new(url)?
    .with_read_preference(ReadPreference::Mixed); // Redis → replica → primary

// Lifeguard automatically:
// 1. Checks Redis first
// 2. Routes to healthy replicas
// 3. Falls back to primary if replicas lagged
// 4. Monitors WAL lag continuously

let users = User::find().all(&pool)?; // Automatically routed
```


---

## 📊 Observability

Comprehensive instrumentation for production operations:

### Prometheus Metrics

- `lifeguard_pool_size` - Current pool size
- `lifeguard_active_connections` - Active connections
- `lifeguard_connection_wait_time` - Time waiting for connection
- `lifeguard_query_duration_seconds` - Query execution time
- `lifeguard_query_errors_total` - Query errors
- `lifeguard_cache_hits_total` - Cache hits
- `lifeguard_cache_misses_total` - Cache misses
- `lifeguard_replica_lag_bytes` - Replica lag (bytes)
- `lifeguard_replica_lag_seconds` - Replica lag (seconds)
- `lifeguard_replicas_healthy` - Number of healthy replicas

### OpenTelemetry Tracing

- Distributed tracing for database operations
- Spans for: connection acquisition, query execution, cache operations
- Integration with existing OpenTelemetry infrastructure

### LifeReflector Metrics

- `reflector_notifications_total` - Notifications received
- `reflector_refreshes_total` - Cache refreshes
- `reflector_ignored_total` - Ignored notifications (inactive items)
- `reflector_active_keys` - Active cache keys
- `reflector_redis_latency_seconds` - Redis operation latency
- `reflector_pg_latency_seconds` - PostgreSQL operation latency
- `reflector_leader_changes_total` - Leader election events

---

## 🧪 Testing

Lifeguard includes a comprehensive testkit for testing database operations:

```rust
use lifeguard::testkit::*;

#[test]
fn test_user_operations() {
    let pool = test_pool!();
    
    let user = NewUser { email: "test@example.com".into() }
        .insert(&pool)?;
    
    assert_eq!(user.email, "test@example.com");
}
```

The testkit provides:
- Test database setup/teardown
- Transaction rollback after each test (clean state)
- Fixture loading helpers
- Test database isolation

---

## 🗺️ Roadmap

### Epic 01: Foundation (Weeks 1-3)
- ✅ Remove SeaORM and Tokio dependencies
- ✅ Integrate `may_postgres` as database client
- ✅ Implement `LifeExecutor` trait
- ✅ Redesign `LifeguardPool` for `may_postgres`
- ✅ Basic metrics and observability
- ✅ Transaction support
- ✅ Raw SQL helpers

### Epic 02: ORM Core (Weeks 3-6)
- ✅ Build `LifeModel` derive macro
- ✅ Build `LifeRecord` derive macro
- ✅ Implement basic CRUD operations
- ✅ Integrate SeaQuery for SQL building
- ✅ Type-safe query builders
- ✅ Batch operations
- ✅ Upsert support
- ✅ Pagination helpers
- ✅ Entity hooks & lifecycle events
- ✅ Validators
- ✅ Soft deletes
- ✅ Auto-managed timestamps
- ✅ Session/Unit of Work pattern
- ✅ Scopes
- ✅ Model Managers
- ✅ F() Expressions

### Epic 03: Migrations (Weeks 6-8)
- ✅ Implement `LifeMigration` trait
- ✅ Build migration runner
- ✅ Create CLI tooling (`lifeguard migrate`)
- ✅ Support core PostgreSQL features
- ✅ Programmatic migrations and data seeding
- ✅ Advanced migration operations

### Epic 04: v1 Release (Weeks 8-10)
- ✅ Complete PostgreSQL feature support
- ✅ Testkit infrastructure
- ✅ Comprehensive documentation
- ✅ Integration with BRRTRouter
- ✅ Performance benchmarks

### Epic 05: Advanced Features (Weeks 10-14)
- ✅ LifeReflector (distributed cache coherence)
- ✅ Redis integration
- ✅ Replica read support with WAL lag awareness
- ✅ Complete relation support
- ✅ Materialized views
- ✅ Query cache support
- ✅ Cache statistics & monitoring

### Epic 06: Enterprise Features (Weeks 15 - 20)
- ✅ PostGIS support
- ✅ Partitioning
- ✅ Triggers and stored procedures
- ✅ Schema introspection tools (Diesel `table!` equivalent)
- ✅ Code generation from database
- ✅ Schema-first design

See [EPICS](./docs/EPICS/) for detailed stories and progress tracking.

---

## 🎯 Competitive Metrics: Lifeguard vs Rust ORMs

*Implementation status based on current codebase analysis (see `lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md`)*

| Feature | Lifeguard Promise | Implementation Status | SeaORM | Diesel | SQLx |
|---------|-------------------|----------------------|--------|--------|------|
| **Concurrency Model** | ✅ Coroutine-native (`may`) | ✅ **Implemented** | ❌ Async/await (Tokio) | ❌ Sync-only | ❌ Async/await (Tokio) |
| **Performance (Hot Paths)** | ✅✅✅ 2-5× faster | 🟡 **Architectural** | ⚠️ Async overhead | ✅ Fast (sync) | ⚠️ Async overhead |
| **Performance (Small Queries)** | ✅✅✅ 10×+ faster | 🟡 **Architectural** | ⚠️ Future allocation | ✅ Fast | ⚠️ Future allocation |
| **Memory Footprint** | ✅✅ Low (stackful coroutines) | 🟡 **Architectural** | ⚠️ Higher (heap futures) | ✅ Low | ⚠️ Higher (heap futures) |
| **Predictable Latency** | ✅✅✅ Deterministic scheduling | 🟡 **Architectural** | ⚠️ Poll-based (variable) | ✅ Predictable | ⚠️ Poll-based (variable) |
| **Type Safety** | ✅✅✅ Compile-time validation | ✅ **Implemented** | ✅✅ Compile-time validation | ✅✅✅ Strong compile-time | ✅✅ Compile-time SQL checks |
| **ORM Features** | ✅✅✅ Complete (SeaORM parity) | 🟡 **67% Complete** (Core traits, relations, query builder) | ✅✅✅ Complete | ✅✅ Good | ❌ Query builder only |
| **CRUD Operations** | ✅✅✅ Full support | ✅ **Implemented** (insert/update/save/delete via ActiveModelTrait) | ✅✅✅ Full support | ✅✅ Full support | ⚠️ Manual SQL |
| **Relations** | ✅✅✅ All types (has_one, has_many, belongs_to, many_to_many) | ✅ **Implemented** (Complete with eager/lazy loading, composite keys, DeriveLinked) | ✅✅✅ All types | ✅✅ Basic support | ❌ Manual joins |
| **Migrations** | ✅✅✅ Programmatic, data seeding, advanced ops | 🟡 **Partial** (DeriveMigrationName missing, infrastructure may exist) | ✅✅✅ Programmatic | ✅✅ CLI-based | ⚠️ Manual SQL |
| **Schema Inference** | ✅✅✅ From database (Diesel equivalent) | ❌ **Not Implemented** | ✅✅ From database | ✅✅✅ `table!` macro | ❌ No |
| **Query Builder** | ✅✅✅ Type-safe, chainable | ✅ **Implemented** (19/20 methods, 95% coverage) | ✅✅✅ Type-safe, chainable | ✅✅✅ Compile-time checked | ✅✅ Compile-time SQL |
| **Transactions** | ✅✅✅ Full support | ✅ **Implemented** (Roadmap Epic 01) | ✅✅✅ Full support | ✅✅ Full support | ✅✅ Full support |
| **Batch Operations** | ✅✅✅ insert_many, update_many, delete_many | ✅ **Implemented** | ✅✅✅ Batch support | ✅✅ Batch support | ⚠️ Manual |
| **Upsert** | ✅✅✅ save(), on_conflict() | ✅ **Implemented** (save() method exists) | ✅✅✅ save(), on_conflict() | ✅✅ on_conflict() | ⚠️ Manual SQL |
| **Pagination** | ✅✅✅ paginate(), paginate_and_count() | ✅ **Implemented** | ✅✅✅ Pagination helpers | ⚠️ Manual | ⚠️ Manual |
| **Entity Hooks** | ✅✅✅ before/after lifecycle events | ✅ **Implemented** (ActiveModelBehavior with 8 lifecycle hooks) | ✅✅✅ Hooks support | ❌ No | ❌ No |
| **Validators** | ✅✅✅ Field & model-level | ❌ **Not Implemented** | ⚠️ Limited | ❌ No | ❌ No |
| **Soft Deletes** | ✅✅✅ Built-in support | ❌ **Not Implemented** | ⚠️ Manual | ❌ No | ❌ No |
| **Auto Timestamps** | ✅✅✅ created_at, updated_at | ❌ **Not Implemented** | ⚠️ Manual | ❌ No | ❌ No |
| **Session/Unit of Work** | ✅✅✅ Identity map, dirty tracking | ❌ **Not Implemented** | ❌ No | ❌ No | ❌ No |
| **Scopes** | ✅✅✅ Named query scopes | ❌ **Not Implemented** | ❌ No | ❌ No | ❌ No |
| **Model Managers** | ✅✅✅ Custom query methods | ❌ **Not Implemented** | ❌ No | ❌ No | ❌ No |
| **F() Expressions** | ✅✅✅ Database-level expressions | ❌ **Not Implemented** | ❌ No | ⚠️ Limited | ❌ No |
| **Subqueries** | ✅✅✅ Full support | 🟡 **Future** (Not yet implemented) | ✅✅✅ Full support | ✅✅ Full support | ✅✅ Manual SQL |
| **CTEs** | ✅✅✅ WITH clauses | 🟡 **Future** (Not yet implemented) | ✅✅✅ WITH clauses | ✅✅ WITH clauses | ✅✅ Manual SQL |
| **Window Functions** | ✅✅✅ Full support | 🟡 **Future** (Not yet implemented) | ✅✅✅ Full support | ✅✅ Full support | ✅✅ Manual SQL |
| **Eager Loading** | ✅✅✅ Multiple strategies (joinedload, subqueryload, selectinload) | ✅ **Implemented** (selectinload strategy with FK extraction) | ✅✅✅ Eager loading | ⚠️ Manual | ❌ Manual |
| **Raw SQL** | ✅✅✅ find_by_statement(), execute_unprepared() | ✅ **Implemented** (Architecture supports raw SQL) | ✅✅✅ Raw SQL support | ✅✅✅ Raw SQL support | ✅✅✅ Primary feature |
| **Connection Pooling** | ✅✅✅ Persistent, semaphore-based, health monitoring | ✅ **Implemented** (LifeguardPool architecture) | ✅✅✅ Built-in pool | ⚠️ External (r2d2) | ✅✅✅ Built-in pool |
| **Replica Read Support** | ✅✅✅ WAL-based health monitoring, automatic routing | 🟡 **Architectural** (Not in SeaORM mapping, may exist) | ❌ No | ❌ No | ❌ No |
| **Read Preferences** | ✅✅✅ primary, replica, mixed, strong | 🟡 **Architectural** (Not in SeaORM mapping, may exist) | ❌ No | ❌ No | ❌ No |
| **Distributed Caching** | ✅✅✅✅ **LifeReflector (UNIQUE)** | 🟡 **Architectural** (Not in SeaORM mapping, may exist) | ❌ No | ❌ No | ❌ No |
| **Cache Coherence** | ✅✅✅✅ **Zero-stale reads (UNIQUE)** | 🟡 **Architectural** (Not in SeaORM mapping, may exist) | ❌ No | ❌ No | ❌ No |
| **TTL-Based Active Set** | ✅✅✅✅ **Adaptive caching (UNIQUE)** | 🟡 **Architectural** (Not in SeaORM mapping, may exist) | ❌ No | ❌ No | ❌ No |
| **PostgreSQL Features** | ✅✅✅ Views, materialized views, JSONB, FTS, PostGIS, partitioning | 🟡 **Partial** (JSONB ✅ core feature, others future) | ✅✅✅ Most features | ✅✅✅ Most features | ✅✅✅ All features (raw SQL) |
| **Observability** | ✅✅✅ Prometheus, OpenTelemetry, comprehensive metrics | 🟡 **Partial** (Metrics infrastructure exists) | ✅✅ Basic metrics | ⚠️ Limited | ⚠️ Limited |
| **Developer Experience** | ✅✅✅ Familiar API, no async/await, clear errors | ✅ **Implemented** (SeaORM-like API) | ✅✅✅ Good, async/await required | ⚠️ Complex type system | ✅✅ Good, async/await required |
| **Learning Curve** | ✅✅ Moderate (familiar if you know SeaORM) | ✅ **Implemented** (SeaORM-like API) | ✅✅ Moderate | ⚠️ Steep (complex macros) | ✅✅ Moderate |
| **Production Ready** | ✅✅✅ Complete observability, health checks, metrics | 🟡 **Partial** (Core ORM ready, advanced features pending) | ✅✅✅ Production ready | ✅✅✅ Production ready | ✅✅✅ Production ready |
| **Multi-Database** | ❌ PostgreSQL only (by design) | ✅ **By Design** | ✅✅ PostgreSQL, MySQL, SQLite | ✅✅ PostgreSQL, MySQL, SQLite | ✅✅✅ PostgreSQL, MySQL, SQLite, MSSQL |
| **Coroutine Runtime** | ✅✅✅✅ **Native support (UNIQUE)** | ✅ **Implemented** | ❌ Incompatible | ❌ Incompatible | ❌ Incompatible |

### Legend

**Implementation Status Column:**
- ✅ **Implemented** = Feature is fully implemented and working
- 🟡 **Partial/Future/Architectural** = Partially implemented, planned for future, or architectural feature (not in SeaORM mapping)
- ❌ **Not Implemented** = Feature promised but not yet implemented

**Feature Comparison Columns:**
- ✅✅✅✅ = **Unique advantage** (no other ORM has this)
- ✅✅✅ = Excellent support
- ✅✅ = Good support
- ✅ = Basic support
- ⚠️ = Limited or manual implementation required
- ❌ = Not supported

### Implementation Status Summary

**✅ Fully Implemented (Core ORM - 67% of SeaORM parity):**
- Core traits (LifeModelTrait, ModelTrait, ActiveModelTrait, ColumnTrait, PrimaryKeyTrait)
- Complete CRUD operations (insert, update, save, delete)
- Relations system (has_one, has_many, belongs_to, has_many_through) with composite key support
- Query builder (19/20 methods, 95% coverage)
- Eager/lazy loading with multiple strategies
- Pagination, batch operations, upsert
- Entity hooks (ActiveModelBehavior with 8 lifecycle hooks)
- JSON support (core feature, always enabled)
- Partial models (DerivePartialModel)
- Multi-hop relationships (DeriveLinked - competitive advantage)

**🟡 Partially Implemented / Future:**
- Migrations (infrastructure may exist, but DeriveMigrationName macro missing)
- Subqueries, CTEs, Window Functions (planned for future)
- PostgreSQL advanced features (JSONB ✅, others future)
- Observability (metrics infrastructure exists, full implementation pending)

**❌ Not Yet Implemented (Promised but Missing):**
- Validators (field & model-level)
- Soft deletes
- Auto-managed timestamps
- Session/Unit of Work pattern
- Scopes (named query scopes)
- Model Managers (custom query methods)
- F() Expressions (database-level expressions)
- Schema inference (Diesel `table!` equivalent)

**🟡 Architectural Features (Not in SeaORM mapping, status unclear):**
- LifeReflector (distributed cache coherence)
- Replica read support with WAL-based routing
- Read preferences (primary, replica, mixed, strong)
- TTL-based active set caching

**Overall Progress:** ~67% of SeaORM feature parity achieved. Core ORM functionality is complete and production-ready. Advanced features (validators, soft deletes, scopes) and architectural features (LifeReflector, replica routing) are pending.

### Key Differentiators

**Lifeguard's Unique Advantages:**
1. **LifeReflector** - Distributed cache coherence (Oracle Coherence-level) - **NO OTHER ORM HAS THIS** (🟡 Status unclear)
2. **Coroutine-Native** - No async overhead, deterministic scheduling - **UNIQUE TO LIFEGUARD** ✅
3. **WAL-Based Replica Routing** - Automatic health monitoring - **UNIQUE TO LIFEGUARD** (🟡 Status unclear)
4. **TTL-Based Active Set** - Adaptive caching - **UNIQUE TO LIFEGUARD** (🟡 Status unclear)
5. **DeriveLinked Macro** - Multi-hop relationship code generation - **COMPETITIVE ADVANTAGE** ✅ (SeaORM doesn't have this)
6. **Session/Unit of Work** - Identity map, automatic change tracking - **NOT IN OTHER RUST ORMs** (❌ Not yet implemented)

**Where Lifeguard Matches or Exceeds:**
- ✅ Complete SeaORM API parity (67% feature coverage, core ORM complete)
- ✅ Relations system with composite keys and eager/lazy loading
- ✅ Query builder with 95% method coverage
- ✅ Better performance potential (2-5× faster on hot paths - architectural)
- ✅ Lower memory footprint (architectural)
- ✅ Predictable latency (architectural)

**Trade-offs:**
- ❌ PostgreSQL-only (by design - enables advanced features)
- ❌ Requires `may` coroutine runtime (not Tokio)
- ❌ Smaller ecosystem (newer project)
- ⚠️ Some promised features not yet implemented (validators, soft deletes, scopes, etc.)

### Performance Comparison (Estimated)

| Metric | Lifeguard | SeaORM | Diesel | SQLx |
|--------|-----------|--------|--------|------|
| **Simple Query Latency** | 0.1-0.5ms | 0.5-2ms | 0.2-1ms | 0.5-2ms |
| **Hot Path Throughput** | 2-5× faster | Baseline | 1-2× faster | Baseline |
| **Small Query Overhead** | Minimal | Future allocation | Minimal | Future allocation |
| **Memory per Connection** | ~100 bytes | ~1-2 KB | ~100 bytes | ~1-2 KB |
| **Concurrent Connections** | 800+ (1MB stack) | Limited by Tokio | Limited by threads | Limited by Tokio |
| **p99 Latency** | < 5ms (predictable) | 5-20ms (variable) | < 5ms (predictable) | 5-20ms (variable) |

*Note: Performance numbers are estimates based on architecture. Actual benchmarks will be published after implementation.*

### Ecosystem Compatibility

**⚠️ Important: BRRTRouter and Lifeguard are a parallel ecosystem, separate from async/await Rust.**

These are **two incompatible worlds** with the only commonality being Rust itself:

| Ecosystem | Runtime | ORM Options | Incompatible With |
|-----------|---------|-------------|-------------------|
| **BRRTRouter + Lifeguard** | `may` coroutines | Lifeguard only | SeaORM, Diesel (async), SQLx, Tokio |
| **Tokio + Async ORMs** | `async/await` | SeaORM, Diesel, SQLx | BRRTRouter, Lifeguard, `may` |

**You cannot mix and match.** If you're using BRRTRouter, you **must** use Lifeguard. The async/await ORMs (SeaORM, Diesel, SQLx) are fundamentally incompatible with the `may` coroutine runtime.

### When to Use Each Ecosystem

**Use BRRTRouter + Lifeguard if:**
- ✅ You're building with **BRRTRouter** (the coroutine API framework)
- ✅ You need **distributed cache coherence** (LifeReflector - unique to Lifeguard)
- ✅ You need **extreme scale** (millions of requests/second)
- ✅ You need **predictable latency** (API routers, real-time systems)
- ✅ You're **PostgreSQL-only** (enables advanced features)
- ✅ You want **Oracle Coherence-level functionality**

**Use Tokio + Async ORMs if:**
- ✅ You're using **Tokio/async-await** runtime
- ✅ You need **multi-database support** (PostgreSQL, MySQL, SQLite, MSSQL)
- ✅ You want **mature, well-documented ORMs** (SeaORM, Diesel, SQLx)
- ✅ You don't need distributed cache coherence
- ✅ You're building traditional async/await microservices

**The choice is made at the ecosystem level, not the ORM level.** Once you choose BRRTRouter, Lifeguard is your only ORM option. Once you choose Tokio, you can choose between SeaORM, Diesel, or SQLx—but you cannot use BRRTRouter.

---

## 🚀 Performance

**Target Performance:**
- 2-5× faster than async ORMs on hot paths
- 10×+ faster on small queries (no future allocation overhead)
- Predictable p99 latency (< 5ms for simple queries)
- Lower memory footprint than async alternatives

**Real-World Use Cases:**
- **BRRTRouter**: High-throughput API routing with sub-millisecond database access (100,000+ requests/second)
- **High-Scale Microservices**: Applications requiring millions of requests/second with limited database connections
- **Low-Latency Systems**: Real-time applications needing predictable p99 latency (< 5ms) for database operations

---

## 📚 Documentation

- [Architecture Overview](./docs/ARCHITECTURE.md)
- [Epics & Stories](./docs/EPICS/)
- [SeaORM Feature Audit](./docs/EPICS/Epic_02/SEAORM_AUDIT.md)
- [Competitive ORM Analysis](./docs/EPICS/COMPETITIVE_ORM_ANALYSIS.md)
- [Missing Features Analysis](./docs/EPICS/MISSING_FEATURES_ANALYSIS.md)
- [Blog Post](./docs/wip/LIFEGUARD_BLOG_POST.md)

---

## 🤝 Contributing

Lifeguard is being rebuilt from scratch. We welcome:
- 📝 Documentation improvements
- 🐛 Bug reports
- 💡 Feature suggestions
- 🧪 Testing and feedback

See [EPICS](./docs/EPICS/) for current development priorities.


---

## 📜 License

Licensed under Apache-2.0.
