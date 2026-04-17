# Lifeguard source document catalog

Inventory of **raw sources** the wiki synthesizes. The wiki does not replace these files; it links and summarizes them.

## Root-level narrative and operations

| Path | Role |
|------|------|
| [`README.md`](../../README.md) | Pitch, getting started, links into docs and book. |
| [`ARCHITECTURE.md`](../../ARCHITECTURE.md) | Diagrams and call flows. |
| [`VISION.md`](../../VISION.md) | Product vision and roadmap narrative. |
| [`COMPARISON.md`](../../COMPARISON.md) | Ecosystem / parity; **repository status** anchor. |
| [`DEVELOPMENT.md`](../../DEVELOPMENT.md) | Clippy, pre-commit, `just`, dev workflow. |
| [`CONTRIBUTING.md`](../../CONTRIBUTING.md) | Contribution expectations. |
| [`ROADMAP.md`](../../ROADMAP.md) | High-level roadmap. |
| [`CHANGELOG.md`](../../CHANGELOG.md) | Release notes. |
| [`OBSERVABILITY.md`](../../OBSERVABILITY.md) | Metrics and tracing overview (duplicate of shorter `docs/OBSERVABILITY.md` in places — prefer rustdoc + `docs/OBSERVABILITY_APP_INTEGRATION.md` for app wiring). |
| [`SECURITY_PROMPT.md`](../../SECURITY_PROMPT.md) | Security review prompts. |
| [`LIFEGUARD_GAP_ANALYSIS.md`](../../LIFEGUARD_GAP_ANALYSIS.md), [`LIFEGUARD_BLOG_POST.md`](../../LIFEGUARD_BLOG_POST.md) | Narrative / marketing. |

## `docs/` — operational reference and investigations

| Path | Role |
|------|------|
| [`docs/TEST_INFRASTRUCTURE.md`](../TEST_INFRASTRUCTURE.md) | Postgres/Redis/Kind/Compose for tests and CI. |
| [`docs/UUID_AND_POSTGRES_TYPES.md`](../UUID_AND_POSTGRES_TYPES.md) | UUID and scalar mapping guidance. |
| [`docs/CHRONO_AND_POSTGRES_TYPES.md`](../CHRONO_AND_POSTGRES_TYPES.md) | Chrono ↔ PostgreSQL alignment. |
| [`docs/postmortem-lifeguard-derive-naivedate-chronodate-2026-04.md`](../postmortem-lifeguard-derive-naivedate-chronodate-2026-04.md) | NaiveDate/ChronoDate bind mismatch postmortem. |
| [`docs/OBSERVABILITY_APP_INTEGRATION.md`](../OBSERVABILITY_APP_INTEGRATION.md) | Host-owned OTel/subscriber wiring. |
| [`docs/POOLING_OPERATIONS.md`](../POOLING_OPERATIONS.md), [`docs/POOL_TCP_KEEPALIVE.md`](../POOL_TCP_KEEPALIVE.md) | Pool behavior notes. |
| [`docs/PERF_ORM.md`](../PERF_ORM.md) | ORM performance notes. |
| [`docs/LIFEGUARD_FINAL_PRD.md`](../LIFEGUARD_FINAL_PRD.md) | Historical / umbrella PRD (large). |

## `docs/planning/` — designs, PRDs, audits

Indexed in [`docs/planning/README.md`](../planning/README.md). Notable clusters:

| Area | Examples |
|------|----------|
| Connection pooling | `PRD_CONNECTION_POOLING.md`, `DESIGN_CONNECTION_POOLING.md` |
| Index compare / schema | `DESIGN_INDEX_COMPARE_ROADMAP.md`, `DESIGN_INDEX_COMPARE_T2B_T3.md`, `DESIGN_SCHEMA_INFERENCE_CLI_CODEGEN.md` |
| Relations / scopes | `DESIGN_FIND_RELATED_SCOPES.md`, `PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md` |
| lifeguard-derive | `lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md`, `AUTHORING_MODEL_TRAIT.md`, `EDGE_CASES_*.md` |
| Audits | `audits/PRD_JSF_PANIC_SAFETY.md`, `audits/MIGRATION_AUDIT.md`, … |

## Crate READMEs

| Path | Role |
|------|------|
| [`lifeguard-migrate/README.md`](../../lifeguard-migrate/README.md) | Migration CLI, compare-schema, infer-schema. |
| [`lifeguard-derive/README.md`](../../lifeguard-derive/README.md) | Derive crate overview. |
| [`lifeguard-codegen/README.md`](../../lifeguard-codegen/README.md) | Codegen utilities. |
| [`lifeguard-reflector/README.md`](../../lifeguard-reflector/README.md) | Redis / NOTIFY adjunct. |
| [`examples/entities/README.md`](../../examples/entities/README.md) | Entity examples + migration generation. |
| [`tests-integration/README.md`](../../tests-integration/README.md) | Integration test layout. |
| [`migrations/README.md`](../../migrations/README.md) | Original vs generated migration trees. |

## `book/` — mdBook

| Path | Role |
|------|------|
| [`book/src/SUMMARY.md`](../../book/src/SUMMARY.md) | mdBook outline. |
| [`book/src/intro.md`](../../book/src/intro.md), [`usage.md`](../../book/src/usage.md), [`architecture.md`](../../book/src/architecture.md), [`performance.md`](../../book/src/performance.md), [`dashboards.md`](../../book/src/dashboards.md) | Published book chapters. |

## Sibling repos (consumers)

When debugging app-level issues, Hauliage and BRRTRouter carry their own wikis and ADRs:

| Location | Role |
|----------|------|
| `../../../hauliage/docs/llmwiki/` | App migrations, seeds, BFF patterns. |
| `../../../BRRTRouter/llmwiki/` | Router, OpenAPI validation, codegen templates. |

Paths assume a `microscaler/` checkout with `lifeguard`, `hauliage`, and `BRRTRouter` as siblings.

## LLM wiki synthesis index (pages → what they cover)

Use this when you know **which subsystem** you are touching; each page links back to raw docs above.

| Wiki page | Maps to |
|-----------|---------|
| [`topics/documentation-landscape.md`](./topics/documentation-landscape.md) | Where to look in `docs/`, `book/`, planning |
| [`reference/workspace-and-module-map.md`](./reference/workspace-and-module-map.md) | Workspace crates, `src/lib.rs` modules |
| [`reference/planning-docs-index.md`](./reference/planning-docs-index.md) | `docs/planning/` clusters (PRDs, compare-schema, derive) |
| [`entities/life-model-and-life-record.md`](./entities/life-model-and-life-record.md) | `LifeModel`/`LifeRecord`, UUID typing |
| [`entities/life-executor-pool-and-routing.md`](./entities/life-executor-pool-and-routing.md) | Pool, WAL routing, `LifeExecutor` |
| [`entities/migrate-compare-and-sql-generation.md`](./entities/migrate-compare-and-sql-generation.md) | `lifeguard-migrate`, compare-schema, SQL gen |
| [`topics/query-select-and-active-model.md`](./topics/query-select-and-active-model.md) | `SelectQuery`, validators, `ActiveModel` |
| [`topics/relations-loaders-scopes.md`](./topics/relations-loaders-scopes.md) | Relations, loaders, scopes |
| [`topics/session-identity-map.md`](./topics/session-identity-map.md) | Session / identity map |
| [`topics/reflector-cache-and-coherence.md`](./topics/reflector-cache-and-coherence.md) | LifeReflector, Redis, `cache` traits |
| [`topics/postgres-scalars-uuid-chrono.md`](./topics/postgres-scalars-uuid-chrono.md) | UUID + chrono scalar rules |
| [`topics/observability-and-logging.md`](./topics/observability-and-logging.md) | Metrics, tracing, channel logs |
| [`topics/migrate-cli-integration.md`](./topics/migrate-cli-integration.md) | CLI usage in apps / CI |
| [`topics/derive-macros-and-attributes.md`](./topics/derive-macros-and-attributes.md) | `lifeguard-derive` surface |
| [`topics/integration-testing-and-ci.md`](./topics/integration-testing-and-ci.md) | `TEST_INFRASTRUCTURE`, test helpers |
| [`topics/index-and-derive-constraints.md`](./topics/index-and-derive-constraints.md) | `#[index]` footguns |
| [`topics/brrtrouter-integration-pitfalls.md`](./topics/brrtrouter-integration-pitfalls.md) | Empty `[]` + BRRTRouter stacks |
