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

---

The new README.md

---

Below is the **new, corrected, complete `README.md`** for the Lifeguard project.
This README is aligned with the final PRD and suitable for immediate inclusion in the GitHub repo as the new canonical documentation.

---

# **Lifeguard**

### *Coroutine-native Postgres Data Platform for Rust*

Lifeguard is a **high-performance, coroutine-native database layer** designed for Rust services built on the [`may`](https://github.com/Xudong-Huang/may) runtime.

It provides:

* A **deterministic, persistent Postgres connection pool**
* A lightweight ORM: **MayModel** and **MayRecord**
* SQL building powered by **SeaQuery**
* A robust **migration system**
* Deep **observability** (Prometheus + OpenTelemetry)
* Support for core → advanced PostgreSQL features

Lifeguard is designed to power **high-throughput microservices**, **API routers**, and **database-heavy systems** with predictable latency and zero async runtime overhead.

---

# **Why Lifeguard?**

Traditional Rust database libraries (sqlx, SeaORM, Diesel):

* rely heavily on **async**, **Tokio**, and **Send/Sync** constraints
* assume a **future-polling execution model**
* are not compatible with **coroutine runtimes** like `may`
* create complexity and performance overhead in low-latency systems

**Lifeguard takes a different approach.**

It is:

### ✔ Coroutine-first

### ✔ Postgres-focused

### ✔ Built for extreme throughput

### ✔ Designed for predictable performance

### ✔ Fully synchronous internally

If you are building microservices or routers using `may`, Axum-with-may, or BRRTRouter—
Lifeguard is the database foundation you actually need.

---

# **Features**

### **✓ Persistent Postgres Connection Pool**

* Pre-allocates `N` long-lived connections
* Reuses them aggressively
* Avoids DB churn & connection storms
* Ensures bounded concurrency via semaphore
* Auto-recovers from bad connections

### **✓ ORM-lite Layer**

* `#[derive(MayModel)]` for reading data
* `#[derive(MayRecord)]` for inserts/updates
* CRUD helpers
* SeaQuery-backed SQL generation

### **✓ Safe, Simple Migrations**

* Lifeguard-native migration trait
* Safe up/down versioning
* Supports full Postgres schema operations:

    * tables, indexes, FKs, views, sequences, JSONB, etc.

### **✓ Metrics & Instrumentation**

* Connection pool gauges
* Query latency histograms
* Error counters
* Optional OTel spans and metrics

### **✓ PostgreSQL Power Features**

Supported now or coming soon:

| Feature               | Status |
| --------------------- | ------ |
| Foreign Keys          | ✓ v1   |
| Views                 | ✓ v1   |
| JSONB                 | ✓ v1   |
| Partial Indexes       | ✓ v1   |
| Materialized Views    | v2     |
| Generated Columns     | v2     |
| Exclusion Constraints | v2     |
| Full-text Search      | v2     |
| Window Functions      | v2     |
| PostGIS               | v3     |
| Partitioning          | v3     |
| Triggers & Procedures | v3     |

---

# **Quick Start**

## **1. Add Lifeguard**

```toml
[dependencies]
lifeguard = { git = "https://github.com/microscaler/lifeguard" }
```

## **2. Configure your database**

```toml
# config.toml
[database]
url = "postgres://app:pass@localhost/mydb"
max_connections = 32
min_connections = 8
```

Load config:

```rust
let config = LifeguardConfig::load()?;
let pool = LifeguardPool::new(config)?;
```

---

# **3. Define a Model**

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

Query:

```rust
let user = User::find_by_id(&pool, 1)?;
let users = User::find().filter(User::Email.eq("test")).all(&pool)?;
```

---

# **4. Insert or Update Records**

```rust
#[derive(MayRecord)]
#[table = "users"]
struct NewUser {
    email: String,
}

let created = NewUser { email: "first@demo.com".into() }.insert(&pool)?;
```

Update existing rows:

```rust
let updated = created
    .to_record()
    .set_email("new@demo.com")
    .update(&pool)?;
```

---

# **5. Migrations**

Create a migration:

```rust
pub struct CreateUsers;

impl Migration for CreateUsers {
    fn up(&self, exec: &impl Executor) -> Result<()> {
        let stmt = Table::create()
            .table(Alias::new("users"))
            .if_not_exists()
            .col(ColumnDef::new(Alias::new("id")).big_integer().not_null().primary_key())
            .col(ColumnDef::new(Alias::new("email")).string().not_null())
            .to_owned();

        exec.execute(&stmt.to_string(PostgresQueryBuilder), &[])?;
        Ok(())
    }

    fn down(&self, exec: &impl Executor) -> Result<()> {
        exec.execute("DROP TABLE IF EXISTS users", &[])?;
        Ok(())
    }

    fn name(&self) -> &'static str {
        "20250101_create_users"
    }
}
```

Run migration:

```
lifeguard migrate up
```

---

# **6. Architecture Summary**

```
LifeguardPool  → Persistent connection slots
MayExecutor    → Direct may_postgres I/O
MayModel       → Select/query
MayRecord      → Insert/update/delete
SeaQuery       → SQL builder
Migrations     → Schema evolution
Metrics        → Prometheus + OTel
Postgres       → The engine powering it all
```

Everything above `may_postgres` is fully synchronous and coroutine-native.

---

# **Roadmap**

### **v1.0 (Current Work)**

* Persistent pool + executor
* MayModel / MayRecord
* CRUD operations
* Migration system
* Metrics + Testkit
* Support: FKs, views, indexes, JSONB

### **v2.0**

* Relation loading
* Materialized views
* Sequences API
* Generated columns
* Full-text search
* Window functions

### **v3.0**

* PostGIS
* Partitioning
* Triggers & stored procedures
* Schema inspector
* Codegen tools

---

# **Philosophy**

Lifeguard is not trying to be SeaORM or Diesel.
It is built for:

* **Deterministic performance**
* **High throughput**
* **Coroutine style concurrency**
* **Deep Postgres feature support**

If you want async, choose sqlx.
If you want “ActiveRecord”, choose SeaORM.
If you want **raw power with coroutine speed**, choose **Lifeguard**.

---

# **Contributing**

PRs are welcome once the v1 API is stabilized.

Key areas for contribution:

* ORM macro improvements
* Migration DSL extensions
* Postgres feature support
* Metrics exporters
* Testkit enhancements

---

# **License**

MIT.

---

# **Next Steps**

If you'd like, I can now generate:

### ✓ `CONTRIBUTING.md`

### ✓ `/src/` scaffolding code

### ✓ Architecture diagrams (SVG + ASCII)

### ✓ Examples directory

### ✓ Template migrations

### ✓ Issue templates + GitHub Actions CI config

Just tell me what you want next.


