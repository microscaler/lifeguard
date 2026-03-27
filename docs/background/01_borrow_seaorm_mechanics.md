Can we borrow SeaORM migration mechanics ?

---

**Yes — and this is one of the *few* parts of SeaORM you *should* borrow.**

SeaORM’s migration tooling is:

* **runtime-agnostic** (not tied to async, not tied to SeaORM's executor)
* **implemented using `sea-schema` + `sea-query`**
* **modular and standalone**
* **already stable, tested, and widely used**

Unlike the ORM engine, the **migration system is clean, decoupled, and absolutely reusable** in a synchronous/coroutine-driven environment.

This is exactly the kind of component you *should not re-invent*.

---

# ✅ **Short answer: Yes, borrow it. No need to replicate it.

SeaORM Migration = safe to keep + fully compatible with MayModel.**

---

# 🔍 Why SeaORM Migration *is compatible* with a MayModel architecture

SeaORM's migration crate:

* does **not** depend on async database drivers
* does **not** depend on `DatabaseConnection`
* does **not** depend on ActiveModel or SeaORM runtime
* uses only **SeaQuery** + **SeaSchema**
* produces SQL strings
* can run through **any executor** — including your custom one
* has a clean migration trait:

```rust
#[async_trait]
pub trait MigrationTrait { ... }
```

Even though it's async, the “async traits” here **do not perform real async I/O** unless *you* glue them into an async executor.
If you're using a synchronous DB executor, the migration code simply does:

```
executor.execute(sql)
```

You can:

* keep the async trait (coroutines can run inside it anyway)
* rewrite it as a synchronous trait
* or offer two backends (async + may)

**The core logic is reusable exactly as-is.**

---

# 🔥 What Parts You Should Keep From SeaORM Migration

### **1. The Migration DSL**

Examples:

```rust
Table::create()
    .table(User::Table)
    .if_not_exists()
    .col(ColumnDef::new(User::Id).integer().not_null().auto_increment().primary_key())
    .col(ColumnDef::new(User::Email).string().not_null())
    .to_owned()
```

This is:

* battle-tested
* readable
* maintainable
* fully SeaQuery-driven
* zero async
* deterministic

Perfect for MayModel.

---

### **2. SeaSchema integration**

This handles:

* schema diffing
* metadata extraction
* type mappings

Not required for v1, but beneficial for v2.

---

### **3. Migration runner CLI**

You can *reuse* its conceptual structure:

* `migration up`
* `migration down`
* `migration status`
* `generate_migration MyMigrationName`

You do **not** need SeaORM async runtime for this.

---

# 🛑 What You Should *Not* Borrow

### ❌ `DatabaseConnection`-based execution

You must replace this with your:

```
MayExecutor
```

That wraps `may_postgres`.

### ❌ SeaORM’s async MigrationTrait

You replace with:

```
trait Migration {
    fn up(&self, exec: &impl Executor) -> Result<()>;
    fn down(&self, exec: &impl Executor) -> Result<()>;
}
```

Or keep async and let May coroutines wrap it; it's trivial either way.

### ❌ SeaORM’s coupling to `ActiveModel`

Your migrations should be **schema-driven**, not model-driven.

---

# ⚙️ How SeaORM Migration Would Integrate with MayModel

### Your migration runner:

```rust
fn run_migrations(exec: &impl Executor) -> Result<()> {
    Migrator::run(exec)
}
```

### Migration itself:

```rust
pub struct CreateUserTable;

impl Migration for CreateUserTable {
    fn up(&self, exec: &impl Executor) -> Result<()> {
        let stmt = Table::create()
            .table(User::TABLE)
            .col(...)
            .to_string(PostgresQueryBuilder);

        exec.execute(&stmt)?;
        Ok(())
    }
}
```

**Zero async.
Zero SeaORM dependency.
Pure SeaQuery.**

---

# 🧬 Example: Migration + MayRecord + MayModel working together

Migration:

```rust
Table::create()
    .table(User::TABLE)
    .col(ColumnDef::new(User::Id).big_integer().not_null().primary_key())
    .col(ColumnDef::new(User::Email).string().not_null())
```

MayModel:

```rust
#[derive(MayModel)]
struct User { id: i64, email: String }
```

MayRecord:

```rust
#[derive(MayRecord)]
struct UserRecord { email: String }
```

Insert:

```rust
let user = UserRecord { email: "a@b.com".to_string() }.insert(&pool)?;
```

Everything lines up.
Clean.
Predictable.
Zero async overhead.

---

# 🧨 Strategic Reality Check

### ✔ You **cannot** reuse SeaORM runtime

### ✔ You **can** reuse SeaORM migration DSL cleanly

### ✔ This saves you months

### ✔ This preserves BI-friendly schema diffs

### ✔ This keeps your ORM-lite small

### ✔ This avoids reinventing good tooling

Your custom ORM does **data access**.
SeaORM migration handles **schema evolution**.
They do not need to be linked.

This is the correct separation of concerns.

---

# 🎯 Final Answer

### **Yes — borrow SeaORM migration mechanics.

Integrate them with MayModel/MayRecord via a custom Executor.
Do NOT reuse SeaORM’s async runtime or ORM code.**

This gives you the best balance of:

* speed
* correctness
* maintainability
* small surface area
* reduced engineering burden

---

If you want, I can generate next:

### 📄 **Migration System PRD for Lifeguard ORM**

### 🔧 A minimal example migration runner

### 📐 Architecture diagram

### 🧪 Testing plan for MayMigration + MayExecutor

Just say:

👉 **“Generate Migration PRD v1.”**


---

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


---

What about things like foreign keys, views, and some of the more advanced functionality that Postgres’s provides? We will need this, maybe not in the first iteration but we need to build these into the PRD

---

Absolutely — **foreign keys, views, constraints, indexes, sequences, triggers, materialized views, generated columns, partial indexes, computed expressions, and Postgres-specific features** must be included in the *full system PRD*, even if phased into later implementations.

Below is the **expanded PRD section** covering all advanced PostgreSQL functionality Lifeguard must eventually support — divided into:

1. **Core v1 must-have features**
2. **Postgres Advanced Features (v2/v3 scope)**
3. **ORM surface requirements**
4. **Migration system requirements**
5. **Query builder enhancements**
6. **Introspection & tooling**
7. **Future expansion**

This can be inserted directly into the earlier PRD.

---

# 🔥 **Lifeguard PRD — Advanced PostgreSQL Feature Support**

The following advanced Postgres functionality must be supported in Lifeguard's ORM layer (`MayModel`, `MayRecord`), migration system, and query builder paths.

This defines the **feature-complete end state**, not just v1.

---

# 1. Base PostgreSQL Capabilities (v1 / v1.5)

These must be present early because they’re structural:

## **1.1 Foreign Keys**

* Add FK constraints via migrations.
* Provide ORM annotations:

```rust
#[belongs_to(User, foreign_key = "user_id")]
```

* Provide query helper:

```rust
Comment::find().with(User).all(&pool)?;
```

* FK enforcement is handled by Postgres; ORM only helps with struct-level mapping & relation loading.

---

## **1.2 Indexes**

Support in migrations:

* `CREATE INDEX`
* `UNIQUE INDEX`
* Multi-column indexes
* Partial indexes

Examples:

```rust
Index::create()
    .name("idx_users_email")
    .table(User::TABLE)
    .col(User::Email)
    .unique()
    .to_owned();
```

Partial index:

```rust
Index::create()
    .table(Order::TABLE)
    .col(Order::Archived)
    .condition(Expr::col(Order::Archived).eq(false))
```

---

## **1.3 Composite Primary Keys**

ORM must be capable of deriving:

```rust
#[derive(MayModel)]
#[primary_key(order_id, line_id)]
```

v1 may simply support read-only composite PKs; updates come in v2.

---

## 1.4 Unique Constraints

In migration:

```rust
ColumnDef::new(Order::Reference)
    .string()
    .unique()
```

ORM-level validation remains optional, but DB-level enforcement is mandatory.

---

# 2. Intermediate PostgreSQL Features (v2)

These are essential in production systems.

## **2.1 Views**

### Requirements:

* Migrations must support `CREATE VIEW` and `CREATE OR REPLACE VIEW`.
* Views must be queryable via `MayModel`:

```rust
#[derive(MayModel)]
#[view = "active_users_view"]
struct ActiveUserView { ... }
```

No `MayRecord` on views (read-only).

---

## **2.2 Materialized Views**

### Requirements:

* `CREATE MATERIALIZED VIEW` support in migrations.
* Trigger refresh:

```rust
MaterializedView::refresh("daily_sales_summary", &pool)?;
```

Optional: auto-refresh scheduling via external cron or Postgres triggers.

---

## **2.3 Generated Columns**

Postgres 12+ supports:

```sql
generated always as (expr) stored
```

Migration DSL:

```rust
ColumnDef::new(User::SearchKey)
    .string()
    .generated("lower(email || ' ' || name)")
    .stored()
```

MayModel should treat these as read-only fields.

---

## **2.4 Check Constraints**

Migration support:

```rust
Table::create()
    .col(ColumnDef::new(User::Age).integer())
    .check(Expr::col(User::Age).gte(0))
```

No ORM-level enforcement necessary.

---

## **2.5 Exclusion Constraints**

For scheduling / geospatial systems:

```rust
Constraint::exclude()
    .using("gist")
    .col((Booking::RoomId, "="))
    .col((Booking::TimeRange, "&&"))
```

---

## **2.6 Sequences**

Migration support:

```rust
Sequence::create().name("user_seq").starts_with(1000)
```

ORM support:

```rust
let id = Sequence::nextval("user_seq", &pool)?;
```

---

# 3. Advanced PostgreSQL Features (v3+)

These are powerful, less commonly used, but must be in the architecture.

---

## **3.1 Triggers**

Migration DSL:

```rust
Trigger::create()
    .name("set_updated_at")
    .table(User::TABLE)
    .when(TriggerEvent::BeforeUpdate)
    .function("update_timestamp()")
```

ORM design:

* No built-in trigger generation beyond migration DSL.
* Triggers operate entirely at the DB level.

---

## **3.2 Stored Procedures & Functions**

Migration support:

```rust
Function::create()
    .name("calculate_discount")
    .language("plpgsql")
    .body("BEGIN ... END;")
```

ORM support for calling functions:

```rust
let result = db.call_function("calculate_discount", &[&id, &qty]);
```

---

## **3.3 Full-Text Search**

Migration DSL:

```rust
Index::create()
    .using("gin")
    .col(Expr::cust("to_tsvector('english', body)"))
```

ORM query helper:

```rust
Post::search("rust programming", &pool)?;
```

Under the hood:

```sql
WHERE to_tsvector('english', body) @@ plainto_tsquery($1)
```

---

## **3.4 JSONB Querying**

ORM helper for:

```rust
User::find().filter(User::data().contains(json!({"active": true}))).all(&pool)
```

Migration must support JSONB columns:

```rust
ColumnDef::new(User::Preferences).json_binary()
```

---

## **3.5 Geospatial (PostGIS) support**

v3+ optional.

* Migration support for `geometry` and `geography` types.
* ORM support for:

```rust
Location::find().within_radius(lat, long, 1000)?;
```

---

## **3.6 Partitioning**

Migration DSL:

```
Table::create().partition_by("RANGE (created_at)")
```

ORM: transparent (models do not need special logic).

---

## **3.7 Logical Replication / CDC Hooks**

Out of scope for ORM, but may be required for ETL integrations in the future.

---

# 4. ORM Requirements for Advanced Features

To support advanced PostgreSQL features, MayModel must be extended:

1. **Column attribute metadata**

    * Types
    * Nullable flags
    * Constraints
    * Default values
    * Generated columns

2. **Relations**

    * `has_one`
    * `has_many`
    * `belongs_to`

3. **Relation loading**

    * `.with(User)`
    * `.with_many(Orders)`
    * `.preload(...)`

4. **Computed columns**

    * Should support read-only fields.

5. **View models**

    * Must be queryable like tables.

6. **Materialized views**

    * Must be queryable + refreshable.

---

# 5. Migration Requirements

Migrations must support:

### Table operations:

* Create/alter/drop tables
* Column add/remove/modify
* Composite primary keys
* Constraints (check, unique, exclusion, FK)
* Index creation (btree, gin, gist, hash)
* Partial indexes
* Adding/removing default expressions

### View operations:

* Create view
* Replace view
* Drop view

### Materialized view operations:

* Create
* Refresh
* Drop

### Trigger operations:

* Create trigger
* Drop trigger

### Function/procedure operations:

* Create
* Replace
* Drop

### Foreign keys & relationships:

* Add FK
* Drop FK

### Sequences:

* Create sequence
* Alter sequence
* Drop sequence

### Partitions (v3):

* Create partitions
* Attach/detach partitions

---

# 6. Query Builder Requirements

Enhancements to SeaQuery wrappers:

### Must support:

* joins (inner / left / right / cross)
* group-by / having
* aggregates (sum, avg, count)
* ordering
* windows:

```rust
OVER (PARTITION BY ... ORDER BY ...)
```

* subqueries
* raw SQL fragments when needed

---

# 7. Introspection & Tooling

Future optional tool:

### `lifeguard inspect`

* Inspect Postgres schema
* Generate draft models from tables
* Validate migrations vs actual schema
* Diff detection

This is not v1, but must be in PRD.

---

# 8. Future Expansion (v3+)

* Connection multiplexing with pgbouncer compatibility
* Async mode (if needed) via feature flag
* Sharding support / read replicas
* CDC hooks (Debezium style)
* Integration into BRRTRouter typed handlers

---

# ⚡ Summary: Yes, Lifeguard Must Support All Advanced PG Features

The PRD now includes:

* Foreign keys
* Views & materialized views
* Indexes (incl. partial & gin/gist)
* Generated columns
* Triggers
* Functions / procedures
* JSONB
* Full-text search
* ORM mapping for complex types
* PostGIS (optional)
* Partitioning

All defined in a phased, structured roadmap (v1 → v2 → v3).

---

If you want, I can:

👉 **Insert this into a fully formatted PRD file for canvas**
👉 Or generate **full system architecture diagrams**
👉 Or design the **derive(MayModel) / derive(MayRecord)** macros with examples of generated code.

Just tell me your preferred next step.


---

Let’s produce a complete single PRD now we know the full scope.
Order the deliverables so we don’t have to jump back and forth.

This will form the bases for a NEW readme for Lifeguard as we have realised the previous design that resulted in the project hitting a proverbial brick wall and stalling.



---


