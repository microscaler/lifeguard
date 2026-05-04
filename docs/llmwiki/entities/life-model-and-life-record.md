# Entity concepts: `LifeModel` and `LifeRecord`

- **Status**: `verified`
- **Source docs**: [`docs/planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md`](../../planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md), [`docs/UUID_AND_POSTGRES_TYPES.md`](../../UUID_AND_POSTGRES_TYPES.md)
- **Code anchors**: `lifeguard-derive/src/life_model.rs`, `lifeguard-derive/src/life_record.rs`, `lifeguard/src/model/`, `lifeguard/src/active_model/`
- **Last updated**: 2026-04-17

## What it is

**`LifeModel`** is the immutable row type (query results); **`LifeRecord`** is the mutable insert/update surface (`ActiveModel`-style). Both are produced by `#[derive(LifeModel, LifeRecord)]` on a struct with `#[table_name = "…"]` and column attributes.

## Rules agents must not violate

- PostgreSQL **`UUID`** columns → Rust **`uuid::Uuid`** (not `String`). See [`topics/postgres-scalars-uuid-chrono.md`](../topics/postgres-scalars-uuid-chrono.md).
- **`#[index]`** strings must only name columns on **this** struct — see [`topics/index-and-derive-constraints.md`](../topics/index-and-derive-constraints.md).

## Where it lives

- Derive expansion: `lifeguard-derive/`
- Runtime traits: `lifeguard/src/model/`, `lifeguard/src/active_model/`, `lifeguard/src/query/` (traits used by generated code)

## Cross-references

- [`topics/query-select-and-active-model.md`](../topics/query-select-and-active-model.md)
- [`topics/derive-macros-and-attributes.md`](../topics/derive-macros-and-attributes.md)
