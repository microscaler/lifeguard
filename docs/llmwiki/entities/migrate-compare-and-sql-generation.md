# `lifeguard-migrate`: SQL generation, ordering, compare-schema

- **Status**: `verified`
- **Source docs**: [`lifeguard-migrate/README.md`](../../../lifeguard-migrate/README.md), [`docs/planning/DESIGN_INDEX_COMPARE_ROADMAP.md`](../../planning/DESIGN_INDEX_COMPARE_ROADMAP.md)
- **Code anchors**: `lifeguard-migrate/src/sql_dependency_order.rs`, `lifeguard-migrate/src/schema_migration_compare.rs`, `lifeguard-migrate/src/sql_generator.rs`
- **Last updated**: 2026-04-17

## What it is

The **`lifeguard-migrate`** crate (CLI binary) generates **per-table SQL** from `LifeModel` definitions, maintains **`apply_order.txt`** (FK/view-aware topological order), **`seed_order.txt`** in consumers like Hauliage, and provides **`compare-schema`** to diff merged migration SQL against a live PostgreSQL catalog (indexes, columns, drift classes **T1–T4**, **T2b**, **T3**, etc. per roadmap).

## When to read source vs docs

- **Day-to-day usage**: [`lifeguard-migrate/README.md`](../../../lifeguard-migrate/README.md)
- **Index / expression / opclass drift theory**: [`DESIGN_INDEX_COMPARE_ROADMAP.md`](../../planning/DESIGN_INDEX_COMPARE_ROADMAP.md), [`DESIGN_INDEX_COMPARE_T2B_T3.md`](../../planning/DESIGN_INDEX_COMPARE_T2B_T3.md)
- **Infer-schema / codegen**: [`DESIGN_SCHEMA_INFERENCE_CLI_CODEGEN.md`](../../planning/DESIGN_SCHEMA_INFERENCE_CLI_CODEGEN.md)

## Cross-references

- [`topics/migrate-cli-integration.md`](../topics/migrate-cli-integration.md)
- Hauliage consumer: [`../../../../hauliage/docs/llmwiki/topics/seed-pipeline.md`](../../../../hauliage/docs/llmwiki/topics/seed-pipeline.md) (four levels up from `entities/` to `microscaler/`)
