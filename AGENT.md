# Agent Guide for Lifeguard

This document provides agentic AI systems with essential context for working on the Lifeguard ORM and migration tooling.

---

## Project Overview

**Lifeguard** is a coroutine-driven ORM for Rust using the `may` runtime. It provides `LifeModel` (immutable rows), `LifeRecord` (mutations), migration generation via `lifeguard-migrate`, and `lifeguard-derive` for `#[derive(LifeModel)]` and related attributes.

---

## Entity and Index Rules (Preventing Invalid Indices)

Migration SQL is generated from `#[derive(LifeModel)]` structs. The `#[index = "idx_name(column)"]` attribute is parsed and emitted as `CREATE INDEX` in migrations. **lifeguard-derive and lifeguard-migrate do not validate that index columns exist on the struct.** Invalid index definitions lead to migration failures at apply-time (e.g. PostgreSQL: column does not exist).

### Rules for correct `#[index]` and `#[indexed]` usage

1. **Every column in `#[index = "idx_name(col1, col2)"]` must exist as a field on that struct.**  
   The derive only parses the string; it does not check the struct’s fields. Indexing a non-existent column produces broken SQL.

2. **Child/specialized entities that reference a base entity via FK do not inherit the base’s columns.**  
   Example: a `CustomerInvoice` that links to `invoices` via `invoice_id` does **not** have `invoice_number`, `due_date`, or `status`; those belong to `invoices`.  
   - ✅ Index on `customer_id` on `CustomerInvoice` (present on the struct).  
   - ❌ Index on `invoice_number` on `CustomerInvoice` (only on `invoices`).

3. **`#[indexed]` on a field** must be on a real field of that struct. Same rule: only index columns that exist on the current model.

### How this class of bug arises

- Copying index attributes from a **base** entity onto a **child** entity that only holds an FK to the base.
- Adding `#[index = "idx_foo(bar)"]` without confirming `bar` is a field on the same struct.
- Refactoring: moving or renaming a column but leaving an index that still references the old name.

### Checklist when adding or changing `#[index]` / `#[indexed]`

1. List all fields of the struct. Each column in every `#[index = "idx_name(x,y)"]` and each `#[indexed]` must be in that list.
2. If the struct is a child of another (e.g. `invoice_id` → `invoices`), do **not** copy indices from the parent; only index columns that exist on the child.
3. Run migration generation and apply in a test database to ensure `CREATE INDEX` statements succeed.

### Possible future improvement

Adding validation in **lifeguard-derive** (at macro expand time) or in **lifeguard-migrate** (when building the table definition) to ensure every index column matches a struct field would prevent this class of bug. When touching the derive or migration code, consider implementing such a check.

---

## Important Paths

- `lifeguard-derive/` – proc macros for `LifeModel`, `LifeRecord`, and attributes (e.g. `#[index]`, `#[indexed]`).
- `lifeguard-migrate/` – migration discovery, SQL generation, and application.
- `lifeguard-derive/src/attributes.rs` – parsing of `#[index = "idx_name(col)"]` and related table attributes.
- `migrations/` – `original/` and `generated/` SQL migrations.
- `book/` – MDBook documentation.

---

## Index Attribute Format

- `#[index = "idx_name(column)"]` – one or more columns.
- `#[index = "idx_name(col1, col2) WHERE condition"]` – partial index (implementation-dependent).
- `#[indexed]` on a field – per-field index (exact behavior as in lifeguard-derive).

---

*This document is maintained for agentic AI systems working on Lifeguard.*

---

## Case Study: "Ghost In The Router" API Response Empty Mismatches

During the formalization of the fleet tracking service architecture, an intricate chain of silent failures resulted in `[]` API responses masking catastrophic DB and Routing logic bugs. 
Agents debugging "missing data" or "returns empty array" in BRRTRouter + Lifeguard stacks should actively verify these two constraints:

### 1. Lifeguard/Database `UUID` vs `String` Translation Panics
If you map a PostgreSQL `UUID` field to a `pub id: String` inside a struct (e.g. `models/vehicle.rs`), the underlying database binding (via `sea-query`) will fail silently with a `TypeErr` while executing `.all()`.
If the microservice logic gracefully degrades to returning an `Err()` branch populated with custom debug objects (`status: "ERROR"`), **BRRTRouter will silent strip out these debug objects** if their fields fall out of bounds of the strict `openapi.yaml` schemas (e.g. `enum: [ACTIVE, ... ]`).
This returns a completely flawless `200 OK` response with a `[]` payload back to the browser UI, completely masking the fact that the underlying ORM panics entirely on the primitive mismatch.

* **Prevention**: Ensure that all DB `UUID` configurations explicitly enforce `uuid::Uuid` primitives inside `#[derive(LifeModel)]` definitions, and handle conversion to `.to_string()` directly before serializing out to the endpoints.

### 2. Startup Handler Sweeping (`registry.rs` vs `main.rs`)
If you manually register concrete controllers in `main.rs` to overwrite `gen/.../controllers`, you must register your hooks strictly **after** invoking the auto-generated macro `registry::register_from_spec(&mut dispatcher, &routes);`. 
The macro inherently begins its startup sequence by running `dispatcher.handlers.clear();` to avoid memory accumulation during live reloads. Any manual overriding of channels registered before this line will be summarily obliterated from the dispatcher block. 
When this occurs, the API seamlessly falls back onto `brrtrouter-gen`'s generated mock stubs (which are hardcoded to natively return `Response(vec![])`), completely dropping your underlying controller logic logic inside `impl/src/`.

* **Prevention**: `register_from_spec` MUST be the very first initialization step in the controller bindings process inside `impl/.../main.rs`. All custom `brrtrouter::typed::spawn_typed_with_stack_size_and_name` hooks MUST sequentially follow it to cleanly overwrite the stub routing pointers.

### Historical Postmortems and ADRs
To trace the specific histories of these failures mapped across active branches, review these formal documents:
- [Consignments Postmortem: `list_jobs` UUID Failure](../hauliage/docs/postmortem-consignments-list-jobs-empty-2026-04.md)
- [Fleet Postmortem: `list_vehicles` Routing Mask](../hauliage/docs/postmortem-fleet-api-response-mismatch-2026-04.md)
- [Fleet Postmortem: Migration Desynchronization via Manual Patching](../hauliage/docs/postmortem-lifeguard-migration-sync-failures-2026-04.md)
- [Architecture Decision Record: BRRTRouter Routing Strategy (ADR 0001)](../hauliage/docs/0001-brrtrouter-controller-routing-strategy.md)
