# `lifeguard-derive`: macros and attributes

- **Status**: `verified`
- **Source docs**: [`lifeguard-derive/README.md`](../../../lifeguard-derive/README.md), [`docs/planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md`](../../planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md)
- **Code anchors**: `lifeguard-derive/src/attributes.rs`, `lifeguard-derive/src/life_model.rs`, `lifeguard-derive/src/life_record.rs`
- **Last updated**: 2026-07-14

## What it is

**`#[derive(LifeModel, LifeRecord)]`** expands to model/entity traits, `Column` enums, relation metadata, and migration hints consumed by **`lifeguard-migrate`**.

## Attribute surface (non-exhaustive)

- Table: `#[table_name]`, soft delete, scopes
- Columns: `#[primary_key]`, `#[column_type]`, `#[default_expr]`, JSON, indexes
- Database-managed columns: `#[readonly]` / `#[generated]` exclude a field from
  `INSERT` and `UPDATE` while hydrating it through `RETURNING` after inserts.
  `#[generated_always_as = "<expression>"]` implies `#[readonly]` and additionally
  emits PostgreSQL `GENERATED ALWAYS AS (<expression>) STORED` migration metadata.
  It cannot be combined with a default. The expression is trusted compile-time
  schema input and must never contain request data.
- Relations: `#[has_many]`, `#[belongs_to]`, `#[has_one]`, composite keys

## Cross-references

- [`docs/adr/0001-generated-column-expressions.md`](../../adr/0001-generated-column-expressions.md)
- [`topics/index-and-derive-constraints.md`](./index-and-derive-constraints.md)
- [`entities/life-model-and-life-record.md`](../entities/life-model-and-life-record.md)
- Edge-case catalogs: [`docs/planning/lifeguard-derive/EDGE_CASES_ATTRIBUTES.md`](../../planning/lifeguard-derive/EDGE_CASES_ATTRIBUTES.md)
