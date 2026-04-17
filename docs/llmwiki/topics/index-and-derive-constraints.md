# `#[index]` and derive constraints

- **Status**: `verified`
- **Source docs**: [`AGENT.md`](../../../AGENT.md) (historical home; rules now point here), [`lifeguard-derive/README.md`](../../../lifeguard-derive/README.md)
- **Code anchors**: `lifeguard-derive/src/attributes.rs`, migration SQL emission from `#[derive(LifeModel)]`
- **Last updated**: 2026-04-17

## What it is

Migration SQL is generated from `#[derive(LifeModel)]` structs. The `#[index = "idx_name(columns…)"]` string is parsed into `CREATE INDEX` DDL. **The derive does not verify that index columns exist on the struct** — invalid definitions fail at PostgreSQL apply time (`column does not exist`).

## Rules

1. Every column named inside `#[index = "…"]` must be a **field on that same struct**.
2. Child entities that only hold an FK to a parent **do not** have the parent’s columns — do not copy index definitions from the parent onto the child.
3. `#[indexed]` applies per field; the same “column must exist” rule applies.

## How bugs arise

- Copy-pasting `#[index]` from a base entity to a specialized entity that only stores `parent_id`.
- Renaming a field without updating the index string.

## Mitigation

Run migration generation and apply against a test database after index changes. A future improvement is compile-time validation in `lifeguard-derive` or validation in `lifeguard-migrate` when assembling table metadata.

## Cross-references

- [`documentation-landscape.md`](./documentation-landscape.md)
- [`lifeguard-derive` planning docs](../../../docs/planning/lifeguard-derive/EDGE_CASES_ATTRIBUTES.md) (attribute edge cases)
