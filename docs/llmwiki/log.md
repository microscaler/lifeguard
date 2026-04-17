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

## [2026-04-17] doc | GraphQL optional feature — platform decision

- Rewrote top of [`topics/graphql-optional-feature.md`](./topics/graphql-optional-feature.md): Hauliage BFF uses OpenAPI/BRRTRouter composed views — **not** GraphQL; optional `graphql` / `async_graphql` is legacy/frozen for existing cfg/tests.
- Updated [`index.md`](./index.md), [`docs-catalog.md`](./docs-catalog.md); aligned root [`README.md`](../../README.md), [`src/lib.rs`](../../src/lib.rs) comments, [`Cargo.toml`](../../Cargo.toml), [`LIFEGUARD_GAP_ANALYSIS.md`](../../LIFEGUARD_GAP_ANALYSIS.md) §6, [`SECURITY_PROMPT.md`](../../SECURITY_PROMPT.md).

## [2026-04-17] ingest | raw SQL vs SelectQuery policy topic

- Added [`topics/raw-sql-vs-selectquery-policy.md`](./topics/raw-sql-vs-selectquery-policy.md): raw SQL last resort; **human approval**; **ADR** must show idiomatic ORM extension is infeasible. Linked from [`topics/query-select-and-active-model.md`](./topics/query-select-and-active-model.md), [`reference/workspace-and-module-map.md`](./reference/workspace-and-module-map.md); updated [`index.md`](./index.md), [`docs-catalog.md`](./docs-catalog.md).

## [2026-04-17] doc | AGENT.md — raw SQL rule

- Added **Core rule §5** to [`../../AGENT.md`](../../AGENT.md) (mirror of raw-SQL policy; links to [`topics/raw-sql-vs-selectquery-policy.md`](./topics/raw-sql-vs-selectquery-policy.md)).
