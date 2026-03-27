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
Outlined originally in *Epic 05*, heavy `Entity::find_by_id()` calls stress PostgreSQL connections unnecessarily for high-read platforms.

### Implementation Strategy
Build a standalone worker/trait definition `CacheProvider` hooking into a distributed key-value store (e.g., Redis). Integrates fundamentally with the Lifecycle hooks (i.e. `after_update` triggers an eviction protocol automatically tracking `Entity` keys across the synchronized cache boundary natively inside the ORM layer, completely transparent to the microservice developer).
