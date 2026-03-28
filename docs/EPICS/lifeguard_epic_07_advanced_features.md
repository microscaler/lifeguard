# EPIC 07: Lifeguard Advanced ORM Features & Lifecycle Hooks

## Context & Justification
Lifeguard has successfully achieved its v1 MVP milestone, proving coroutine-native (zero `async/await`), zero-cost database mapping via the `may` runtime. However, the current framework strictly defers advanced mutation hooks, validations, and query aggregations. 

As downstream platforms (such as the `hauliage` microservice ecosystem) scale, manually injecting timestamps, handling soft-deletes natively in isolated `Service` patterns, and managing audit trails becomes boilerplate-heavy and susceptible to developer drift. 

This EPIC outlines the next critical phase for Lifeguard: pushing enterprise-grade guardrails down into the macro AST compilation boundary.

---

## 1. Lifecycle Hooks & Timestamp Automations

### Why it's needed
Currently, ORM users must manually inject `created_at` and `updated_at` properties inside their BrrtRouter handlers or custom Service wrappers before calling `.insert()`. Furthermore, if a system demands an event-driven architecture (i.e. emitting an event to Kafka when a record is created), the developer must remember to execute this logic manually right after ORM interactions. 

### Implementation Strategy
Expand the `lifeguard_derive` `syn` mappings to accept event-based attributes:
- `#[before_insert]`, `#[after_insert]`
- `#[before_update]`, `#[after_update]`
- `#[before_delete]`, `#[after_delete]`
- `#[auto_timestamp]` (syntactic sugar injecting default timestamp behaviors)

**Execution Flow**:
Inside the code generation core (`lifeguard-derive/src/active_model.rs`), modify the trait implementations for `insert`, `update`, and `delete`. The macro should parse explicit function pointers passed into the attributes and systematically invoke them chronologically around the `may_postgres::batch_execute` block.

**Usage Example:**
```rust
#[derive(LifeModel, LifeRecord)]
#[auto_timestamp] // Dynamically applies created_at/updated_at logic
#[after_insert(emit_platform_event)]
#[before_update(validate_business_rules)]
pub struct Consignment {
    #[primary_key]
    pub id: String,
    // ...
}

fn emit_platform_event(record: &ConsignmentRecord) {
    // Dispatch to pub/sub...
}
```

---

## 2. Native Soft Deletion

### Why it's needed
Calling `.delete()` currently truncates the row permanently from PostgreSQL. Enterprise systems rarely do this natively. Implementing soft deletes across a microservice today requires every single model developer to manually append `.filter(Column::DeletedAt.is_null())` into their SeaQuery conditions, risking severe data-leak bugs if forgotten.

### Implementation Strategy
Introduce a table-level `#[soft_delete]` attribute. 

**Execution Flow**:
1. **Model Generation**: The macro dynamically maps an implicit `deleted_at: Option<chrono::NaiveDateTime>` field internally if not present.
2. **Delete Trait**: Overrides `ActiveModelTrait::delete` to execute `UPDATE {table} SET deleted_at = NOW()` instead of `DELETE FROM`.
3. **Query Engine**: Overrides `Entity::find()` to intrinsically inject `sea_query::Expr::col(...).is_null()` protecting all `.all()` and `.find_one()` retrievals by default.
4. **Bypass Method**: Expose a `.with_trashed()` command on the `LifeQuery` builder for explicit admin reads.

**Usage Example:**
```rust
#[derive(LifeModel, LifeRecord)]
#[soft_delete]
#[table_name = "vehicles"]
pub struct Vehicle { ... }

// Intrinsically filters out soft-deleted Vehicles safely.
let active = Vehicle::find().all(&executor)?; 
```

---

## 3. Advanced Querying Capabilities (PostGIS & Aggregations)

### Why it's needed
Aggregations (`COUNT()`, `SUM()`, `GROUP BY`) and CTEs (Common Table Expressions) require breaking the Lifeguard typed abstractions and writing raw string SQL queries. Furthermore, as telemetry vectors scale, raw spatial bindings (`PostGIS`) execution wrappers do not exist.

### Implementation Strategy
Broaden the `LifeQuery` builder explicitly wrapping the underlying `sea-query` components seamlessly:
1. Provide a `LifeAggregate` interface for dynamic unpacking of standard numeric columns without tying the struct to a `#[derive(LifeModel)]` entity.
2. Develop standard method expansions for `.count()`, `.sum(Column::X)`, and `.group_by(Column::Y)`.

---

## 4. Cache Coherence Architecture (LifeReflector)

### Why it's needed
Outlined originally in *Epic 05*, heavy `Entity::find_by_id()` calls stress PostgreSQL connections unnecessarily for high-read platforms. We needed to mitigate connection exhaustion natively inside the ORM queries without relying on the developer to wrap their application code in a Redis layer.

### Phase 4 MVP Implementation Status (Completed)
We have successfully implemented the core transparent caching layer, known as **LifeReflector**, bounded by minimal PostgreSQL overheads:
1. **Abstract Cache Interface**: Scaffolded `lifeguard/src/cache/mod.rs` and the `CacheProvider` trait exposing `.get()`, `.set()`, and `.invalidate()`. Integrated `RedisCacheProvider` natively.
2. **Transparent Read-Through**: Wrapped `Entity::find_by_id(pk)` inside `query/manager.rs` to automatically fetch from `CacheProvider` before executing SQL logic.
3. **Synchronous Write-Through Hooks**: Overrode `ActiveModelBehavior` macros (`insert`, `update`, `delete`) to synchronously update/invalidate the Redis cache the moment a record completes its transaction. This effectively solves the "CDC Delay Paradox".
4. **WAL Lag Polling Coroutine**: Implemented `WalLagMonitor` (`src/pool/wal.rs`) utilizing a background `may::coroutine` to poll Postgres replica lag (`pg_is_in_recovery()`, LSN replay diffs) every 500ms safely outside of the request pipeline.
5. **LifeReflector Daemon Scaffold**: Created the standalone background worker loop (`src/cache/reflector.rs`) executing PostgreSQL `LISTEN/NOTIFY` for out-of-band CDC.

### Deferred Architecture / Considerations Held-Off

During implementation, critical trade-offs were evaluated. We held off on the following mechanisms to ship a stable MVP:

- **Raft Consensus for Leader Election**: We **abandoned** the original design intent of utilizing a P2P Raft network (e.g. `openraft`) for the LifeReflector worker nodes. Instead, we implemented **Redis Distributed Locks (Redlock)**. Standing up a distributed consensus protocol just to synchronize a cache invalidator adds unjustifiable operational complexity when the architecture already intrinsically requires Redis.
- **Secondary Index / Cross-Column Caching**: Caching is strictly limited to Primary Key point-read lookups (`lifeguard:model:entity:id`). We **deferred** complex reverse-lookup implementations or `find_by()` query invalidations, as they demand brittle active-set coherency tracking that goes beyond initial MVP limits.
- **Exclusive Reliance on `LISTEN/NOTIFY`**: Instead of relying *solely* on `LISTEN/NOTIFY` as initially conceptualized in the LifeReflector blog spec, we implemented dual guarantees. Synchronous macro hooks (Write-Through) act as the primary defense against cache stampedes natively from Lifeguard ORM interactions, while async `LISTEN/NOTIFY` acts solely as a fallback mapping for manual database administrator mutations.
