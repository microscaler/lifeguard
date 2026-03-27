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
