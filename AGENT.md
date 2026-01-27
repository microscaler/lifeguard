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
