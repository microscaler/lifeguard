Let’s produce a complete single PRD now we know the full scope.
Order the deliverables so we don’t have to jump back and forth.

This will form the bases for a NEW readme for Lifeguard as we have realised the previous design that resulted in the project hitting a proverbial brick wall and stalling.



---

Below is the **complete, unified, end-to-end Lifeguard PRD**, fully ordered, exhaustive, and structured so the project can move linearly from foundations → ORM → migrations → advanced Postgres features → tooling → documentation.

This PRD supersedes **all previous designs** and becomes the blueprint for the new Lifeguard README and implementation roadmap.

---

# **LIFEGUARD PRD — Coroutine-Native Database System for Rust (May Runtime)**

**Version: 1.0 — Complete System Design**

---

# **0. Strategic Context**

The original Lifeguard implementation attempted to wrap SeaORM inside a custom coroutine worker and connection pool.
This design failed because:

* SeaORM is async-first and incompatible with the `may` coroutine runtime.
* Async runtime assumptions (Tokio, async traits, Send + Sync types) conflict with coroutine scheduling.
* SeaORM internals are too tightly coupled to async drivers (sqlx, sea-orm connectors).

**Conclusion: Lifeguard must become a standalone coroutine-native DB system**—borrowing SeaQuery and SeaORM migration DSL where appropriate, but without exposing any SeaORM APIs to users.

This PRD defines Lifeguard as an independent stack with:

* **Lifeguard Pool** — coroutine worker-based connection management
* **MayExecutor** — direct Postgres IO via `may_postgres`
* **MayModel / MayRecord** — lightweight ORM layer
* **Lifeguard Migration System** — Postgres schema management
* **Metrics and Observability**
* **Testkit + scaffolding**
* **Extensibility for advanced PostgreSQL features**

---

# **1. Product Overview**

### **1.1 Purpose**

Lifeguard is a **high-throughput, coroutine-native** data access layer for Rust, powered by the `may` stack.
It provides:

* A specialized connection pool designed for coroutines
* Direct Postgres access via `may_postgres`
* A minimal ORM (MayModel & MayRecord)
* SQL building via SeaQuery
* Schema migrations
* Metrics for observability
* Optional advanced PG features (views, triggers, FTS, JSONB, PostGIS, partitioning)

### **1.2 Goals**

* Faster than async ORMs (SeaORM/sqlx) in microservice workloads
* Deterministic scheduling (via `may`)
* Postgres-first (no support for MySQL/SQLite)
* Clean and simple API for application developers
* No SeaORM visible in public APIs
* Support for simple models now and advanced PostgreSQL features later
* Stable foundation for large-scale systems (BRRTRouter, ERP backend, event sourcing)

### **1.3 Non-goals**

* Multi-database compatibility
* Reproducing full ActiveModel change tracking
* Codegen from database schema (future optional)
* SQL query inspection tools (future optional)

---

# **2. Architecture Overview**

### **2.1 Main Components**

```
                 ┌──────────────────────────────┐
                 │        Lifeguard Pool        │
                 │  Coroutine Worker + Manager  │
                 └──────────────┬───────────────┘
                                │
                                ▼
                 ┌──────────────────────────────┐
                 │        MayExecutor           │
                 │  (may_postgres wrapper)      │
                 └──────────────┬───────────────┘
                                │
                ┌───────────────┴────────────────┐
                │         ORM Layer (v1)         │
                │   MayModel + MayRecord + DSL   │
                └─────────────────────────────────┘
                                │
                ┌───────────────┴────────────────┐
                │      SeaQuery SQL Builder      │
                └─────────────────────────────────┘
                                │
                     ┌──────────┴────────────┐
                     │    PostgreSQL Server  │
                     └────────────────────────┘
```

Additional components:

* **Migrations**
* **Metrics Subsystem**
* **Testkit (Postgres + Grafana + Prometheus)**
* **PG Advanced Features (v2/v3)**

---

# **3. Configuration System**

### **3.1 Requirements**

* Load configuration from:

    * `config.toml`
    * env vars (`LIFEGUARD_*`)
    * defaults

* Configure:

    * DB URL
    * pool settings
    * worker concurrency
    * retry policy
    * metrics flags
    * migration config

### **3.2 Example**

```toml
[database]
url = "postgres://user:pass@localhost/db"
max_connections = 20
min_connections = 5

[pool]
retry_count = 3
connection_timeout_ms = 500

[metrics]
prometheus = true
otel = true
```

---

# **4. Connection System — Lifeguard Pool**

### **4.1 Requirements**

* One worker loop per pool
* Each DB operation receives a fresh Postgres connection
* No async runtime
* Fail-fast or retry on connection failure
* Queue separation (future support):

    * coroutine jobs
    * async/Tokio fallback jobs (optional)

### **4.2 Worker Loop**

* Pull next job
* Attempt up to `retry_count` connections
* Track metrics:

    * job latency
    * job failures
    * pool utilization

### **4.3 Public API**

```rust
let pool = LifeguardPool::new(config)?;
pool.execute(|conn| { ... });
```

---

# **5. Executor Layer — MayExecutor**

### **5.1 Requirements**

* Safe, simple, synchronous trait
* Abstraction over `may_postgres`
* Used by:

    * ORM
    * migrations
    * custom raw SQL

### **5.2 Trait**

```rust
trait Executor {
    fn execute(&self, sql: &str, params: &[&(dyn ToSql)]) -> Result<u64>;
    fn query(&self, sql: &str, params: &[&(dyn ToSql)]) -> Result<Vec<Row>>;
}
```

---

# **6. ORM Layer — MayModel / MayRecord**

### **6.1 Requirements**

* Rust-based ORM without async
* Compile-time safe model definitions
* Record-building for mutations
* Relations (v2)
* Complex PG types (v2/v3)

---

## **6.2 MayModel — Read-Only Rows**

```rust
#[derive(MayModel)]
#[table = "users"]
struct User {
    #[primary_key]
    id: i64,
    email: String,
    is_active: bool,
}
```

Generated:

* TABLE constant
* column metadata
* from_row()
* Query interfaces:

    * `find()`
    * `find_by_id()`
    * `filter()`
    * `all()`
* optional relation metadata (v2)

---

## **6.3 MayRecord — Mutations**

```rust
#[derive(MayRecord)]
struct NewUser {
    email: String,
}
```

API:

```rust
let user = NewUser { email: "x".into() }.insert(&pool)?;
```

Updates:

```rust
let updated = user.to_record().set_email("y").update(&pool)?;
```

---

# **7. Query Layer**

### Uses SeaQuery under the hood.

Support:

* select
* update
* delete
* joins
* ordering
* group-by
* aggregates
* subqueries
* raw SQL fallbacks

Later phases add:

* window functions (v2)
* CTEs (v2)
* full-text search DSL (v3)

---

# **8. Migration System — Lifeguard Migrate**

### **8.1 Requirements**

* Build on SeaQuery + SeaORM migration DSL
* Provide Lifeguard CLI
* Track migration versions
* Support safe up/down operations

### **8.2 Migration Trait**

```rust
trait Migration {
    fn up(&self, exec: &impl Executor) -> Result<()>;
    fn down(&self, exec: &impl Executor) -> Result<()>;
    fn name(&self) -> &'static str;
}
```

### **8.3 Features Supported**

* tables (create/alter/drop)
* columns (add/remove/update)
* indexes (unique, partial, gin/gist/hash)
* constraints (FK, check, exclusion)
* sequences
* views
* materialized views
* triggers
* functions/procedures
* partitioning (v3)
* PostGIS types (v3)

---

# **9. Advanced PostgreSQL Support (Phased)**

## **v1.0 — Launch**

* Foreign keys
* Indexes
* Unique constraints
* Composite PKs
* Views
* Basic JSONB
* Basic raw SQL passthrough

## **v2.0 — Enhanced ORM & Schema**

* Relations + relation loading
* Materialized views
* Generated columns
* Check constraints
* Exclusion constraints
* Window functions
* Advanced JSONB operators
* FTS (full-text search)
* CTEs
* Sequences API

## **v3.0 — Enterprise PG Features**

* PostGIS / spatial queries
* Partitioning & sharding
* Stored procedures
* Triggers
* Advanced FTS dictionary support
* Logical replication hooks
* Background refresh of materialized views

---

# **10. Metrics & Observability**

### **10.1 Metrics**

Emit:

* DB connection attempts
* pool idle/in-use gauge
* query latency histogram
* migration duration
* failure counts

Use Prometheus + OTel exporters.

---

# **11. Testkit**

### `docker-compose.test.yml` includes:

* Postgres
* Prometheus
* Grafana
* Loki
* OTel collector

### Test utilities:

* seed-db
* teardown
* snapshot testing
* metrics validation
* load-testing hooks (Goose integration in future)

---

# **12. Developer Tooling**

### CLI:

```
lifeguard migrate up
lifeguard migrate down
lifeguard model generate
lifeguard inspect (future)
```

### Codegen (v3 optional):

* Generate model skeletons from schema
* Generate migration stubs

---

# **13. Public Documentation Structure (New README)**

1. Getting Started
2. What is Lifeguard?
3. Feature Overview
4. Installation
5. Configuration
6. Connection Pool Usage
7. Defining Models
8. CRUD Ops
9. Migrations
10. Advanced Features
11. Metrics
12. Architecture Summary
13. Roadmap

---

# **14. Implementation Roadmap (Ordered)**

### **Phase 1 — Foundation (Week 1–2)**

1. Config system
2. Executor trait
3. Pool + worker refactor
4. Basic metrics
5. Minimal README update

### **Phase 2 — ORM Core (Week 3–5)**

6. `MayModel` macro
7. `MayRecord` macro
8. Query builder integration
9. Basic CRUD support

### **Phase 3 — Migrations (Week 6–7)**

10. Migration trait & runner
11. CLI tooling
12. Implement all v1 PG schema features

### **Phase 4 — v1 Completion (Week 8–10)**

13. Foreign key support
14. Views
15. Indexes
16. JSONB basics
17. Testkit infrastructure
18. Documentation v1

### **Phase 5 — v2 Features (Quarter 2)**

19. Relations & loaders
20. Materialized views
21. Generated columns
22. Exclusion constraints
23. Full-text search
24. Sequences API
25. Advanced query DSL

### **Phase 6 — v3 Enterprise (Quarter 3)**

26. PostGIS
27. Partitioning
28. Triggers
29. Procedures
30. Schema introspection tools
31. Codegen from schema
32. Background jobs (MV refresh)

---

# **15. Success Criteria**

* Zero async dependencies in core runtime
* Seamless integration with BRRTRouter
* SQL throughput surpasses SeaORM/sqlx in benchmarks
* Full PG schema safety via migrations
* Developers can build full applications without touching SeaORM
* Stable public API v1.0 documented and published

---

# **16. Risks & Mitigations**

| Risk                         | Mitigation                                  |
| ---------------------------- | ------------------------------------------- |
| Procedural macros complexity | Keep v1 macros minimal; expand gradually    |
| Schema drift                 | Migrations + inspect tools                  |
| Future need for async        | Optional async executor feature flag        |
| Postgres-specific design     | Lifeguard positions itself as Postgres-only |
| Worker loop contention       | Offer multi-worker pools in v2              |

---

# **This PRD is now ready for README integration and implementation.**

If you want next:

* A **new README.md** generated from this PRD
* Architecture diagrams
* Derive macro designs
* Migration examples
* A scaffolding skeleton for `/src/`

Just tell me what to generate.


---

Each DB operation receives a fresh Postgres connection!

This will be expensive.
Many cloud providers limit the number of active connections and on high throughput this would become the bottleneck with lifeguard blocked waiting for connections to be closed.

Ideally Lifeguard or a microservice would be configured with max connections (incase other micro services are talking to the same DB, this allocation would be an SRE concern).
Within its allocation a microservice’s lifeguard would need to open up all connections and then actively reuse them in the most aggressive and efficient way.

---

You are **100% correct**, and this changes the architecture of Lifeguard in a *fundamental* way.

Opening a new Postgres connection **per DB operation** is absolutely not sustainable:

* Opening a Postgres TCP connection is expensive
* Providers like RDS, Cloud SQL, Neon, Supabase cap connections (200–500 max)
* Under load, constantly opening/closing connections thrashes CPU, kernel buffers, auth overhead
* DB becomes bottleneck
* Lifeguard worker will stall waiting for connection creation
* Eventually the service deadlocks during traffic spikes

You’ve identified a **real-world operational constraint**:
**Connection reuse is mandatory.**
**Connections must be pre-allocated.**
**The pool must own and manage long-lived connections until explicitly dropped.**

This is *exactly* the reason pgbouncer, r2d2, postgres.async pools exist.

So we must rewrite the PRD section to reflect:

👉 Lifeguard must have a **true connection pool**, not a "connection per job".

---

# ✔ Corrected Lifeguard Architecture

## **Lifeguard MUST maintain a pool of long-lived Postgres connections**

### Why?

1. Cloud providers **limit total active connections** per project.
2. Creating a new connection can take **2–50 ms** depending on networking.
3. Under high throughput, connection churn becomes the bottleneck.
4. Postgres is optimized for **persistent backend processes**, not constant churn.
5. Modern microservices must coordinate their connection footprint with SRE policies.

### Therefore:

### **Lifeguard must:**

* Pre-open N connections on startup
* Keep them alive until shutdown
* Reuse them aggressively
* Refresh or recreate connections only when broken
* Enforce a strict max connection limit configured by SREs
* Queue inbound requests when no connections are available
* Provide metrics so SREs can tune pool sizing

---

# ✔ Updated Pool Design

## **LifeguardPool Structure**

```
LifeguardPool
   ├── Vec<PGConnectionSlot> (size = max_connections)
   ├── Semaphore / channel (available connection counter)
   ├── Worker tasks (optional)
   ├── Retry + backoff policy
   ├── Metrics integration
```

A `PGConnectionSlot` is:

```rust
struct ConnectionSlot {
    id: usize,
    conn: may_postgres::Client,
    in_use: AtomicBool,
    last_used: Instant,
}
```

### Acquire operation:

1. Wait on a semaphore (bounded by pool size)
2. Select a free connection slot
3. Mark it as `in_use = true`
4. Return it

### Release operation:

1. Mark slot as `in_use = false`
2. Push token back into semaphore
3. Update usage metrics

### If connection is broken:

* Try to reconnect
* If reconnect fails and retry count exceeded → mark pool degraded and surface metrics
* Optionally shed load (429) if no healthy connections remain

---

# ✔ Lifeguard Pool Behavior

### **On Startup**

* Create all connections (`max_connections`)
* Eager connection creation avoids latency spikes during load
* If some fail to initialize → retry
* If below a threshold (<50%) → startup may fail or warn depending on config

---

# ✔ Why this aligns with your entire ecosystem

### BRRTRouter

* Wants deterministic execution
* Wants predictable DB latency
* Cannot afford connection churn in hot path

### ERP backend

* Needs consistent throughput
* Must avoid DB overload or connection exhaustion under load

### SRE policies

* Connections per service must be **bounded and controlled**
* Pool must expose metrics for `pgbouncer`, RDS, Cloud SQL dashboards

### Mayfa / coroutine architecture

* Coroutines naturally cooperate on shared objects
* Connection reuse plays well with coroutine scheduling

---

# ✔ Updated Worker Model (Corrected)

Previously:

* 1 worker per job establishing connection (incorrect)

Now:

* N long-lived Postgres connections
* Jobs borrow a connection
* Worker loops manage job execution using shared pool state

Correct architecture:

```
       ┌───────────────────────────────┐
       │ LifeguardPool                 │
       │ max_connections = 32          │
       │ semaphore: 32 tokens          │
       └───────────┬───────────────────┘
                   │ acquire
                   ▼
       ┌───────────────────────────────┐
       │ ConnectionSlot[i]             │
       │ in_use = true                 │
       │ conn: may_postgres::Client    │
       └───────────┬───────────────────┘
                   │ run query
                   ▼
       ┌───────────────────────────────┐
       │ Release                       │
       │ in_use = false                │
       │ semaphore += 1                │
       └───────────────────────────────┘
```

---

# ✔ Updated PRD Section (Drop-In Replacement)

Below is the corrected section replacing the earlier flawed design.

---

## **4. Connection System — Lifeguard Pool (Corrected Architecture)**

### **4.1 Requirements**

* **Maintain a fixed-size pool of persistent Postgres connections**
* Max connections = SRE-defined limit
* Min connections = optional warm pool
* Pre-allocate all connections on startup
* Reuse connections aggressively
* enforce max-acquire timeout
* detect broken connections and reinitialize
* expose pool metrics (health, usage, stalls)
* optionally support connection warmers & refresh policies

### **4.2 Rationale**

* Creating connections per request is too expensive
* Cloud DBs rate-limit active connections
* High traffic requires predictable performance
* Pool must avoid DB churn and exhaustion

### **4.3 Behavior**

#### On startup:

* Immediately open `max_connections`
* Validate connection health
* If <50% succeed, fail startup
* Expose “pool_ready” metric

#### On query:

1. Acquire semaphore token
2. Find a free connection slot
3. Mark slot as `in_use`
4. Run query via `MayExecutor`
5. Release slot

#### On error:

* Attempt reconnection
* If fails, mark slot unhealthy
* Expose metrics
* If too many slots unhealthy → mark pool degraded

#### On shutdown:

* Gracefully close connections

### **4.4 Public API**

```rust
let pool = LifeguardPool::new(config)?;
pool.acquire(|conn| {
    conn.query("SELECT 1", &[])
});
```

OR more ergonomic:

```rust
let rows = pool.query(User::find().build_query())?;
```

---

# ✔ We should update the PRD & README accordingly

I can now generate:

### ✅ Updated complete PRD (merged with corrected connection design)

### ✅ Updated architecture diagrams

### ✅ Updated README.md reflecting the corrected pool semantics

### ✅ Updated system overview narrative

Just tell me:

**“Generate the corrected full PRD.”**

---

Generate the corrected full PRD.

---

Below is the **corrected, unified, end-to-end Lifeguard PRD**, incorporating the **proper connection pool architecture** (persistent pooled connections, NOT re-establishing connections per job).

This document replaces all earlier drafts and becomes the **authoritative system specification** for the next generation of Lifeguard.

---

# **LIFEGUARD PRD — Coroutine-Native Postgres Data Platform for Rust**

**Version: 2.0 — Corrected Architecture**

---

# **0. Context & Motivation**

The initial Lifeguard concept attempted to embed SeaORM inside a coroutine-based system using `may`. This failed due to:

* SeaORM’s async-only architecture (Tokio-dependent)
* Incompatible executor assumptions
* Heavy runtime overhead from async futures
* Inability to integrate with `may_postgres` cleanly
* Blockers around connection concurrency, worker design, schema evolution

**Lifeguard now evolves into a first-class, coroutine-native Postgres data platform** that:

* Uses `may_postgres` for I/O
* Uses a **true connection pool** with **persistent connections**
* Implements its own ORM (`MayModel` / `MayRecord`)
* Uses SeaQuery for SQL generation
* Uses a Lifeguard-native Migration system
* Supports advanced PostgreSQL features
* Exposes clean, simple, stable APIs

This PRD defines the **complete system scope**, **ordered roadmap**, and the **foundation for the new README**.

---

# **1. Product Overview**

### **1.1 What is Lifeguard?**

Lifeguard is a **high-performance, coroutine-native Postgres access layer** designed for Rust services running on the `may` runtime.

It provides:

* A **deterministic connection pool**
* A fast ORM-like layer: `MayModel` and `MayRecord`
* SeaQuery-based SQL building
* A robust migration system
* Postgres feature support (views, FKs, JSONB, sequences, etc.)
* Observability & metrics
* Strong API stability with zero SeaORM exposure

### **1.2 What makes Lifeguard unique**

* Built for **coroutines**, not async
* Predictable **worker and connection management**
* Extremely low overhead
* Tight integration with BRRTRouter / ERP backend / microservice workloads
* Postgres-first design: no compromises for other databases

---

# **2. High-Level Goals**

1. **Remove all SeaORM user-facing dependencies**
2. Provide a **robust, safe, persistent connection pool**
3. Expose a simple ORM API (`MayModel`, `MayRecord`)
4. Provide schema migrations that work with Postgres natively
5. Deliver advanced PG features in structured phases
6. Integrate deeply with Prometheus, OTel, Grafana
7. Offer excellent tooling and developer experience
8. Guarantee predictable performance under load

---

# **3. Scope Definition**

### **3.1 In Scope (v1 → v3)**

* Persistent pooled Postgres connections
* Coroutine-native execution
* Query building (SeaQuery)
* ORM layer for CRUD
* Migrations
* Foreign keys
* Views + materialized views
* JSONB + full-text search
* Sequences
* Indexes (unique, partial, gin/gist, composite)
* Constraints
* Generated columns
* Triggers & functions
* PostGIS (v3)
* Partitioning (v3)
* Observability
* Tooling (CLI, testkit, codegen v3)

### **3.2 Out of Scope**

* Multi-database support
* Async runtime (`tokio`) support (optional future feature flag)
* SQL parsing / schema introspection (future optional)

---

# **4. System Architecture**

```
                 ┌──────────────────────────────┐
                 │       Lifeguard Pool         │
                 │  Persistent Connection Slots │
                 │  Bounded Semaphore (Concurrency)│
                 └──────────────┬───────────────┘
                                │ acquire
                                ▼
                 ┌──────────────────────────────┐
                 │         MayExecutor          │
                 │ (may_postgres wrapper + I/O) │
                 └──────────────┬───────────────┘
                                │
                ┌───────────────┴────────────────┐
                │         ORM Layer               │
                │   MayModel + MayRecord + DSL   │
                └─────────────────────────────────┘
                                │
                ┌───────────────┴────────────────┐
                │      SeaQuery SQL Builder      │
                └─────────────────────────────────┘
                                │
                      ┌─────────┴─────────┐
                      │   PostgreSQL       │
                      └────────────────────┘
```

---

# **5. Configuration System**

### Requirements:

* TOML + ENV + defaults
* Allow SREs to configure database resource usage precisely
* Prevent services from exceeding allowed DB connections

### Example:

```toml
[database]
url = "postgres://app:pass@localhost/mydb"
max_connections = 32
min_connections = 8
connection_timeout_ms = 1000
acquire_timeout_ms = 5000

[pool]
retry_count = 3

[metrics]
prometheus = true
otel = true
```

### Behavior:

* max_connections = hard cap
* At startup, Lifeguard MUST pre-open *all* connections
* If too few connections initialize → error or degraded mode

---

# **6. Lifeguard Pool — Correct Architecture**

## **Core Requirements**

1. **Persistent connections**
2. **Aggressive connection reuse**
3. **No creation during hot path**
4. **Bounded concurrency**
5. **Semaphore to control acquisition**
6. **Connection health monitoring**
7. **Reconnection policy on failure**

## **Connection Slot Structure**

```rust
struct ConnectionSlot {
    id: usize,
    conn: may_postgres::Client,
    in_use: AtomicBool,
    last_used: Instant,
}
```

### Acquire Flow:

1. Acquire semaphore token
2. Find free slot
3. Set in_use = true
4. Hand slot to requester

### Release Flow:

1. Mark slot free
2. Return semaphore token

### Failure Handling:

* reconnect slot
* if fails repeatedly → mark unhealthy
* surface pool health metrics

---

# **7. Executor Layer — MayExecutor**

Simple trait backing ORM & migrations.

```rust
trait Executor {
    fn execute(&self, sql: &str, params: &[&(dyn ToSql)]) -> Result<u64>;
    fn query(&self, sql: &str, params: &[&(dyn ToSql)]) -> Result<Vec<Row>>;
}
```

---

# **8. ORM Layer**

Provides ergonomic, synchronous, coroutine-friendly database access.

## **8.1 MayModel (DB Row Representation)**

```rust
#[derive(MayModel)]
#[table = "users"]
struct User {
    #[primary_key]
    id: i64,
    email: String,
    is_active: bool,
}
```

Generated:

* Metadata: table, columns
* from_row(row: &Row)
* Queries:

    * `find()`
    * `find_by_id()`
    * `filter()`
    * `all()`

---

## **8.2 MayRecord (Insert/Update Builder)**

```rust
#[derive(MayRecord)]
struct NewUser {
    email: String,
}
```

Generated:

```rust
let u = NewUser { email: "x".into() }.insert(&pool)?;
```

For updates:

```rust
user.to_record().set_email("new").update(&pool)?;
```

---

# **9. Query Layer**

Backed by SeaQuery.

### Must support (v1):

* SELECT
* UPDATE
* DELETE
* INSERT
* filtering
* limit / offset
* ordering
* joins
* functions (COUNT, SUM, etc.)
* raw SQL escape hatch

### v2:

* window functions
* CTEs
* full-text search
* advanced JSONB operations

---

# **10. Migration System — Lifeguard Migrate**

A migration DSL independent of SeaORM but similar in style.

## Migration Trait:

```rust
trait Migration {
    fn up(&self, exec: &impl Executor) -> Result<()>;
    fn down(&self, exec: &impl Executor) -> Result<()>;
    fn name(&self) -> &'static str;
}
```

## Features Supported:

### v1:

* Tables (create/alter/drop)
* Columns
* Indexes (unique, partial, composite)
* Foreign keys
* Views
* JSONB

### v2:

* Materialized views
* Generated columns
* Sequences
* Check constraints
* Exclusion constraints

### v3:

* Triggers
* Functions/procedures
* Partitioning
* PostGIS

CLI:

```
lifeguard migrate up
lifeguard migrate down
lifeguard migrate status
```

---

# **11. Advanced PostgreSQL Feature Roadmap**

## v1 — Launch Foundation

* Foreign keys
* Views
* JSONB (basic)
* Index types
* Sequence support

## v2 — Enhanced ORM & Schema

* Relation loading
* Materialized views (refresh)
* Generated columns
* Check & exclusion constraints
* FTS (full-text search)
* Window functions
* Sequences API
* Advanced JSONB querying

## v3 — Enterprise Extensions

* PostGIS
* Partitioning (RANGE/HASH)
* Stored procedures
* Triggers
* Logical replication hooks
* Codegen from DB schema
* Model inspector CLI

---

# **12. Metrics & Observability**

### Must expose:

* number of open connections
* connection failures
* reconnections
* query latency histogram
* pool acquire latency
* job queue depth
* migration duration

### Integrations:

* Prometheus exporter
* OTel spans & metrics

---

# **13. Testkit**

### `docker-compose.test.yml`:

* Postgres
* Prometheus
* Grafana
* Loki
* OTel collector

### Provide:

* seed functions
* snapshot testing
* performance benchmarks
* pool stress tester

---

# **14. Tooling and CLI**

### Commands:

```
lifeguard migrate up
lifeguard migrate down
lifeguard migrate status
lifeguard inspect     (v3)
lifeguard model new   (v3)
```

### Test scaffolding:

```rust
let pool = testkit::test_pool();
testkit::seed(&pool)?;
```

---

# **15. Documentation Plan (New README)**

1. Introduction
2. Why Lifeguard exists
3. Architecture overview
4. Getting started
5. Configuration
6. Using the connection pool
7. Defining Models and Records
8. CRUD examples
9. Migrations
10. Advanced topics
11. Observability
12. Future roadmap

---

# **16. Implementation Roadmap (Ordered)**

## **Phase 1 — Core Infrastructure (Weeks 1–3)**

1. Config loader
2. Persistent LifeguardPool
3. MayExecutor
4. Metrics foundations
5. Basic error handling

## **Phase 2 — ORM v1 (Weeks 3–6)**

6. MayModel derive macro
7. MayRecord derive macro
8. CRUD operations
9. Query builder glue (SeaQuery)

## **Phase 3 — Migrations (Weeks 6–8)**

10. Migration trait
11. Migration runner
12. CLI integration
13. Support for all v1 schema operations

## **Phase 4 — v1 Release Prep (Weeks 8–10)**

14. Documentation
15. Testkit
16. Benchmarks
17. Integration tests with BRRTRouter

## **Phase 5 — Advanced ORM & PG Features (Quarter 2)**

18. Relation loading
19. Materialized views
20. Generated columns
21. Sequences API
22. JSONB / FTS
23. Exclusion constraints

## **Phase 6 — Enterprise Extensions (Quarter 3)**

24. Partitioning
25. PostGIS
26. Triggers
27. Stored procedures
28. Inspect/Codegen tooling

---

# **17. Success Criteria**

* Zero async runtime dependencies in core
* High-throughput query performance
* Pool reuse prevents DB overload
* No SeaORM types in public API
* Developer experience is simpler than SeaORM/sqlx
* Smooth integration with BRRTRouter
* Clean migration story for all PostgreSQL features

---

# **18. Risks & Mitigations**

| Risk                | Mitigation                                  |
| ------------------- | ------------------------------------------- |
| Macro complexity    | Start with minimal features, iterate safely |
| Connection churn    | Persistent pool resolves this               |
| Schema drift        | Migration system + inspect (v3)             |
| Deadlock under load | Semaphore + enforce acquire timeout         |
| Broken connections  | Auto-reconnect logic + metrics              |

---

# **PRD Complete — Ready for README + Implementation.**

If you want, I can now generate:

### → **The new README.md**

### → Full architecture diagrams (ASCII or rendered images)

### → The initial scaffolding for `/src/`

### → Derive macro designs (MayModel / MayRecord)

### → Migration examples

Just tell me what to generate next.


