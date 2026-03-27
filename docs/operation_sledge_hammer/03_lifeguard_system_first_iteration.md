We need a comprehensive system design and PRD for all components we need to build into Lifeguard to make it its own standing system (but beg/borrow/steal) where needed and build new where necessary.

The idea will be that a user of LifeGuard will not need to know anything about SeaORM.

Anything we borrow from SeaORM will be implicit under the hood and fully facaded.


---

Alright, let’s turn Lifeguard into its own **coroutine-native data access system**, with SeaORM fully buried and facaded.

Below is a **combined system design + PRD** you can basically drop into `docs/PRDs/lifeguard_system_PRD.md` and iterate from there.

I’ll structure it as:

1. Product overview & goals
2. High-level architecture
3. Detailed component design
4. Beg/Borrow/Steal plan (what we reuse vs build)
5. Public API surface (what the *user* of Lifeguard sees)
6. Phased implementation roadmap

---

## 1. Product Overview

### 1.1 Purpose

Lifeguard is a **coroutine-native Postgres access and pooling system** for Rust, built on the `may` runtime, providing:

* Connection pooling
* A lightweight ORM layer (`MayModel` / `MayRecord`)
* Migration tooling
* Metrics & observability
* Batched operations and stress-test tooling

Users of Lifeguard:

* **Must not know** SeaORM exists or ever existed.
* **Should never see** SeaORM types, traits, or error messages.
* Interact only with **Lifeguard primitives**: pool, models, records, migrations, metrics.

### 1.2 Problem Statement

Current Lifeguard is:

* Too tightly coupled to **SeaORM** (async-first, Tokio ecosystem).
* Misaligned with **`may` + `may_postgres`** coroutine architecture.
* Hard to reason about performance: async overhead, double indirections, limited control over IO.

We want a **standalone system** that:

* Speaks Postgres directly via `may_postgres`.
* Uses SeaQuery / SeaORM migration mechanics under the hood where it makes sense.
* Exposes a coherent, small, **Lifeguard-native** API.

---

## 2. High-Level Goals & Non-Goals

### 2.1 Goals

1. **SeaORM-free public API**

    * No `SeaOrmError`, `DatabaseConnection`, `EntityTrait` in user code.
    * All dependencies on SeaORM (if any remain) are hidden behind Lifeguard-branded modules.

2. **Coroutine-native DB stack**

    * Use `may_postgres` for all runtime DB IO.
    * No Tokio / async/await in core execution paths.

3. **Lightweight ORM layer**

    * `#[derive(MayModel)]` for row structs.
    * `#[derive(MayRecord)]` for “change sets” (insert/update).
    * SeaQuery as the SQL builder.

4. **Migration system**

    * Borrow the SeaORM migration DSL/approach where useful.
    * Provide a Lifeguard-branded CLI and API: `lifeguard migrate up/down/status`.

5. **Performance & Reliability**

    * Batched inserts (e.g. 500 rows per query).
    * Connection retry policies.
    * Metrics via Prometheus + OpenTelemetry hooks.
    * Tunable pool and worker configuration (`config.toml` + environment).

6. **DX & Testability**

    * Test harness (`docker-compose.test.yml`) with Postgres + Grafana + Prometheus + Loki + OTel (you already planned this).
    * Clear `README` and examples.

### 2.2 Non-Goals (for now)

* Multi-database support (MySQL, SQLite etc.) → **Postgres only**.
* Complex ActiveRecord-style change tracking like SeaORM.
* Graph-based relation loading and ORMs for every use case.
* Codegen from schema (nice-to-have later; not v1).

---

## 3. System Architecture

### 3.1 Top-Level Modules

Proposed crate layout:

```text
src/
  lib.rs
  config.rs
  pool/
    mod.rs
    manager.rs
    worker.rs
  db/
    executor.rs
    row.rs
    error.rs
  orm/
    mod.rs
    model.rs        // MayModel
    record.rs       // MayRecord
    macros.rs       // derive(MayModel/MayRecord)
    query.rs        // find/find_by_id/filter wrappers
  migrate/
    mod.rs
    runner.rs
    migration_trait.rs
  metrics/
    mod.rs
    prometheus.rs
    otel.rs
  testkit/
    mod.rs
    seeds.rs
    harness.rs
```

### 3.2 Data Flow (happy path)

1. User configures and initializes a **Lifeguard pool**:

   ```rust
   let pool = LifeguardPool::from_config("config.toml")?;
   ```

2. User defines models/records:

   ```rust
   #[derive(MayModel)]
   struct User {
       id: i64,
       email: String,
       is_active: bool,
   }

   #[derive(MayRecord)]
   struct NewUser {
       email: String,
   }
   ```

3. Insert flow:

   ```rust
   let user = NewUser { email: "x@y.com".into() }
       .insert(&pool)?;
   ```

    * `MayRecord::insert` builds SQL via SeaQuery.
    * Delegates to `Executor` which uses a Lifeguard connection from the pool.
    * `may_postgres` executes query.
    * Row is mapped to `MayModel::from_row`.

4. Query flow:

   ```rust
   let user = User::find_by_id(&pool, id)?;
   let users = User::find().filter(User::email().eq("test")).all(&pool)?;
   ```

5. Migrations:

   ```bash
   lifeguard migrate up
   ```

    * Migrations are defined via a Lifeguard `Migration` trait.
    * Under the hood, use SeaQuery/SeaORM-like DSL and run using the same `Executor`.

---

## 4. Detailed Component Design

### 4.1 Configuration System

**Requirements**

* Support `config.toml` + environment overrides + defaults (you already planned this).
* Expose sane defaults for:

    * pool size, min/max connections
    * connection timeout, acquire timeout
    * retry counts
    * metrics endpoints

**Design**

```rust
pub struct LifeguardConfig {
    pub database_url: String,
    pub max_connections: usize,
    pub min_connections: usize,
    pub connection_timeout_ms: u64,
    pub acquire_timeout_ms: u64,
    pub retry_count: u8,
    // metrics flags, etc.
}
```

Load order:

1. `config.toml` (optional)
2. Environment variables (`LIFEGUARD_*`)
3. Hardcoded defaults

Exposed API:

```rust
impl LifeguardConfig {
    pub fn load() -> Result<Self>;
}
```

---

### 4.2 Connection Pool & Worker Model

You *already* have design ideas here; let’s formalize.

**Requirements**

* Single worker per `DbPoolManager` handling all coroutine jobs (per your earlier design).
* Each job gets a **fresh** `may_postgres::Client` (no persistent per-job connection).
* Worker retries connection up to N times.
* Separate queues for:

    * coroutine jobs
    * (future) async jobs (if you ever reintroduce them)

**Core types**

```rust
pub struct LifeguardPool {
    manager: DbPoolManager,
}

pub struct DbPoolManager {
    // internal queues, metrics, config
}

pub struct DbJob {
    query: DbOperation,
    // callback or channel for result
}
```

Worker loop:

```rust
fn run_worker_loop(manager: Arc<DbPoolManager>) {
    loop {
        let job = manager.next_job();
        let mut attempts = 0;

        while attempts < manager.config.retry_count {
            match manager.get_connection() {
                Ok(conn) => {
                    let res = job.execute_with(&conn);
                    manager.return_result(job, res);
                    break;
                }
                Err(err) => {
                    attempts += 1;
                    if attempts == manager.config.retry_count {
                        manager.return_error(job, err);
                    }
                }
            }
        }
    }
}
```

All ORM operations use this pool under the hood, but the **user-facing API appears synchronous**.

---

### 4.3 DB Executor Layer

This is where you **talk directly to `may_postgres`**.

**Goal**: one trait the entire ORM & migrations depend on, with a `may_postgres` implementation.

```rust
pub trait Executor {
    fn execute(&self, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64, DbError>;
    fn query(&self, sql: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>, DbError>;
}
```

Implementation for pool:

```rust
impl Executor for LifeguardPool {
    fn execute(...) -> Result<u64, DbError> { ... }
    fn query(...) -> Result<Vec<Row>, DbError> { ... }
}
```

No SeaORM types here. This is the **primary seam** that keeps Lifeguard independent.

---

### 4.4 ORM Layer: MayModel & MayRecord

#### 4.4.1 MayModel (read-only row)

**Purpose**

* Represent a database row as an immutable struct.
* Provide static metadata: table name, columns.
* Provide mapping from `Row` to `Self`.
* Expose ergonomic query helpers.

**Example**

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

Macro generates:

```rust
impl User {
    pub const TABLE: &'static str = "users";

    pub fn find() -> QueryBuilder<Self> { ... }
    pub fn find_by_id(executor: &impl Executor, id: i64) -> Result<Self, DbError> { ... }

    fn from_row(row: &Row) -> Result<Self, DbError> { ... }
}
```

Under the hood:

* Use SeaQuery to build SQL.
* Use `Executor::query` to run it.
* Map each row via `from_row`.

#### 4.4.2 MayRecord (change set)

**Purpose**

* Represent mutations (insert/update) separately.
* Avoid mixing “DB state” with “pending changes”.
* Keep API simple and predictable.

**Example**

```rust
#[derive(MayRecord)]
#[table = "users"]
struct NewUser {
    email: String,
}
```

Generated API:

```rust
impl NewUser {
    pub fn insert(self, executor: &impl Executor) -> Result<User, DbError> { ... }
}
```

For updates:

```rust
#[derive(MayRecord)]
#[table = "users"]
struct UserRecord {
    #[primary_key]
    id: i64,
    email: Option<String>,
    is_active: Option<bool>,
}

impl User {
    pub fn to_record(&self) -> UserRecord { ... }
}

// usage:
user.to_record()
    .set_email(Some("new@mail.com".into()))
    .update(&pool)?;
```

ORM must **never** expose SeaORM `Model`/`ActiveModel` types. Everything routes through `MayModel` / `MayRecord` and `Executor`.

---

### 4.5 Migration System

Goal: **borrow SeaORM’s migration mechanics**, branded and wired through Lifeguard’s Executor.

**User-facing API:**

```rust
pub trait Migration {
    fn up(&self, exec: &impl Executor) -> Result<(), DbError>;
    fn down(&self, exec: &impl Executor) -> Result<(), DbError>;
    fn name(&self) -> &'static str;
}
```

Migration authoring:

```rust
pub struct CreateUsersTable;

impl Migration for CreateUsersTable {
    fn up(&self, exec: &impl Executor) -> Result<(), DbError> {
        let stmt = Table::create()
            .table(Alias::new("users"))
            .if_not_exists()
            .col(ColumnDef::new(Alias::new("id")).big_integer().not_null().primary_key())
            .col(ColumnDef::new(Alias::new("email")).string().not_null())
            .to_string(PostgresQueryBuilder);

        exec.execute(&stmt, &[])?;
        Ok(())
    }

    fn down(&self, exec: &impl Executor) -> Result<(), DbError> { ... }
    fn name(&self) -> &'static str { "m0001_create_users_table" }
}
```

A `Migrator` aggregates migrations:

```rust
pub struct Migrator;

impl Migrator {
    pub fn migrations() -> Vec<Box<dyn Migration>> { ... }

    pub fn up(exec: &impl Executor) -> Result<(), DbError> { ... }
    pub fn down(exec: &impl Executor) -> Result<(), DbError> { ... }
}
```

CLI:

```bash
lifeguard migrate up
lifeguard migrate down
lifeguard migrate status
```

Under the hood you can:

* Reuse SeaORM’s migration crate or patterns for version tracking and schema table structure.
* But **all exposed types are Lifeguard-native**.

---

### 4.6 Metrics and Observability

**Requirements**

* Expose metrics for:

    * pool size, in-use, idle
    * job queue depth
    * query latency, grouped by model/table
    * error counts by type
* Integrate with Prometheus and OpenTelemetry.

**Design**

* Use a `metrics` module with feature-flags: `prometheus`, `otel`.
* Wrap executor calls:

```rust
fn instrumented_execute(...) {
    let start = Instant::now();
    let result = inner_execute(...);
    metrics::record("lifeguard_query_latency_ms", start.elapsed());
    // increment success/failure counters
    result
}
```

---

### 4.7 Testkit / Developer Experience

Provide:

* `docker-compose.test.yml` that spins up:

    * Postgres
    * Prometheus
    * Grafana
    * Loki
    * OTel collector (optional)

* Helper to seed DB:

```rust
pub fn seed_test_db(pool: &LifeguardPool) -> Result<(), DbError> { ... }
```

* Example tests:

```rust
#[test]
fn test_insert_and_query_user() {
    let pool = testkit::test_pool();
    testkit::seed_test_db(&pool).unwrap();

    let user = NewUser { email: "test@test.com".into() }
        .insert(&pool)
        .unwrap();

    let fetched = User::find_by_id(&pool, user.id).unwrap();
    assert_eq!(fetched.email, "test@test.com");
}
```

---

## 5. Beg / Borrow / Steal Plan

### Borrow / Reuse

* **SeaQuery**

    * SQL builder.
    * Column/table definitions and DSL.
* **SeaORM migration patterns**

    * Migration table schema.
    * Versioning approach.
    * SeaQuery patterns for migrations.

### Wrap / Facade

* SeaORM migration crate (if you choose to depend on it):

    * Wrap everything under `lifeguard::migrate::*`.
    * Never expose SeaORM types in public API.

### Build New

* `MayModel` / `MayRecord` macros and runtime.
* Lifeguard-specific connection pool on `may`.
* Lifeguard `Executor` abstraction.
* Metrics integration.
* Testkit & examples.

---

## 6. Public API Surface Summary

When someone adds Lifeguard as a dependency, they should see roughly:

```rust
use lifeguard::{
    LifeguardConfig,
    LifeguardPool,
    Executor,
    MayModel,
    MayRecord,
    migrate::Migrator,
};
```

And in their code:

* **Init:**

```rust
let config = LifeguardConfig::load()?;
let pool = LifeguardPool::new(config)?;
```

* **Models & Records:**

```rust
#[derive(MayModel)]
struct User { ... }

#[derive(MayRecord)]
struct NewUser { ... }
```

* **CRUD:**

```rust
let user = NewUser { ... }.insert(&pool)?;
let u2 = User::find_by_id(&pool, user.id)?;
```

* **Migrations:**

```bash
lifeguard migrate up
```

No SeaORM anywhere.

---

## 7. Implementation Roadmap

### Phase 0 — Cleanup & Isolation (1–2 days)

* Isolate all SeaORM usage into a dedicated module like `sea_compat.rs`.
* Mark it as “to be deleted” once MayModel is in place.
* Add compile-time feature gate `seaorm_legacy` for transition.

### Phase 1 — Core Executor + Pool Refactor (3–5 days)

* Finalize `Executor` trait and implement with `may_postgres`.
* Rewire connection management to be fully coroutine-based, no SeaORM involvement.
* Add basic metrics hooks.

### Phase 2 — MayModel / MayRecord v1 (1–2 weeks)

* Implement `#[derive(MayModel)]` and `#[derive(MayRecord)]`.

* Support:

    * single table
    * basic CRUD
    * primary key lookups
    * simple filters via SeaQuery.

* Replace SeaORM entity usage in Lifeguard internals where feasible.

### Phase 3 — Migration System (3–5 days)

* Implement Lifeguard `Migration` trait + runner.
* Reuse SeaQuery migration patterns.
* Create CLI entrypoint `lifeguard-cli` or `cargo lifeguard`.

### Phase 4 — Metrics & Testkit (3–5 days)

* Wire Prometheus + OTel metrics around Executor.
* Provide `docker-compose.test.yml` and test harness.
* Write documentation & examples.

### Phase 5 — Kill SeaORM (as a user-visible dependency) (3–7 days)

* Remove SeaORM from all public signatures.
* If still used internally for migration glue, keep it behind a private module.
* Update README, docs, and examples to be Lifeguard-only.

---

If you’d like, next step I can:

* Turn this into a **markdown PRD** ready to drop into `docs/PRDs/lifeguard_system_PRD.md`.
* Or zoom into **one** slice (e.g. `MayModel/MayRecord macro design with example generated code) and flesh that out in detail.
