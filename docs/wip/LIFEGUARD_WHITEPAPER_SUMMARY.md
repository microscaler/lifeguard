# Lifeguard: Building a Parallel Universe ORM for Coroutine-Native Rust

## Executive Summary

**Lifeguard** is an ambitious project to build a **complete, production-grade ORM and data access platform** specifically designed for Rust's `may` coroutine runtime. This is not an incremental improvement or wrapper around existing solutions—it is a **ground-up rebuild** of database access patterns for a fundamentally different concurrency model.

**The Core Problem:** Existing Rust ORMs (SeaORM, Diesel, SQLx) are built for async/await and Tokio. The `may` coroutine runtime uses stackful coroutines, not async futures. These are **fundamentally incompatible architectures**—you cannot bridge them without significant performance penalties and architectural compromises.

**The Solution:** Build a "parallel universe ORM"—a complete alternative that provides SeaORM-like functionality but is architected from the ground up for coroutines. No async runtime. No Tokio. Pure coroutine-native database access.

**Why This Matters:** Coroutines offer deterministic scheduling, lower memory overhead, and predictable latency—critical for high-throughput microservices, API routers, and real-time systems. But without a proper ORM, developers are forced to choose between:
- Using async ORMs (performance overhead, complexity)
- Writing raw SQL (no type safety, no migrations, no abstractions)
- Building custom solutions (reinventing the wheel)

Lifeguard solves this by providing a **complete data platform** that matches the feature set of async ORMs but is built for coroutines.

---

## What We're Building

### 1. Core ORM Layer: LifeModel & LifeRecord

A complete ORM system with two primary abstractions:

**LifeModel** (Immutable Database Rows)
- Represents database rows as immutable Rust structs
- Generated via `#[derive(LifeModel)]` procedural macro
- Provides type-safe query builders
- Automatic row-to-struct mapping
- Metadata generation (table names, columns, relationships)

**LifeRecord** (Mutable Change Sets)
- Separate abstraction for inserts and updates
- Generated via `#[derive(LifeRecord)]` procedural macro
- Type-safe mutation builders
- Automatic SQL generation via SeaQuery
- Returns LifeModel instances after writes

**Example:**
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

// Usage
let user = NewUser { email: "test@example.com".into() }
    .insert(&pool)?;

let found = User::find_by_id(&pool, user.id)?;
```

### 2. Connection Pool: LifeguardPool

A sophisticated connection pool designed for coroutines:

- **Persistent connections:** Pre-allocated, long-lived Postgres connections
- **Semaphore-based concurrency:** Bounded acquisition prevents connection storms
- **Health monitoring:** Automatic detection and reconnection of failed connections
- **Metrics integration:** Real-time pool utilization, latency, and health metrics
- **Coroutine-native:** No async runtime, pure coroutine I/O

**Key Innovation:** Unlike async pools that create connections on-demand, LifeguardPool maintains a fixed-size pool of persistent connections, aggressively reusing them to avoid the overhead of connection creation (2-50ms per connection).

### 3. Executor Layer: LifeExecutor

A clean abstraction over database I/O:

- Wraps `may_postgres::Client` (coroutine-native Postgres client)
- Provides unified interface for ORM, migrations, and raw SQL
- No async/await, no Tokio, pure coroutine I/O
- Type-safe parameter binding
- Error handling with structured error types

### 4. Migration System: LifeMigration

A complete schema evolution system:

- Borrows SeaORM's migration DSL patterns (compatible, runtime-agnostic)
- `LifeMigration` trait for up/down migrations
- CLI tooling: `lifeguard migrate up/down/status`
- Full PostgreSQL feature support:
  - Tables, columns, indexes (unique, partial, composite)
  - Foreign keys, constraints (check, exclusion)
  - Views, materialized views
  - Sequences, triggers, functions
  - JSONB, full-text search
  - PostGIS, partitioning (v3)

### 5. Cache Coherence: LifeReflector

A **revolutionary distributed cache coherence system**—this is Lifeguard's "killer feature":

**The Problem:** Microservices need caching (Redis) for performance, but maintaining cache consistency across multiple services is notoriously difficult. Traditional approaches:
- Manual invalidation (error-prone, complex)
- TTL-based expiration (stale data, cache misses)
- Event-driven invalidation (complex infrastructure)

**The Solution: LifeReflector**

A **standalone microservice** that maintains cluster-wide cache coherence:

- **Leader-elected Raft system:** Only one active reflector at a time (no duplicate work)
- **Postgres LISTEN/NOTIFY integration:** Subscribes to database change events
- **Intelligent cache refresh:** Only updates keys that exist in Redis (TTL-based active set)
- **Zero-stale reads:** Redis always reflects current database state
- **Horizontal scaling:** All microservices benefit from single reflector

**How It Works:**
1. LifeRecord writes to Postgres → triggers NOTIFY
2. LifeReflector (leader) receives notification
3. Checks if key exists in Redis (active item)
4. If exists → refreshes from database → updates Redis
5. If not → ignores (inactive item, TTL expired)
6. All microservices read from Redis → always fresh data

**Result:** Oracle Coherence-level cache consistency with Postgres + Redis, but:
- Lighter (no JVM, no enterprise licensing)
- Faster (coroutine-native, no async overhead)
- Simpler (TTL-based active set, no manual invalidation)
- Open source

### 6. Replica Read Support

Advanced read routing with WAL lag awareness:

- **WAL position tracking:** Monitors `pg_current_wal_lsn()` vs `pg_last_wal_replay_lsn()`
- **Dynamic health checks:** Automatically detects replica lag
- **Intelligent routing:** Routes reads to replicas only when healthy
- **Automatic fallback:** Falls back to primary if replicas are stale
- **Strong consistency mode:** Optional causal read-your-writes consistency

**Read Preference Modes:**
- `primary` - Always read from primary
- `replica` - Use replicas when healthy
- `mixed` - Automatic selection based on health
- `strong` - Causal consistency (wait for replica to catch up)

### 7. Observability & Metrics

Comprehensive instrumentation:

- **Prometheus metrics:** Query latency, pool utilization, cache hit rates
- **OpenTelemetry integration:** Distributed tracing, spans
- **Grafana dashboards:** Pre-configured monitoring
- **Alert rules:** Connection failures, replica lag, cache coherence issues

### 8. Developer Experience

- **Ergonomic macros:** `#[derive(LifeModel)]`, `#[derive(LifeRecord)]`
- **Type-safe queries:** Compile-time SQL validation
- **Clear error messages:** Structured error types
- **Comprehensive documentation:** Examples, guides, API reference
- **Testkit:** Docker Compose stack for development and testing

---

## Why This Is Necessary: The Fundamental Incompatibility

### The Async vs. Coroutine Divide

**Async/await (Tokio, SeaORM):**
- Uses heap-allocated futures
- Poll-based scheduling (indeterministic)
- Requires `Send + Sync` bounds
- Context switching via future polling
- Designed for I/O-bound workloads

**Coroutines (`may`):**
- Uses stackful coroutines (user-space stacks)
- Cooperative scheduling (deterministic)
- Not `Send + Sync` (coroutines are not thread-safe)
- Context switching via stack switching
- Designed for high-throughput, predictable latency

**The Incompatibility:**
- SeaORM's `DatabaseConnection` is async and requires `Send + Sync`
- Coroutines are not `Send + Sync` (they run on user-space stacks)
- You cannot use async traits in coroutine context without an async runtime
- Embedding Tokio in coroutines defeats the purpose (overhead, complexity)

**Attempted Workaround (Current Implementation):**
- Spawn `tokio::runtime::current_thread` in each coroutine worker
- Wrap SeaORM calls in async blocks
- Use channels to bridge sync/async boundary

**Why This Fails:**
- Still requires Tokio (defeats coroutine benefits)
- Double indirection (coroutine → Tokio → SeaORM → Postgres)
- Performance overhead (future polling, heap allocations)
- Complexity (managing two concurrency models)

**The Only Solution:**
Build a coroutine-native database client and ORM layer. No async runtime. No Tokio. Pure coroutine I/O.

---

## Why This Is Worth It: The Value Proposition

### 1. Performance Benefits

**Coroutine Advantages:**
- **Deterministic scheduling:** Predictable latency, no future polling overhead
- **Lower memory overhead:** Stackful coroutines vs. heap-allocated futures
- **Faster context switching:** Stack switching vs. future state machine transitions
- **No async tax:** Zero async runtime overhead

**Real-World Impact:**
- 2-5× faster than async ORMs on hot paths
- 10×+ faster on small queries (no future allocation overhead)
- Predictable p99 latency (critical for API routers)
- Lower memory footprint (important for high-concurrency systems)

### 2. Architectural Alignment

**BRRTRouter Integration:**
- BRRTRouter uses `may` coroutines for request handling
- Lifeguard provides coroutine-native database access
- Perfect architectural fit (no async/await boundaries)
- Unified concurrency model across the stack

**Microservice Architecture:**
- High-throughput services benefit from coroutine performance
- LifeReflector provides distributed cache coherence
- Replica read support enables horizontal scaling
- Complete observability for production operations

### 3. Unique Features

**LifeReflector:**
- Distributed cache coherence (Oracle Coherence-level functionality)
- TTL-based active set (no full-database caching)
- Leader-elected Raft system (high availability)
- Zero-stale reads across microservices

**Replica Read Support:**
- WAL lag awareness (automatic health checks)
- Intelligent routing (replicas when healthy, primary when stale)
- Strong consistency mode (causal reads)

**Complete PostgreSQL Support:**
- All advanced features (views, FTS, JSONB, PostGIS)
- Migration system (borrowing SeaORM patterns)
- Type-safe query builders (SeaQuery integration)

### 4. Developer Experience

**Familiar Patterns:**
- Similar to SeaORM/Diesel (developers can learn quickly)
- Procedural macros (`#[derive(LifeModel)]`)
- Type-safe queries (compile-time validation)
- Clear error messages (structured error types)

**Better Than Alternatives:**
- No async/await complexity (synchronous API)
- No `Send + Sync` bounds (coroutine-friendly)
- Integrated caching (LifeReflector handles coherence)
- Complete tooling (migrations, CLI, testkit)

---

## The Scope: What We're Actually Building

### Phase 1: Foundation (Weeks 1-3)
- Remove SeaORM and Tokio dependencies
- Integrate `may_postgres` as database client
- Implement `LifeExecutor` trait
- Redesign `LifeguardPool` for `may_postgres`
- Basic metrics and observability

### Phase 2: ORM Core (Weeks 3-6)
- Build `LifeModel` derive macro
- Build `LifeRecord` derive macro
- Implement basic CRUD operations
- Integrate SeaQuery for SQL building
- Type-safe query builders

### Phase 3: Migrations (Weeks 6-8)
- Implement `LifeMigration` trait
- Build migration runner
- Create CLI tooling (`lifeguard migrate`)
- Support core PostgreSQL features (tables, indexes, FKs)

### Phase 4: v1 Release (Weeks 8-10)
- Complete PostgreSQL feature support (views, JSONB, sequences)
- Testkit infrastructure
- Comprehensive documentation
- Integration with BRRTRouter
- Performance benchmarks

### Phase 5: Advanced Features (Quarter 2)
- LifeReflector (distributed cache coherence)
- Redis integration (read-through/write-through)
- Replica read support (WAL lag awareness)
- Relation loading (has_one, has_many, belongs_to)
- Materialized views, generated columns
- Full-text search, window functions

### Phase 6: Enterprise Features (Quarter 3)
- PostGIS support
- Partitioning
- Triggers and stored procedures
- Schema introspection tools
- Code generation from database
- Advanced performance optimizations

---

## Why This Is "Bonkers" (And Why We're Doing It Anyway)

### The Challenges

1. **Greenfield Development:**
   - Building a complete ORM from scratch
   - No existing coroutine-native ORM to reference
   - Must match SeaORM's feature set
   - Estimated 6-12 months for v1

2. **Small Ecosystem:**
   - `may` runtime is less popular than Tokio
   - `may_postgres` has smaller community
   - Fewer examples, less documentation
   - Must build tooling and ecosystem

3. **Maintenance Burden:**
   - Maintaining a complete ORM is significant work
   - PostgreSQL feature support requires ongoing development
   - Migration system needs continuous updates
   - Documentation and examples require constant attention

4. **Risk:**
   - What if `may` runtime becomes obsolete?
   - What if async Rust becomes the only viable path?
   - What if the effort doesn't pay off?

### Why We're Doing It Anyway

1. **The Problem Is Real:**
   - Coroutines offer real performance benefits
   - BRRTRouter and similar systems need coroutine-native database access
   - Current workarounds are inadequate
   - The gap in the ecosystem is significant

2. **The Opportunity Is Unique:**
   - LifeReflector provides distributed cache coherence (rare feature)
   - Complete coroutine-native stack (BRRTRouter + Lifeguard)
   - First-mover advantage in coroutine-native ORM space
   - Potential to become the standard for coroutine-based Rust services

3. **The Architecture Is Sound:**
   - Clear separation of concerns
   - Borrow compatible components (SeaQuery, migration patterns)
   - Build only what's necessary (no over-engineering)
   - Proven foundation (`may_postgres`)

4. **The Vision Is Compelling:**
   - High-performance microservices with predictable latency
   - Distributed cache coherence out of the box
   - Complete PostgreSQL feature support
   - Developer-friendly API
   - Production-ready observability

5. **The Alternative Is Worse:**
   - Continue using async ORMs (performance overhead, complexity)
   - Write raw SQL (no type safety, no migrations)
   - Build custom solutions (reinventing the wheel)
   - Accept the limitations (compromised architecture)

---

## The Competitive Landscape

### What Exists Today

**Async ORMs (SeaORM, Diesel, SQLx):**
- Mature, well-documented, large communities
- Built for Tokio/async-await
- Incompatible with coroutine runtimes

**Coroutine Runtimes (`may`):**
- High-performance, deterministic scheduling
- Lower memory overhead
- No ORM support (this is the gap)

**The Gap:**
- No coroutine-native ORM exists
- Developers must choose: async ORM (overhead) or raw SQL (no safety)
- No distributed cache coherence solution
- No integrated replica read support

### What Lifeguard Provides

**Complete ORM:**
- LifeModel/LifeRecord (full ORM functionality)
- Type-safe queries (compile-time validation)
- Migration system (schema evolution)
- PostgreSQL feature support (views, FTS, JSONB, etc.)

**Advanced Features:**
- LifeReflector (distributed cache coherence)
- Replica read support (WAL lag awareness)
- Complete observability (Prometheus, OTel, Grafana)

**Unique Value:**
- First coroutine-native ORM for Rust
- Oracle Coherence-level cache consistency
- Integrated with BRRTRouter ecosystem
- Production-ready from day one

---

## Success Criteria

### Technical Goals

1. **Performance:**
   - 2-5× faster than SeaORM on hot paths
   - 10×+ faster on small queries
   - Predictable p99 latency (< 5ms for simple queries)
   - Lower memory footprint than async alternatives

2. **Feature Completeness:**
   - Full PostgreSQL feature support
   - Migration system matching SeaORM capabilities
   - Type-safe query builders
   - Complete observability

3. **Reliability:**
   - Zero data loss
   - Automatic connection recovery
   - Replica health monitoring
   - Cache coherence guarantees

### Adoption Goals

1. **BRRTRouter Integration:**
   - Seamless integration with BRRTRouter handlers
   - Unified coroutine model across stack
   - Production deployments

2. **Community:**
   - Documentation and examples
   - Active maintenance and support
   - Growing user base

3. **Ecosystem:**
   - Standard for coroutine-native Rust services
   - Reference implementation for coroutine ORMs
   - Foundation for future coroutine-based tools

---

## Conclusion: Why This Matters

Lifeguard is not just another ORM—it's a **complete data platform** designed for a fundamentally different concurrency model. It solves a real problem (coroutine-native database access) with a comprehensive solution (ORM + migrations + caching + observability).

**The "Bonkers" Factor:**
Yes, building a complete ORM from scratch is ambitious. Yes, it's a significant investment. Yes, there are risks.

**But:**
- The problem is real and unsolved
- The architecture is sound and well-planned
- The opportunity is unique (first-mover advantage)
- The alternative is worse (compromised architecture or raw SQL)
- The vision is compelling (high-performance, predictable, production-ready)

**The Bottom Line:**
If we want coroutine-native Rust services to succeed, we need coroutine-native database access. Lifeguard provides that, plus distributed cache coherence, replica read support, and complete observability. It's not just an ORM—it's a **complete data platform** for the coroutine era.

This is worth doing because it unlocks a new class of high-performance, predictable, coroutine-native microservices. And that's a future worth building.

---

## Next Steps

1. **Complete the analysis** (this document)
2. **Finalize the PRD** (detailed specifications)
3. **Begin Phase 1** (remove SeaORM, integrate `may_postgres`)
4. **Build the foundation** (LifeExecutor, LifeguardPool)
5. **Implement the ORM** (LifeModel, LifeRecord)
6. **Create the ecosystem** (migrations, tooling, documentation)

**The journey begins now.**

