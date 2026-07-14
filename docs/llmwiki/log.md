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

## [2026-04-18] ingest | JSF + Microsoft Pragmatic Rust references and wiki

- Added [`docs/references/`](../references/) — `jsf-writeup.md`, `jsf-audit-opinion.md`, `jsf-compliance.md`, `rust-guidelines.md` (same bundle as BRRTRouter / `microscaler-observability`).
- New topics: [`topics/coding-standards-jsf-inspired.md`](./topics/coding-standards-jsf-inspired.md), [`topics/pragmatic-rust-guidelines.md`](./topics/pragmatic-rust-guidelines.md).
- Updated [`../../AGENT.md`](../../AGENT.md) **Core rule §6**; expanded [`docs-catalog.md`](./docs-catalog.md) and [`index.md`](./index.md).
- Aligned [`../../clippy.toml`](../../clippy.toml) numeric thresholds with the platform JSF-inspired profile.

## [2026-05-04] feat | Story 6 — End-to-end RLS integration tests

- Created `tests/db_integration/rls_integration.rs` with 4 scenarios:
  - **Test A:** Direct executor filters rows via `MayPostgresExecutor::with_session_context`
  - **Test B:** Fail-closed (no context = 0 rows visible)
  - **Test C:** Transaction `begin_with_session` propagates context to all queries
  - **Test D:** Pool worker isolation — different contexts see different rows
- Fixed `rls_set_session` SQL function: `set_config(..., false)` for session-scoped
  GUC persistence (critical: `set_config(..., true)` is transaction-scoped and
  vanishes after autocommit in direct executor path).
- Fixed pool test: `rls_test_role` now has LOGIN + password; pool uses role URL
  so workers authenticate as non-superuser (superusers bypass RLS by default).
- Fixed test assertions: removed explicit `WHERE tenant = $X` queries (bypass
  RLS), replaced with full-count queries verifying visible row count.

## [2026-06-12] fix | TextParam — Value::String binds to JSON/JSONB with text::jsonb cast semantics

- Root cause (reported from Tiffany's WAL sink; likely the Hauliage JSONB-update
  failure mode too): `Value::String` was bound via `String`'s `ToSql`, which
  rejects `jsonb` params → "cannot convert between the Rust type
  `Option<String>` and the Postgres type `jsonb`". Callers had to write
  `($n::text)::jsonb` casts by hand.
- New [`src/value/text_param.rs`](../../src/value/text_param.rs) (`TextParam`,
  exported at crate root): TEXT-family unchanged; JSON/JSONB parses the string
  as a document (PostgreSQL `text::jsonb` semantics, bind-time error on invalid
  JSON). Wired into **both** dispatch paths: `query/converted_params.rs`
  (strings + null-strings buckets) and `pool/owned_param.rs`
  (`OwnedParam::String` now holds `TextParam` — construction sites internal).
- Tests: unit coverage in `text_param.rs` (wire-byte equality with native
  `serde_json::Value`, PG cast semantics for scalars, invalid-JSON error,
  typed NULLs) + regression tests on both dispatch paths. 492 lib tests green;
  clippy/fmt clean. Live-verified against Postgres 16 from Tiffany
  (`crates/executor/tests/wal_pg.rs`).
- Updated [`topics/postgres-scalars-uuid-chrono.md`](./topics/postgres-scalars-uuid-chrono.md)
  with the JSON/JSONB binding section.

## [2026-07-14] feat | readonly and generated PostgreSQL columns

- Completed `#[readonly]` / `#[generated]` write exclusion and insert hydration.
- Added `#[generated_always_as = "<expression>"]` metadata for both runtime
  schema creation and `lifeguard-migrate` SQL generation.
- Added live PostgreSQL coverage using a real `GENERATED ALWAYS ... STORED`
  column and documented the trusted compile-time expression boundary.

## [2026-07-14] fix | RLS helper contract and live integration coverage

- Schema-qualified every runtime call as `public.rls_set_session(...)` so
  application search paths cannot hide or redirect the helper.
- Made pooled execution fail closed when the helper is absent.
- Serialized shared test-fixture DDL across concurrent nextest discovery and
  consolidated the helper definition to prevent fixture drift.
- Corrected stale UUID and tenant fixtures; the serial DB suite passes 104/104.

## [2026-07-14] fix | transaction-local RLS context

- Replaced the interim session-scoped helper contract with transaction-local
  `set_config(..., true)` GUCs.
- Direct and pooled contextual one-shot operations now bind context injection
  and the application statement inside one short transaction, rolling back on
  any failure.
- Explicit transaction setup now rolls back if context serialization or
  injection fails, rather than returning with an open transaction.
- Added same-connection and single-pool-worker tests proving context is cleared
  before context-free work runs. This supersedes the earlier session-scoped
  workaround recorded above.
