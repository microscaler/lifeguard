# Competitive comparison and ecosystem

*Snapshot for quick orientation. **Implementation Status** labels **shipped** crate behavior (including optional features), **partial** gaps, and **vision** rows (especially transparent cache and explicit read-preference APIs). Authoritative row-by-row coverage and percentages live in [SEAORM_LIFEGUARD_MAPPING.md](./docs/planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md) and `cargo doc`. For what is implemented **today** in this repository, see [STATUS.md](./STATUS.md). The [short summary](#implementation-status-summary-short) below complements that status with parity-oriented completion notes.*

## Competitive metrics: Lifeguard vs Rust ORMs

| Feature | Lifeguard Promise | Implementation Status | SeaORM | Diesel | SQLx |
|---------|-------------------|----------------------|--------|--------|------|
| **Concurrency Model** | ✅ Coroutine-native (`may`) | ✅ **Implemented** | ❌ Async/await (Tokio) | ❌ Sync-only | ❌ Async/await (Tokio) |
| **Performance (Hot Paths)** | ✅✅✅ 2-5× faster | 🟡 **Architectural** | ⚠️ Async overhead | ✅ Fast (sync) | ⚠️ Async overhead |
| **Performance (Small Queries)** | ✅✅✅ 10×+ faster | 🟡 **Architectural** | ⚠️ Future allocation | ✅ Fast | ⚠️ Future allocation |
| **Memory Footprint** | ✅✅ Low (stackful coroutines) | 🟡 **Architectural** | ⚠️ Higher (heap futures) | ✅ Low | ⚠️ Higher (heap futures) |
| **Predictable Latency** | ✅✅✅ Deterministic scheduling | 🟡 **Architectural** | ⚠️ Poll-based (variable) | ✅ Predictable | ⚠️ Poll-based (variable) |
| **Type Safety** | ✅✅✅ Compile-time validation | ✅ **Implemented** | ✅✅ Compile-time validation | ✅✅✅ Strong compile-time | ✅✅ Compile-time SQL checks |
| **ORM Features** | ✅✅✅ Complete (SeaORM parity) | 🟡 **High coverage** (core traits, relations, query builder; see mapping doc for %) | ✅✅✅ Complete | ✅✅ Good | ❌ Query builder only |
| **CRUD Operations** | ✅✅✅ Full support | ✅ **Implemented** (insert/update/save/delete via ActiveModelTrait) | ✅✅✅ Full support | ✅✅ Full support | ⚠️ Manual SQL |
| **Relations** | ✅✅✅ All types (has_one, has_many, belongs_to, many_to_many) | ✅ **Implemented** (Complete with eager/lazy loading, composite keys, DeriveLinked) | ✅✅✅ All types | ✅✅ Basic support | ❌ Manual joins |
| **Migrations** | ✅✅✅ Programmatic, data seeding, advanced ops | 🟡 **Partial** (`lifeguard::migration` + `lifeguard-migrate` + **`DeriveMigrationName`** / **`MigrationName`**; codegen paths still evolve) | ✅✅✅ Programmatic | ✅✅ CLI-based | ⚠️ Manual SQL |
| **Schema Inference** | ✅✅✅ From database (Diesel equivalent) | 🟡 **Partial** (`lifeguard-migrate infer-schema` / `schema_infer`, composite PK attributes, **`compare-schema`** column-name drift vs merged migrations; see [PRD §5](./docs/planning/PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md)) | ✅✅ From database | ✅✅✅ `table!` macro | ❌ No |
| **Query Builder** | ✅✅✅ Type-safe, chainable | ✅ **Implemented** (19/20 methods, 95% coverage) | ✅✅✅ Type-safe, chainable | ✅✅✅ Compile-time checked | ✅✅ Compile-time SQL |
| **Transactions** | ✅✅✅ Full support | ✅ **Implemented** (Roadmap Epic 01) | ✅✅✅ Full support | ✅✅ Full support | ✅✅ Full support |
| **Batch Operations** | ✅✅✅ insert_many, update_many, delete_many | ✅ **Implemented** | ✅✅✅ Batch support | ✅✅ Batch support | ⚠️ Manual |
| **Upsert** | ✅✅✅ save(), on_conflict() | ✅ **Implemented** (save() method exists) | ✅✅✅ save(), on_conflict() | ✅✅ on_conflict() | ⚠️ Manual SQL |
| **Pagination** | ✅✅✅ paginate(), paginate_and_count() | ✅ **Implemented** | ✅✅✅ Pagination helpers | ⚠️ Manual | ⚠️ Manual |
| **Entity Hooks** | ✅✅✅ before/after lifecycle events | ✅ **Implemented** (ActiveModelBehavior with 8 lifecycle hooks) | ✅✅✅ Hooks support | ❌ No | ❌ No |
| **Validators** | ✅✅✅ Field & model-level | 🟡 **Partial** — `run_validators` / `run_validators_with_strategy`, `ValidationStrategy::{FailFast, Aggregate}`, `ActiveModelBehavior::validate_fields` / `validate_model` / `validation_strategy`, derive `#[validate(custom = …)]`, `ValidateOp::Delete`; [`lifeguard::predicates`](./src/active_model/predicates.rs) for compose-in-`validate_fields`; not SeaORM’s full built-in validator attribute set — [PRD §6](./docs/planning/PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md) | ⚠️ Limited | ❌ No | ❌ No |
| **Soft Deletes** | ✅✅✅ Built-in support | ✅ **Implemented** (`#[soft_delete]` + `SelectQuery` / loader filtering) | ⚠️ Manual | ❌ No | ❌ No |
| **Auto Timestamps** | ✅✅✅ created_at, updated_at | ✅ **Implemented** (`#[auto_timestamp]` on `LifeRecord` insert/update paths) | ⚠️ Manual | ❌ No | ❌ No |
| **Session/Unit of Work** | ✅✅✅ Identity map, dirty tracking | 🟡 **Partial** (`ModelIdentityMap`, `Session`, `attach_session` / auto-dirty enqueue, `flush_dirty` / `flush_dirty_with_map_key`, pending insert + promote, `flush_dirty_in_transaction` / `flush_dirty_in_transaction_pooled`, `LifeRecord::identity_map_key`; [PRD §9](./docs/planning/PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md)) | ❌ No | ❌ No | ❌ No |
| **Scopes** | ✅✅✅ Named query scopes | 🟡 **Partial** (`SelectQuery::scope`, `scope_or` / `scope_any`, `IntoScope`, `lifeguard::scope`; **`find_related`** does not merge parent scopes—chain on returned `SelectQuery` — [PRD §7](./docs/planning/PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md)) | ❌ No | ❌ No | ❌ No |
| **Model Managers** | ✅✅✅ Custom query methods | ✅ **Implemented** (ModelManager trait + custom methods pattern) | ❌ No | ❌ No | ❌ No |
| **F() Expressions** | ✅✅✅ Database-level expressions | 🟡 **Partial** — `ColumnTrait::f_add` / `f_sub` / `f_mul` / `f_div`, derived `set_*_expr` + `update()`, `Expr::expr` + `ExprTrait` / `order_by_expr` for `WHERE`/`ORDER BY`; **PostgreSQL:** mixed numeric operand types follow server promotion rules—Lifeguard does not inject casts; use matching types, `SimpleExpr`, or `Expr::cust` for explicit `::bigint` / `::numeric` when required — [PRD §8](./docs/planning/PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md) | ❌ No | ⚠️ Limited | ❌ No |
| **Subqueries** | ✅✅✅ Full support | 🟡 **Partial** ([`join_subquery`](./src/query/select.rs), [`subquery_column`](./src/query/select.rs); not every SeaQuery subquery surface) | ✅✅✅ Full support | ✅✅ Full support | ✅✅ Manual SQL |
| **CTEs** | ✅✅✅ WITH clauses | 🟡 **Partial** ([`with_cte`](./src/query/select.rs) + lifeguard `all`/`one`; opt-in advanced SQL — [crate `query::select`](./src/query/select.rs)) | ✅✅✅ WITH clauses | ✅✅ WITH clauses | ✅✅ Manual SQL |
| **Window Functions** | ✅✅✅ Full support | 🟡 **Partial** ([`window`](./src/query/select.rs) / [`expr_window*`](./src/query/select.rs) / [`window_function_cust`](./src/query/select.rs)) | ✅✅✅ Full support | ✅✅ Full support | ✅✅ Manual SQL |
| **Eager Loading** | ✅✅✅ Multiple strategies (joinedload, subqueryload, selectinload) | ✅ **Implemented** (selectinload strategy with FK extraction) | ✅✅✅ Eager loading | ⚠️ Manual | ❌ Manual |
| **Raw SQL** | ✅✅✅ find_by_statement(), execute_unprepared() | ✅ **Implemented** (Architecture supports raw SQL) | ✅✅✅ Raw SQL support | ✅✅✅ Raw SQL support | ✅✅✅ Primary feature |
| **Connection Pooling** | ✅✅✅ Persistent, semaphore-based, health monitoring | ✅ **Shipped** ([`LifeguardPool`](./src/pool/pooled.rs): bounded queues, acquire timeout, heal, lifetime, metrics w/ `pool_tier`; see [pooling PRD](./docs/planning/PRD_CONNECTION_POOLING.md) for remaining parity) | ✅✅✅ Built-in pool | ⚠️ External (r2d2) | ✅✅✅ Built-in pool |
| **Replica Read Support** | ✅✅✅ WAL-based health monitoring, automatic routing | ✅ **Shipped** (replica tier + [`WalLagMonitor`](./src/pool/wal.rs); routing is pool-internal, not SeaORM-identical API) | ❌ No | ❌ No | ❌ No |
| **Read Preferences** | ✅✅✅ primary, replica, mixed, strong | 🟡 **Partial** ([`ReadPreference`](./src/pool/pooled.rs) + [`PooledLifeExecutor::with_read_preference`](./src/pool/pooled.rs) for explicit primary-tier reads; default pool routing still WAL/replica-aware; not full SeaORM “mixed/strong” semantics) | ❌ No | ❌ No | ❌ No |
| **Distributed Caching** | ✅✅✅✅ **LifeReflector (UNIQUE)** | 🟡 **Architectural** (Not in SeaORM mapping, may exist) | ❌ No | ❌ No | ❌ No |
| **Cache Coherence** | ✅✅✅✅ **Zero-stale reads (UNIQUE)** | 🟡 **Architectural** (Not in SeaORM mapping, may exist) | ❌ No | ❌ No | ❌ No |
| **TTL-Based Active Set** | ✅✅✅✅ **Adaptive caching (UNIQUE)** | 🟡 **Architectural** (Not in SeaORM mapping, may exist) | ❌ No | ❌ No | ❌ No |
| **PostgreSQL Features** | ✅✅✅ Views, materialized views, JSONB, FTS, PostGIS, partitioning | 🟡 **Partial** (JSONB ✅ core feature, others future) | ✅✅✅ Most features | ✅✅✅ Most features | ✅✅✅ All features (raw SQL) |
| **Observability** | ✅✅✅ Prometheus, OpenTelemetry, comprehensive metrics | ✅ **Implemented** (optional `metrics` / `tracing`; OTel-compatible / OTLP; [OBSERVABILITY.md](./OBSERVABILITY.md); pool series with `pool_tier`) | ✅✅ Basic metrics | ⚠️ Limited | ⚠️ Limited |
| **Developer Experience** | ✅✅✅ Familiar API, no async/await, clear errors | ✅ **Implemented** (SeaORM-like API) | ✅✅✅ Good, async/await required | ⚠️ Complex type system | ✅✅ Good, async/await required |
| **Learning Curve** | ✅✅ Moderate (familiar if you know SeaORM) | ✅ **Implemented** (SeaORM-like API) | ✅✅ Moderate | ⚠️ Steep (complex macros) | ✅✅ Moderate |
| **Production Ready** | ✅✅✅ Complete observability, health checks, metrics | 🟡 **Workload-dependent** (core ORM + pool + metrics/tracing ship; validate migrations, cache, and ops for your deployment) | ✅✅✅ Production ready | ✅✅✅ Production ready | ✅✅✅ Production ready |
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

### Implementation status summary (short)

**Strong in-tree today:** core traits (`LifeModelTrait`, `ModelTrait`, `ActiveModelTrait`, …), CRUD/save paths, `SelectQuery` stack, relations and eager/loader paths (including composite keys and linked traversals), migrations framework (`lifeguard::migration`, `lifeguard-migrate`), JSON column support, derive **`#[soft_delete]`** / **`#[auto_timestamp]`**, partial models, lifecycle hooks, **`LifeguardPool`** / **`PooledLifeExecutor`** with primary+replica tiers, WAL lag routing, slot heal, idle liveness, max connection lifetime, and optional **metrics** (including **`pool_tier`** labels) / **tracing**.

**Partial (PRD v0 shipped; see [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md](./docs/planning/PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md)):** schema inference CLI/module (**composite PK** `#[primary_key]` on each column; **`compare-schema`** column-name drift vs merged migrations); validators (pipeline + aggregate mode + derive `custom` + **`lifeguard::predicates`** — this document and the mapping doc spell out shipped vs SeaORM gaps); `SelectQuery::scope` + **`scope_or` / `scope_any`** + **`#[scope]`** (parent scopes not merged into **`find_related`**—chain on the returned query); F() on **`UPDATE`** (derived `set_*_expr`) and **`WHERE`/`ORDER BY`** via SeaQuery (**PostgreSQL numeric promotion** documented in PRD §8 / `ColumnTrait::f_add`); **`Session`** / **`ModelIdentityMap`** with **`mark_dirty_key`**, **`attach_session`** (dirty enqueue when PK set), **`flush_dirty_in_transaction`** / **`flush_dirty_in_transaction_pooled`** ( **`LifeguardPool::exclusive_primary_write_executor`** ), **`register_pending_insert`** / **`flush_dirty_with_map_key`** / **`promote_pending_to_loaded`**.

**Partial or roadmap:** deeper SQL builder coverage (e.g. more `SeaQuery` surface re-exported on [`SelectQuery`](./src/query/select.rs)), further migration tooling parity, and any remaining pooling parity called out in [PRD_CONNECTION_POOLING.md](./docs/planning/PRD_CONNECTION_POOLING.md) and [POOLING_OPERATIONS.md](./docs/POOLING_OPERATIONS.md). **Shipped on `SelectQuery`:** [`with_cte`](./src/query/select.rs) (CTE + `all`/`one`), [`join_subquery`](./src/query/select.rs), [`window`](./src/query/select.rs) / [`expr_window*`](./src/query/select.rs), existing [`subquery_column`](./src/query/select.rs) / [`window_function_cust`](./src/query/select.rs). **Pool reads:** [`ReadPreference`](./src/pool/pooled.rs) + [`PooledLifeExecutor::with_read_preference`](./src/pool/pooled.rs) force primary-tier reads when you need read-your-writes; default routing still follows WAL lag. **Session:** `LifeRecord::attach_session_with_model` auto-syncs literals into the identity-map `Rc` via `to_model()` when mutations notify the session ([PRD §9](./docs/planning/PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md)); F-style `set_*_expr` remains record-only until `update()`.

**Roadmap / vision:** productized “transparent Redis on every read”; LifeReflector and cache coherence in [`lifeguard-reflector`](./lifeguard-reflector/).

For percentages and row-by-row status, use the mapping document linked in the section intro rather than this table alone.

### Key differentiators

**Lifeguard's Unique Advantages:**
1. **LifeReflector** - Distributed cache coherence (Oracle Coherence–style active set) — **unique**; **🟡** product evolution in [`lifeguard-reflector`](./lifeguard-reflector/)
2. **Coroutine-Native** - No async overhead, deterministic scheduling — **unique** among these ORMs ✅
3. **WAL-Based Replica Routing** - Pool + [`WalLagMonitor`](./src/pool/wal.rs) — **shipped** for `LifeguardPool` reads ✅
4. **TTL-Based Active Set** - Adaptive caching — **🟡** vision / reflector path; not automatic on every app read
5. **DeriveLinked Macro** - Multi-hop relationship code generation — **competitive advantage** ✅ (SeaORM has no direct equivalent)
6. **Session/Unit of Work** — **🟡** `Session` + identity map + `flush_dirty` / `flush_dirty_with_map_key` / pending insert + promote / `flush_dirty_in_transaction` / `flush_dirty_in_transaction_pooled` ([PRD §9](./docs/planning/PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md))

**Where Lifeguard Matches or Exceeds:**
- ✅ Substantial SeaORM-oriented coverage (see mapping doc for %; core ORM paths strong)
- ✅ Relations system with composite keys and eager/lazy loading
- ✅ Query builder with 95% method coverage
- ✅ Better performance potential (2-5× faster on hot paths - architectural)
- ✅ Lower memory footprint (architectural)
- ✅ Predictable latency (architectural)

**Trade-offs:**
- ❌ PostgreSQL-only (by design - enables advanced features)
- ❌ Requires `may` coroutine runtime (not Tokio)
- ❌ Smaller ecosystem (newer project)
- ⚠️ Some roadmap items remain (further query-builder / migration tooling parity, etc.); see [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md](./docs/planning/PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md), mapping doc, and pooling docs

### Performance comparison (estimated)

| Metric | Lifeguard | SeaORM | Diesel | SQLx |
|--------|-----------|--------|--------|------|
| **Simple Query Latency** | 0.1-0.5ms | 0.5-2ms | 0.2-1ms | 0.5-2ms |
| **Hot Path Throughput** | 2-5× faster | Baseline | 1-2× faster | Baseline |
| **Small Query Overhead** | Minimal | Future allocation | Minimal | Future allocation |
| **Memory per Connection** | ~100 bytes | ~1-2 KB | ~100 bytes | ~1-2 KB |
| **Concurrent Connections** | 800+ (1MB stack) | Limited by Tokio | Limited by threads | Limited by Tokio |
| **p99 Latency** | < 5ms (predictable) | 5-20ms (variable) | < 5ms (predictable) | 5-20ms (variable) |

*Note: Performance numbers are estimates based on architecture. Actual benchmarks will be published after implementation.*

### Target performance claims (product narrative)

**Target Performance:**
- 2-5× faster than async ORMs on hot paths
- 10×+ faster on small queries (no future allocation overhead)
- Predictable p99 latency (< 5ms for simple queries)
- Lower memory footprint than async alternatives

**Real-World Use Cases:**
- **BRRTRouter**: High-throughput API routing with sub-millisecond database access (100,000+ requests/second)
- **High-Scale Microservices**: Applications requiring millions of requests/second with limited database connections
- **Low-Latency Systems**: Real-time applications needing predictable p99 latency (< 5ms) for database operations

### Ecosystem compatibility

**⚠️ Important: BRRTRouter and Lifeguard are a parallel ecosystem, separate from async/await Rust.**

These are **two incompatible worlds** with the only commonality being Rust itself:

| Ecosystem | Runtime | ORM Options | Incompatible With |
|-----------|---------|-------------|-------------------|
| **BRRTRouter + Lifeguard** | `may` coroutines | Lifeguard only | SeaORM, Diesel (async), SQLx, Tokio |
| **Tokio + Async ORMs** | `async/await` | SeaORM, Diesel, SQLx | BRRTRouter, Lifeguard, `may` |

**You cannot mix and match.** If you're using BRRTRouter, you **must** use Lifeguard. The async/await ORMs (SeaORM, Diesel, SQLx) are fundamentally incompatible with the `may` coroutine runtime.

### When to use each ecosystem

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

[← README](./README.md) · [Status](./STATUS.md) · [Roadmap](./ROADMAP.md)
