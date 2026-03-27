# LIFEGUARD PRD — Coroutine-Native Postgres Data Platform for Rust

**Version:** 4.0 — Final Consolidated PRD  
**Date:** 2025-01-XX  
**Status:** Authoritative System Specification

---

## 0. Executive Summary

Lifeguard is a **high-performance, coroutine-native Postgres data platform** built for Rust's `may` coroutine runtime. It provides a complete ORM, connection pooling, distributed cache coherence, and advanced Postgres features—all without async runtime overhead.

**Key Differentiators:**
- **Coroutine-native:** No async/await, no Tokio, pure coroutine I/O
- **LifeReflector:** Distributed cache coherence (Oracle Coherence-level)
- **WAL-based replica routing:** Automatic health monitoring
- **Persistent connection pooling:** Handles millions of requests with limited connections
- **Postgres-first:** Advanced features (views, FTS, JSONB, PostGIS, partitioning)

**Current Status:** Being rebuilt from scratch. Current implementation uses SeaORM/Tokio bridge (to be replaced).

---

## 1. Product Overview

### 1.1 What is Lifeguard?

Lifeguard is a **complete, production-grade ORM and data access platform** designed for:
- **BRRTRouter** (coroutine API framework)
- High-throughput microservices (millions of requests/second)
- Enterprise ERP backends
- Applications requiring predictable, low-latency database access

### 1.2 Core Value Propositions

1. **Performance:** 2-5× faster than async ORMs, 10×+ faster on small queries
2. **Predictability:** Deterministic scheduling, no async overhead
3. **Scalability:** Persistent connection pooling for extreme scale
4. **Cache Coherence:** LifeReflector provides distributed cache consistency
5. **Postgres Power:** Full support for advanced Postgres features

### 1.3 Target Users

- Developers building with `may` coroutine runtime
- Teams needing high-throughput database access
- Organizations requiring distributed cache coherence
- Applications with extreme scale requirements (millions of requests/second)
- Systems needing predictable latency (< 5ms p99)

---

## 2. System Architecture

### 2.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                         │
│              (BRRTRouter, Microservices)                    │
└──────────────────────┬──────────────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────────────┐
│                   LifeguardPool                              │
│  Persistent Connection Slots (N = max_connections)          │
│  Semaphore-Based Concurrency Control                        │
│  Health Monitoring + Auto-Reconnection                      │
└──────────────────────┬──────────────────────────────────────┘
                       │ acquire()
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                    LifeExecutor                              │
│              (may_postgres wrapper)                          │
└──────────────────────┬──────────────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────────────┐
│                    ORM Layer                                 │
│         LifeModel + LifeRecord + LifeQuery                  │
└──────────────────────┬──────────────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────────────┐
│                   SeaQuery                                   │
│              (SQL Builder - Borrowed)                        │
└──────────────────────┬──────────────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────────────┐
│                 may_postgres                                 │
│         (Coroutine-Native Postgres Client)                   │
└──────────────────────┬──────────────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────────────┐
│                  PostgreSQL                                  │
│            Primary + Read Replicas                          │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│              LifeReflector (Separate Microservice)           │
│  Leader-Elected Raft | LISTEN/NOTIFY | Redis Cache Refresh  │
└──────────────────────┬──────────────────────────────────────┘
                       │
┌──────────────────────▼──────────────────────────────────────┐
│                    Redis                                     │
│         Distributed Cache (TTL-Based Active Set)            │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Component Responsibilities

| Component | Responsibility | Technology |
|-----------|---------------|------------|
| **LifeguardPool** | Persistent connection management, semaphore-based acquisition, health monitoring | Rust, `may` coroutines |
| **LifeExecutor** | Database execution abstraction over `may_postgres` | Rust trait |
| **LifeModel** | Immutable database row representation | Procedural macro |
| **LifeRecord** | Mutable insert/update builder | Procedural macro |
| **LifeQuery** | Query builder wrapper around SeaQuery | Rust, SeaQuery |
| **LifeMigration** | Schema evolution system | Rust trait + CLI |
| **LifeReflector** | Distributed cache coherence microservice | Separate Rust crate, Raft |
| **SeaQuery** | SQL building (borrowed) | External crate |
| **may_postgres** | Coroutine-native Postgres client | External crate |

---

## 3. Core Components

### 3.1 LifeguardPool — Persistent Connection Pool

#### Requirements

1. **Pre-allocate all connections at startup** (`max_connections`)
2. **Persistent connection slots** (no per-job creation)
3. **Semaphore-based concurrency control** (bounded acquisition)
4. **Health monitoring** (automatic reconnection on failure)
5. **Replica support** (WAL lag monitoring, dynamic routing)
6. **Metrics** (pool size, active connections, wait times, replica lag)

#### Connection Slot Structure

```rust
struct LifeConnectionSlot {
    id: usize,
    conn: may_postgres::Client,
    in_use: AtomicBool,
    last_used: Instant,
    healthy: AtomicBool,
    replica_info: Option<ReplicaInfo>, // For read replicas
}

struct ReplicaInfo {
    url: String,
    current_lsn: Lsn,
    replay_lsn: Lsn,
    lag_bytes: u64,
    lag_seconds: f64,
    healthy: bool,
}
```

#### Acquire Flow

```
1. Acquire semaphore token (bounded by max_connections)
2. Find free connection slot
3. Mark slot as in_use = true
4. Return slot to requester
5. Execute query via LifeExecutor
6. Mark slot as in_use = false
7. Return semaphore token
```

#### Replica Health Monitoring

- Periodically query `pg_current_wal_lsn()` on primary
- Query `pg_last_wal_replay_lsn()` on each replica
- Calculate lag: `lag_bytes = current_lsn - replay_lsn`
- Mark replica unhealthy if lag exceeds thresholds
- Automatically route reads away from unhealthy replicas

#### Read Preference Modes

```rust
enum ReadPreference {
    Primary,    // Always read from primary
    Replica,    // Use replicas when healthy
    Mixed,      // Automatic: Redis → replica → primary
    Strong,     // Causal consistency (wait for replica to catch up)
}
```

#### Configuration

```toml
[database]
url = "postgres://user:pass@localhost/db"
max_connections = 32
min_connections = 8
connection_timeout_ms = 1000
acquire_timeout_ms = 5000

[replicas]
read_preference = "mixed"  # primary | replica | mixed | strong
replica_urls = [
    "postgres://user:pass@replica1:5432/db",
    "postgres://user:pass@replica2:5432/db"
]
lag_threshold_seconds = 1.0
lag_threshold_bytes = 1_000_000  # 1MB

[pool]
retry_count = 3
health_check_interval_seconds = 30
```

---

### 3.2 LifeExecutor — Database Execution Abstraction

#### Trait Definition

```rust
pub trait LifeExecutor {
    fn execute(&self, sql: &str, params: &[&(dyn ToSql)]) -> Result<u64, LifeError>;
    fn query(&self, sql: &str, params: &[&(dyn ToSql)]) -> Result<Vec<LifeRow>, LifeError>;
    fn query_one(&self, sql: &str, params: &[&(dyn ToSql)]) -> Result<Option<LifeRow>, LifeError>;
}
```

#### Implementation

- Wraps `may_postgres::Client`
- Synchronous (no async)
- Used by ORM layer and migrations
- Provides unified error handling

---

### 3.3 LifeModel — Immutable Database Row Representation

#### Purpose

- Represents database rows as immutable Rust structs
- Generated via `#[derive(LifeModel)]` procedural macro
- Provides type-safe query builders
- Automatic row-to-struct mapping

#### Example

```rust
#[derive(LifeModel)]
#[table = "users"]
#[cache(primary = true, ttl_seconds = 600, reflector = true)]
struct User {
    #[primary_key]
    id: i64,
    email: String,
    is_active: bool,
    created_at: DateTime<Utc>,
}
```

#### Generated API

```rust
impl User {
    pub const TABLE: &'static str = "users";
    
    // Query methods
    pub fn find() -> LifeQueryBuilder<Self>;
    pub fn find_by_id(exec: &impl LifeExecutor, id: i64) -> Result<Self, LifeError>;
    pub fn find_one(exec: &impl LifeExecutor) -> Result<Option<Self>, LifeError>;
    
    // Row mapping
    fn from_row(row: &LifeRow) -> Result<Self, LifeError>;
    
    // Conversion to record
    pub fn to_record(&self) -> UserRecord;
}
```

#### Query Builder Usage

```rust
let users = User::find()
    .filter(User::Email.eq("test@example.com"))
    .filter(User::IsActive.eq(true))
    .order_by(User::CreatedAt.desc())
    .limit(20)
    .all(&pool)?;
```

---

### 3.4 LifeRecord — Mutable Insert/Update Builder

#### Purpose

- Separate abstraction for inserts and updates
- Generated via `#[derive(LifeRecord)]` procedural macro
- Type-safe mutation builders
- Automatic SQL generation via SeaQuery

#### Example

```rust
#[derive(LifeRecord)]
#[table = "users"]
struct NewUser {
    email: String,
    is_active: bool,
}

#[derive(LifeRecord)]
#[table = "users"]
struct UserRecord {
    #[primary_key]
    id: i64,
    email: Option<String>,
    is_active: Option<bool>,
}
```

#### Usage

```rust
// Insert
let user = NewUser {
    email: "test@example.com".into(),
    is_active: true,
}.insert(&pool)?;

// Update
let updated = user.to_record()
    .set_email(Some("new@example.com".into()))
    .set_is_active(Some(false))
    .update(&pool)?;

// Delete
user.delete(&pool)?;
```

---

### 3.5 LifeQuery — Query Builder

#### Purpose

- Wrapper around SeaQuery for SQL building
- Type-safe query construction
- Supports all SeaQuery features

#### Features

- SELECT, INSERT, UPDATE, DELETE
- Filtering (`eq`, `ne`, `gt`, `lt`, `like`, `in`, etc.)
- Joins (inner, left, right, cross)
- Ordering, grouping, having
- Aggregates (COUNT, SUM, AVG, etc.)
- Subqueries, CTEs
- Window functions (v2)
- Full-text search (v2)

---

### 3.6 LifeMigration — Schema Evolution System

#### Purpose

- Lifeguard-native migration system
- Borrows SeaORM migration patterns (DSL)
- Uses `LifeExecutor` (no async dependency)
- CLI tooling: `lifeguard migrate`

#### Migration Trait

```rust
pub trait LifeMigration {
    fn up(&self, exec: &impl LifeExecutor) -> Result<(), LifeError>;
    fn down(&self, exec: &impl LifeExecutor) -> Result<(), LifeError>;
    fn name(&self) -> &'static str;
}
```

#### Example Migration

```rust
pub struct CreateUsersTable;

impl LifeMigration for CreateUsersTable {
    fn up(&self, exec: &impl LifeExecutor) -> Result<(), LifeError> {
        let stmt = Table::create()
            .table(Alias::new("users"))
            .if_not_exists()
            .col(ColumnDef::new(Alias::new("id"))
                .big_integer()
                .not_null()
                .primary_key())
            .col(ColumnDef::new(Alias::new("email"))
                .string()
                .not_null()
                .unique())
            .col(ColumnDef::new(Alias::new("is_active"))
                .boolean()
                .default(false))
            .to_string(PostgresQueryBuilder);
        
        exec.execute(&stmt, &[])?;
        Ok(())
    }
    
    fn down(&self, exec: &impl LifeExecutor) -> Result<(), LifeError> {
        exec.execute("DROP TABLE IF EXISTS users", &[])?;
        Ok(())
    }
    
    fn name(&self) -> &'static str {
        "20250101_create_users_table"
    }
}
```

#### CLI Commands

```bash
lifeguard migrate up          # Apply pending migrations
lifeguard migrate down        # Rollback last migration
lifeguard migrate status      # Show migration status
lifeguard migrate create NAME # Create new migration
```

#### Supported Schema Operations

**v1:**
- Tables (create, alter, drop)
- Columns (add, remove, modify)
- Indexes (unique, partial, composite, gin, gist, hash)
- Foreign keys
- Views
- JSONB columns

**v2:**
- Materialized views
- Generated columns
- Sequences
- Check constraints
- Exclusion constraints

**v3:**
- Triggers
- Functions/procedures
- Partitioning (RANGE, HASH, LIST)
- PostGIS types

---

## 4. Advanced Features

### 4.1 LifeReflector — Distributed Cache Coherence

#### Purpose

LifeReflector is a **standalone microservice** that maintains cluster-wide cache coherence between Postgres and Redis.

#### Architecture

- **Leader-elected Raft system** (only one active reflector)
- **Postgres LISTEN/NOTIFY integration** (subscribes to database changes)
- **Redis cache refresh** (only updates keys that exist - TTL-based active set)
- **Zero-stale reads** (Redis always reflects current database state)

#### How It Works

```
1. LifeRecord writes to Postgres → triggers NOTIFY
2. LifeReflector (leader) receives notification
3. Checks if key exists in Redis (active item)
4. If exists → refreshes from database → updates Redis
5. If not → ignores (inactive item, TTL expired)
6. All microservices read from Redis → always fresh data
```

#### Cache Key Strategy

**Primary key cache:**
```
lifeguard:model:<table>:<pk>
```

**Query cache (optional):**
```
lifeguard:query:<table>:<query_hash>
```

**TTL-based active set:**
- Only recently accessed items stay cached
- Cold items expire automatically
- Reflector only touches keys that exist

#### LifeReflector Configuration

```toml
[reflector]
enabled = true
db_url = "postgres://..."
redis_url = "redis://..."
tables = ["users", "orders", "invoices"]

# TTL policy (seconds)
default_ttl = 600
user_ttl = 900

# Policy: "refresh" or "invalidate"
on_change = "refresh"

# Raft configuration
raft_cluster_size = 3
heartbeat_interval_ms = 1000
```

#### Metrics

- `reflector_notifications_total` - Notifications received
- `reflector_refreshes_total` - Cache refreshes
- `reflector_ignored_total` - Ignored notifications (inactive items)
- `reflector_active_keys` - Active cache keys
- `reflector_redis_latency_seconds` - Redis operation latency
- `reflector_pg_latency_seconds` - PostgreSQL operation latency
- `reflector_leader_changes_total` - Leader election events

#### Deployment

- Separate microservice (not embedded in each app)
- Leader-elected (Raft) for high availability
- Replicas = 1 or leader-elected cluster
- Kubernetes deployment with anti-affinity

---

### 4.2 Redis Caching Integration

#### Read-Through Cache

```rust
// LifeModel::find_by_id automatically:
// 1. Checks Redis first
// 2. Falls back to database if cache miss
// 3. Caches result for future reads
// 4. LifeReflector keeps cache fresh

let user = User::find_by_id(&pool, 42)?; // May come from Redis or DB
```

#### Write-Through Cache

```rust
// LifeRecord::insert/update automatically:
// 1. Writes to Postgres
// 2. Updates Redis key
// 3. Triggers NOTIFY for LifeReflector

let user = NewUser { email: "test@example.com".into() }
    .insert(&pool)?; // Written to both Postgres and Redis
```

#### Cache Configuration

```rust
#[derive(LifeModel)]
#[table = "users"]
#[cache(primary = true, ttl_seconds = 600, reflector = true)]
struct User {
    // ...
}
```

---

### 4.3 Replica Read Support

#### WAL Lag Monitoring

Lifeguard continuously monitors replica health:

```sql
-- On primary
SELECT pg_current_wal_lsn();

-- On replica
SELECT pg_last_wal_replay_lsn();
```

#### Dynamic Routing

- Calculates lag: `lag_bytes = current_lsn - replay_lsn`
- Marks replica unhealthy if lag exceeds thresholds
- Automatically routes reads away from unhealthy replicas
- Falls back to primary if all replicas unhealthy

#### Read Preference Modes

```rust
enum ReadPreference {
    Primary,  // Always read from primary
    Replica,  // Use replicas when healthy
    Mixed,    // Automatic: Redis → replica → primary
    Strong,   // Causal consistency (wait for replica to catch up)
}
```

#### Strong Consistency Mode

For write-followed-by-read scenarios:

```rust
// Track commit LSN from write
let commit_lsn = pool.write(...)?;

// Wait until replica LSN >= commit LSN
pool.read_with_consistency(commit_lsn, |exec| {
    User::find_by_id(exec, id)
})?;
```

#### Metrics

- `lifeguard_replica_lag_bytes{replica="r1"}`
- `lifeguard_replica_lag_seconds{replica="r1"}`
- `lifeguard_replicas_healthy`
- `lifeguard_replicas_unhealthy`

---

### 4.4 Advanced PostgreSQL Features

#### v1 — Launch Foundation

- Foreign keys
- Views
- JSONB (basic)
- Index types (btree, gin, gist, hash)
- Sequence support
- Partial indexes
- Composite primary keys

#### v2 — Enhanced ORM & Schema

- Relation loading (has_one, has_many, belongs_to, many_to_many)
- Materialized views (refresh)
- Generated columns
- Check & exclusion constraints
- Full-text search (FTS)
- Window functions
- CTEs (Common Table Expressions)
- Sequences API
- Advanced JSONB querying

#### v3 — Enterprise Extensions

- PostGIS (spatial queries)
- Partitioning (RANGE, HASH, LIST)
- Stored procedures
- Triggers
- Logical replication hooks
- Schema introspection tools
- Code generation from database
- Model inspector CLI

---

## 5. Observability

### 5.1 Prometheus Metrics

#### Pool Metrics

- `lifeguard_pool_size` - Current pool size
- `lifeguard_active_connections` - Active connections
- `lifeguard_idle_connections` - Idle connections
- `lifeguard_connection_wait_time_seconds` - Time waiting for connection
- `lifeguard_connection_retries_total` - Connection retry attempts
- `lifeguard_connection_failures_total` - Connection failures

#### Query Metrics

- `lifeguard_queries_total` - Total queries executed
- `lifeguard_query_duration_seconds` - Query execution time (histogram)
- `lifeguard_query_errors_total` - Query errors by type

#### Cache Metrics

- `lifeguard_cache_hits_total` - Cache hits
- `lifeguard_cache_misses_total` - Cache misses
- `lifeguard_cache_hit_rate` - Cache hit rate

#### Replica Metrics

- `lifeguard_replica_lag_bytes{replica="r1"}` - Replica lag in bytes
- `lifeguard_replica_lag_seconds{replica="r1"}` - Replica lag in seconds
- `lifeguard_replicas_healthy` - Number of healthy replicas
- `lifeguard_replicas_unhealthy` - Number of unhealthy replicas

#### LifeReflector Metrics

- `reflector_notifications_total` - Notifications received
- `reflector_refreshes_total` - Cache refreshes
- `reflector_ignored_total` - Ignored notifications
- `reflector_active_keys` - Active cache keys
- `reflector_redis_latency_seconds` - Redis operation latency
- `reflector_pg_latency_seconds` - PostgreSQL operation latency
- `reflector_leader_changes_total` - Leader election events

### 5.2 OpenTelemetry Tracing

- Distributed tracing for database operations
- Spans for: connection acquisition, query execution, cache operations
- Integration with existing OpenTelemetry infrastructure

### 5.3 Grafana Dashboards

Pre-configured dashboards for:
- Connection pool health
- Query performance
- Cache hit rates
- Replica lag
- LifeReflector status

---

## 6. Testing Infrastructure

### 6.1 Testkit

```rust
use lifeguard::testkit::*;

#[test]
fn test_user_operations() {
    let pool = test_pool!();
    testkit::seed_test_db(&pool)?;
    
    let user = NewUser { email: "test@example.com".into() }
        .insert(&pool)?;
    
    let fetched = User::find_by_id(&pool, user.id)?;
    assert_eq!(fetched.email, "test@example.com");
}
```

### 6.2 Docker Compose Test Environment

- Postgres (primary + replicas)
- Redis
- Prometheus
- Grafana
- Loki
- OpenTelemetry Collector

### 6.3 Test Helpers

- Test database setup/teardown
- Transaction rollback after each test
- Fixture loading helpers
- Test database isolation

---

## 7. Developer Tooling

### 7.1 CLI Commands

```bash
lifeguard migrate up          # Apply pending migrations
lifeguard migrate down        # Rollback last migration
lifeguard migrate status      # Show migration status
lifeguard migrate create NAME # Create new migration
lifeguard inspect             # Inspect database schema (v3)
lifeguard model generate      # Generate models from schema (v3)
```

### 7.2 Code Generation (v3)

- Generate `LifeModel` skeletons from database schema
- Generate migration stubs
- Schema introspection tools

---

## 8. Implementation Roadmap

### Phase 1: Foundation (Weeks 1-3)

**Goal:** Remove SeaORM/Tokio, integrate `may_postgres`

1. ✅ Remove SeaORM and Tokio dependencies
2. ✅ Integrate `may_postgres` as database client
3. ✅ Implement `LifeExecutor` trait
4. ✅ Redesign `LifeguardPool` for `may_postgres` (persistent connections)
5. ✅ Basic metrics and observability
6. ✅ Transaction support
7. ✅ Raw SQL helpers

**Deliverables:**
- `LifeExecutor` trait implemented
- `LifeguardPool` with persistent connection slots
- Basic Prometheus metrics
- OpenTelemetry tracing hooks

---

### Phase 2: ORM Core (Weeks 3-6)

**Goal:** Build LifeModel/LifeRecord ORM layer

8. ✅ Build `LifeModel` derive macro
9. ✅ Build `LifeRecord` derive macro
10. ✅ Implement basic CRUD operations
11. ✅ Integrate SeaQuery for SQL building
12. ✅ Type-safe query builders
13. ✅ Batch operations
14. ✅ Upsert support
15. ✅ Pagination helpers
16. ✅ Entity hooks & lifecycle events
17. ✅ Validators
18. ✅ Soft deletes
19. ✅ Auto-managed timestamps
20. ✅ Session/Unit of Work pattern
21. ✅ Scopes
22. ✅ Model Managers
23. ✅ F() Expressions

**Deliverables:**
- `LifeModel` and `LifeRecord` macros working
- Complete CRUD API
- Query builder with SeaQuery integration
- All SeaORM API parity features

---

### Phase 3: Migrations (Weeks 6-8)

**Goal:** Build migration system

24. ✅ Implement `LifeMigration` trait
25. ✅ Build migration runner
26. ✅ Create CLI tooling (`lifeguard migrate`)
27. ✅ Support core PostgreSQL features
28. ✅ Programmatic migrations and data seeding
29. ✅ Advanced migration operations

**Deliverables:**
- `LifeMigration` trait and runner
- CLI tool for migrations
- Support for all v1 schema operations

---

### Phase 4: v1 Release (Weeks 8-10)

**Goal:** Complete v1 feature set

30. ✅ Complete PostgreSQL feature support (v1)
31. ✅ Testkit infrastructure
32. ✅ Comprehensive documentation
33. ✅ Integration with BRRTRouter
34. ✅ Performance benchmarks

**Deliverables:**
- v1.0 release
- Complete documentation
- Performance benchmarks showing 2-5× improvement

---

### Phase 5: Advanced Features (Weeks 10-14)

**Goal:** LifeReflector, Redis, replicas

35. ✅ LifeReflector (distributed cache coherence)
36. ✅ Redis integration
37. ✅ Replica read support with WAL lag awareness
38. ✅ Complete relation support
39. ✅ Materialized views
40. ✅ Query cache support
41. ✅ Cache statistics & monitoring

**Deliverables:**
- LifeReflector microservice
- Redis caching working
- Replica routing with health monitoring
- v2.0 release

---

### Phase 6: Enterprise Features (Weeks 15-20)

**Goal:** Advanced Postgres features

42. ✅ PostGIS support
43. ✅ Partitioning
44. ✅ Triggers and stored procedures
45. ✅ Schema introspection tools (Diesel `table!` equivalent)
46. ✅ Code generation from database
47. ✅ Schema-first design

**Deliverables:**
- v3.0 release
- Complete Postgres feature support
- Enterprise-grade tooling

---

## 9. Success Criteria

### Performance

- ✅ 2-5× faster than async ORMs on hot paths
- ✅ 10×+ faster on small queries
- ✅ Predictable p99 latency (< 5ms for simple queries)
- ✅ Lower memory footprint than async alternatives

### Architecture

- ✅ Zero async runtime dependencies in core
- ✅ Zero SeaORM types in public API
- ✅ Persistent connection pool operational
- ✅ Handles millions of requests/second with limited connections

### Features

- ✅ Complete SeaORM API parity
- ✅ LifeReflector providing cache coherence
- ✅ Replica routing with health monitoring
- ✅ Full Postgres feature support

### Developer Experience

- ✅ Clean, simple API
- ✅ No async/await required
- ✅ Comprehensive documentation
- ✅ Excellent tooling

---

## 10. Risks & Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| `may_postgres` maturity | Medium | Evaluate thoroughly, have fallback plan, contribute fixes if needed |
| Macro complexity | Medium | Start simple, iterate, comprehensive testing, learn from SeaORM patterns |
| Performance regression | Low | Benchmark continuously, validate improvements, compare against baseline |
| Missing features | Medium | Prioritize by usage, implement incrementally, maintain compatibility layer |
| Breaking changes | High | Provide migration guide, maintain compatibility during transition |
| LifeReflector complexity | Medium | Start with simple leader election, iterate on Raft implementation |

---

## 11. Competitive Positioning

### Lifeguard vs. Other Rust ORMs

| Feature | Lifeguard | SeaORM | Diesel | SQLx |
|---------|-----------|--------|--------|------|
| **Concurrency Model** | ✅ Coroutine-native | ❌ Async/await | ❌ Sync-only | ❌ Async/await |
| **Performance** | ✅✅✅ 2-5× faster | ⚠️ Async overhead | ✅ Fast | ⚠️ Async overhead |
| **Distributed Caching** | ✅✅✅✅ LifeReflector | ❌ No | ❌ No | ❌ No |
| **Replica Support** | ✅✅✅ WAL-based | ❌ No | ❌ No | ❌ No |
| **Postgres Features** | ✅✅✅ Complete | ✅✅ Most | ✅✅ Most | ✅✅✅ All (raw SQL) |

### Unique Advantages

1. **LifeReflector** - Distributed cache coherence (Oracle Coherence-level) - **NO OTHER ORM HAS THIS**
2. **Coroutine-Native** - No async overhead, deterministic scheduling - **UNIQUE TO LIFEGUARD**
3. **WAL-Based Replica Routing** - Automatic health monitoring - **UNIQUE TO LIFEGUARD**
4. **TTL-Based Active Set** - Adaptive caching - **UNIQUE TO LIFEGUARD**

---

## 12. Conclusion

Lifeguard represents a **complete reimagining** of database access for coroutine-based Rust applications. By building from the ground up with `may_postgres`, we achieve:

- **Performance:** 2-5× faster than async ORMs
- **Features:** Complete ORM + distributed cache coherence + replica support
- **Architecture:** Clean, coroutine-native, no async overhead
- **Developer Experience:** Simple API, no async/await, excellent tooling

The migration from the current SeaORM/Tokio bridge to this native architecture is substantial but necessary to achieve these goals. This PRD provides the complete specification for that migration.

---

## Appendix A: Migration from Current Implementation

See `docs/MIGRATION_AUDIT.md` for detailed strip & replace analysis.

---

## Appendix B: API Reference

### LifeModel

```rust
#[derive(LifeModel)]
#[table = "users"]
struct User { /* ... */ }

// Generated methods:
User::find() -> LifeQueryBuilder<Self>
User::find_by_id(exec: &impl LifeExecutor, id: i64) -> Result<Self, LifeError>
User::find_one(exec: &impl LifeExecutor) -> Result<Option<Self>, LifeError>
```

### LifeRecord

```rust
#[derive(LifeRecord)]
#[table = "users"]
struct NewUser { /* ... */ }

// Generated methods:
NewUser::insert(exec: &impl LifeExecutor) -> Result<User, LifeError>
UserRecord::update(exec: &impl LifeExecutor) -> Result<User, LifeError>
User::delete(exec: &impl LifeExecutor) -> Result<(), LifeError>
```

### LifeguardPool

```rust
let pool = LifeguardPool::new(config)?;
let user = User::find_by_id(&pool, 42)?;
```

---

**End of PRD**
