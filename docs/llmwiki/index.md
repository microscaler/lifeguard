# Lifeguard LLM Wiki — Index

Content catalog for the Lifeguard llm-wiki. See [`SCHEMA.md`](./SCHEMA.md).

## Core operational

- [`SCHEMA.md`](./SCHEMA.md) — Source-of-truth order, page conventions, ingest / query / lint, agent workflow.
- [`log.md`](./log.md) — Chronological append-only activity log.
- [`docs-catalog.md`](./docs-catalog.md) — Raw doc inventory + **wiki synthesis index** table.

## Reference (navigation)

- [`reference/workspace-and-module-map.md`](./reference/workspace-and-module-map.md) — Workspace crates; `lifeguard` `src/lib.rs` module map.
- [`reference/planning-docs-index.md`](./reference/planning-docs-index.md) — Router into `docs/planning/` (PRDs, compare-schema, derive).

## Entities (stable concepts)

- [`entities/life-model-and-life-record.md`](./entities/life-model-and-life-record.md) — `LifeModel` / `LifeRecord`, UUID typing.
- [`entities/life-executor-pool-and-routing.md`](./entities/life-executor-pool-and-routing.md) — `LifeExecutor`, `LifeguardPool`, WAL / replicas.
- [`entities/migrate-compare-and-sql-generation.md`](./entities/migrate-compare-and-sql-generation.md) — `lifeguard-migrate`, ordering, compare-schema.
- [`entities/transaction-boundaries.md`](./entities/transaction-boundaries.md) — `Transaction`, isolation, error type; pointer to rustdoc.

## Topics (cross-cutting)

- [`topics/documentation-landscape.md`](./topics/documentation-landscape.md) — Where root vs `docs/` vs `book/` vs planning live.
- [`topics/query-select-and-active-model.md`](./topics/query-select-and-active-model.md) — `SelectQuery`, `ActiveModel`, validators.
- [`topics/relations-loaders-scopes.md`](./topics/relations-loaders-scopes.md) — Relations, loaders, `find_related`, scopes.
- [`topics/session-identity-map.md`](./topics/session-identity-map.md) — Session / identity map / UoW direction.
- [`topics/reflector-cache-and-coherence.md`](./topics/reflector-cache-and-coherence.md) — LifeReflector, Redis, cache traits.
- [`topics/postgres-scalars-uuid-chrono.md`](./topics/postgres-scalars-uuid-chrono.md) — UUID + chrono ↔ Postgres.
- [`topics/observability-and-logging.md`](./topics/observability-and-logging.md) — Metrics, tracing, channel logging.
- [`topics/migrate-cli-integration.md`](./topics/migrate-cli-integration.md) — Running migrate in apps and CI.
- [`topics/derive-macros-and-attributes.md`](./topics/derive-macros-and-attributes.md) — `lifeguard-derive` attributes.
- [`topics/integration-testing-and-ci.md`](./topics/integration-testing-and-ci.md) — `TEST_INFRASTRUCTURE`, test helpers.
- [`topics/index-and-derive-constraints.md`](./topics/index-and-derive-constraints.md) — `#[index]` / `#[indexed]` constraints.
- [`topics/brrtrouter-integration-pitfalls.md`](./topics/brrtrouter-integration-pitfalls.md) — BRRTRouter + Lifeguard footguns.
- [`topics/graphql-optional-feature.md`](./topics/graphql-optional-feature.md) — `graphql` feature + `async_graphql` version alignment.

## Planned (optional next passes)

- [ ] Deeper rustdoc extracts for `stream_all` txn cleanup (if session work expands).

## Cross-references

- **Hauliage:** [`../../../hauliage/docs/llmwiki/`](../../../hauliage/docs/llmwiki/)
- **BRRTRouter:** [`../../../BRRTRouter/llmwiki/`](../../../BRRTRouter/llmwiki/)
- **Planning root:** [`../planning/README.md`](../planning/README.md)
