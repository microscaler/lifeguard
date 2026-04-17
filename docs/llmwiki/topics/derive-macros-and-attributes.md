# `lifeguard-derive`: macros and attributes

- **Status**: `verified`
- **Source docs**: [`lifeguard-derive/README.md`](../../../lifeguard-derive/README.md), [`docs/planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md`](../../planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md)
- **Code anchors**: `lifeguard-derive/src/attributes.rs`, `lifeguard-derive/src/life_model.rs`, `lifeguard-derive/src/life_record.rs`
- **Last updated**: 2026-04-17

## What it is

**`#[derive(LifeModel, LifeRecord)]`** expands to model/entity traits, `Column` enums, relation metadata, and migration hints consumed by **`lifeguard-migrate`**.

## Attribute surface (non-exhaustive)

- Table: `#[table_name]`, soft delete, scopes
- Columns: `#[primary_key]`, `#[column_type]`, `#[default_expr]`, JSON, indexes
- Relations: `#[has_many]`, `#[belongs_to]`, `#[has_one]`, composite keys

## Cross-references

- [`topics/index-and-derive-constraints.md`](./index-and-derive-constraints.md)
- [`entities/life-model-and-life-record.md`](../entities/life-model-and-life-record.md)
- Edge-case catalogs: [`docs/planning/lifeguard-derive/EDGE_CASES_ATTRIBUTES.md`](../../planning/lifeguard-derive/EDGE_CASES_ATTRIBUTES.md)
