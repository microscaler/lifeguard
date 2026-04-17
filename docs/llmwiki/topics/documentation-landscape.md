# Documentation landscape

- **Status**: `verified`
- **Source docs**: [`README.md`](../../../README.md), [`docs/planning/README.md`](../../planning/README.md)
- **Code anchors**: n/a
- **Last updated**: 2026-04-17

## What it is

Lifeguard spreads documentation across **root markdown** (narrative + ops), **`docs/`** (deep references and postmortems), **`docs/planning/`** (PRDs, designs, audits — large tree), **crate READMEs**, and **`book/`** (mdBook). The wiki sits under `docs/llmwiki/` and links outward.

## Where to look first

| Question | Start here |
|----------|------------|
| How do I run tests / CI Postgres? | [`docs/TEST_INFRASTRUCTURE.md`](../../TEST_INFRASTRUCTURE.md) |
| UUID / chrono / Postgres types? | [`docs/UUID_AND_POSTGRES_TYPES.md`](../../UUID_AND_POSTGRES_TYPES.md), [`docs/CHRONO_AND_POSTGRES_TYPES.md`](../../CHRONO_AND_POSTGRES_TYPES.md) |
| Pooling / replicas | [`docs/POOLING_OPERATIONS.md`](../../POOLING_OPERATIONS.md), [`docs/planning/PRD_CONNECTION_POOLING.md`](../../planning/PRD_CONNECTION_POOLING.md) |
| Derive / SeaORM mapping | [`docs/planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md`](../../planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md) |
| Compare-schema / index drift | [`docs/planning/DESIGN_INDEX_COMPARE_ROADMAP.md`](../../planning/DESIGN_INDEX_COMPARE_ROADMAP.md), `lifeguard-migrate` README |
| Published book | [`book/src/SUMMARY.md`](../../../book/src/SUMMARY.md) |

## Gotchas

> **Drift:** Some root files (e.g. `OBSERVABILITY.md` vs `docs/OBSERVABILITY.md`) overlap; prefer the path linked from `README.md` for your task and check dates.

## Cross-references

- [`docs-catalog.md`](../docs-catalog.md) — full inventory tables.
- [`index.md`](../index.md) — wiki page catalog.
