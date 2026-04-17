# Lifeguard LLM Wiki — Log

Chronological, append-only. New entries at the bottom: `## [YYYY-MM-DD] <op> | <short>`.

---

## [2026-04-17] ingest | initial karpathy-pattern wiki scaffold

- Added `docs/llmwiki/` with `SCHEMA.md`, `README.md`, `index.md`, `log.md`, `docs-catalog.md`.
- Seeded `topics/index-and-derive-constraints.md`, `topics/brrtrouter-integration-pitfalls.md`, `topics/documentation-landscape.md` (content migrated from narrative sections of root `AGENT.md`).
- Rewrote [`AGENT.md`](../../AGENT.md) to strict agent instructions + explicit rule to read this wiki first.

## [2026-04-17] ingest | comprehensive wiki coverage (crates, docs, planning)

Expanded `docs/llmwiki/` so agents can route by subsystem without re-discovering `docs/planning/` layout.

- **Reference:** `reference/workspace-and-module-map.md`, `reference/planning-docs-index.md`
- **Entities:** `life-model-and-life-record`, `life-executor-pool-and-routing`, `migrate-compare-and-sql-generation`
- **Topics:** query/active model, relations/scopes, session, reflector/cache, postgres scalars, observability, migrate CLI, derive macros, integration testing — plus existing index/BRRTRouter pitfall topics
- **`docs-catalog.md`:** added **LLM wiki synthesis index** table mapping pages → coverage
- **`index.md`:** full catalog replaces the short stub
- **`documentation-landscape.md`:** links to new reference pages

## [2026-04-17] ingest | transaction + GraphQL wiki pages

- Added [`entities/transaction-boundaries.md`](./entities/transaction-boundaries.md) and [`topics/graphql-optional-feature.md`](./topics/graphql-optional-feature.md); updated [`index.md`](./index.md).
