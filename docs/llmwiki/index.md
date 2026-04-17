# Lifeguard LLM Wiki — Index

Content catalog for the Lifeguard llm-wiki. Every page has a one-line summary. See [`SCHEMA.md`](./SCHEMA.md).

## Core operational

- [`SCHEMA.md`](./SCHEMA.md) — Source-of-truth order, page conventions, ingest / query / lint, agent workflow.
- [`log.md`](./log.md) — Chronological append-only activity log.
- [`docs-catalog.md`](./docs-catalog.md) — Inventory of `docs/`, root markdown, book, and crate READMEs.

## Topics

- [`topics/index-and-derive-constraints.md`](./topics/index-and-derive-constraints.md) — `#[index]` / `#[indexed]` must reference real struct fields; child entities do not inherit parent columns.
- [`topics/brrtrouter-integration-pitfalls.md`](./topics/brrtrouter-integration-pitfalls.md) — UUID vs `String` row decode, `register_from_spec` ordering, empty `[]` API symptoms (stacks using BRRTRouter + Lifeguard).
- [`topics/documentation-landscape.md`](./topics/documentation-landscape.md) — Where reference vs planning vs narrative docs live; how to navigate `docs/planning/`.

## Entities

*(None yet — add `entities/<slug>.md` when a concept deserves a stable reference page, e.g. `LifeModel`, `SelectQuery`, compare-schema pipeline.)*

## Planned

- [ ] `entities/compare-schema-pipeline.md` — link `docs/planning/DESIGN_INDEX_COMPARE_*.md` to `lifeguard-migrate` modules.
- [ ] `topics/pool-and-replicas.md` — consolidate `POOLING_OPERATIONS.md`, `PRD_CONNECTION_POOLING.md`, read-replica testing PRD.

## Cross-references

- **Hauliage** (downstream consumer patterns, migrations in apps): [`../../../hauliage/docs/llmwiki/`](../../../hauliage/docs/llmwiki/) when both repos exist side-by-side under `microscaler/`.
- **Planning index:** [`../planning/README.md`](../planning/README.md).
