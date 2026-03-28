# Lifeguard foundation work: continuation, completion & hardening

**Audience:** Engineers extending Lifeguard toward [SeaORM parity](../../../LIFEGUARD_GAP_ANALYSIS.md).  
**Purpose:** Turn **foundation** work (correct relation filtering, unified DB test harness, streaming tests in-suite) into **completed, robust** capabilities—without claiming closure of unrelated gaps (§3 CLI, §4 schema diff, §6 GraphQL) unless explicitly in scope.

---

## 0. What “complete” means for each foundation thread

| Foundation thread | Incomplete today | **Complete** = (acceptance) |
|-------------------|------------------|----------------------------|
| **Relation SQL** (`build_where_condition`) | PK fallback + `Expr::cust`; limited identifier tests | Documented contract; `get_by_column_name` path primary for all derived models; optional `sea_query` column refs where needed; composite + BelongsTo + HasMany covered by unit + integration tests |
| **`FindRelated` / `LazyLoader` parity** | Lazy path shares builder; Lazy has weak runtime test coverage | Same predicates verified by SQL or row-level integration tests for both paths; public rustdoc for `Related::to()` orientation |
| **`find_linked` (multi-hop)** | Two-hop only; filter uses first hop only | Intended SQL documented; integration test asserts join + filter shape; roadmap entry for N-hop if product requires it |
| **DB integration harness** | Parallel tests can race DDL or exhaust connections | CI uses a defined profile (serial or pooled or isolated DB); env vars documented; predictable behavior with `just dev-up` + tests |
| **§5 validation (cursor/stream)** | Tests exist; gap doc wants richer APIs later | Current APIs stay regression-protected; follow-up epic for `stream_all` / server cursor is scoped separately |

---

## 1. Audit: gap sections vs current parity

| Gap (see [`LIFEGUARD_GAP_ANALYSIS.md`](../../../LIFEGUARD_GAP_ANALYSIS.md)) | Parity status | Foundation touched by recent work? |
|--------------------------------------|---------------|-------------------------------------|
| **§1** Nested object graph / `save_graph` | Not implemented | Harness only: `active_model_graph` in `db_integration_suite` |
| **§2** DataLoader / `.with()` N+1 avoidance | Not implemented | **Yes:** `build_where_condition`, `FindRelated`, `LazyLoader` share correct related-table filtering |
| **§3** Reverse-engineering CLI | Not implemented | No |
| **§4** Entity-first DDL / schema diff | Not implemented | No |
| **§4.1** RLS policies | Not implemented | No |
| **§5** Cursor pagination & channel streaming | Partially present; gap doc asks for more | **Yes:** `stream_and_cursor` tests + shared PG per process |
| **§6** GraphQL bridges | Not implemented | No |
| **Test infrastructure** | Improved | **Yes:** `tests/context.rs`, `db_integration_suite`, container cleanup |

---

## 2. Phased plan: complete the foundation

Dependencies: **A → B** (loaders must trust relation layer); **A + C** can overlap; **D** after A is stable; **E** is optional stretch for §5.

### Phase A — Relation layer contract & hardening (prerequisite)

| # | Task | Deliverable | Robustness / hardening |
|---|------|-------------|-------------------------|
| A1 | Rustdoc on `build_where_condition`, `Related::to()`, `FindRelated`, `LazyLoader` | Clear statement: `from_tbl` = entity implementing `Related`, `to_tbl` = related `SelectQuery` target; `from_col` / `to_col` zip semantics | Reduces misuse in hand-written `RelationDef` |
| A2 | Unit tests: BelongsTo + HasMany + composite FK/PK (minimal mock `ModelTrait`) | All paths use `to_tbl.to_col` + correct value source | Lock behavior before loader work |
| A3 | Integration: `related_trait` + `dataloader_n_plus_one` run clean under **documented** CI settings | No flaky DDL races in default pipeline | See Phase C |
| A4 | Review `Expr::cust("table.col")` vs `Expr::col((table, col))` | Decision recorded; if migrate, add quoted-identifier tests | Hardening against reserved words / schema |
| A5 | Derive guarantee: `get_by_column_name` covers all columns used in generated `Identity` | Macro test or codegen audit | Makes PK fallback truly “stub-only” |

**Exit gate:** `cargo test -p lifeguard --lib relation::` green; `related_trait` integration tests green under Phase C profile (or `--test-threads=1` until `db-serial` exists).

---

### Phase A — setup tasks (trackable backlog)

Use this section as the working checklist. Status: unchecked = not started; check when merged to `main` (or your release branch).

#### A1 — Public contract (rustdoc)

| ID | Task | Primary locations | Done when |
|----|------|-------------------|-----------|
| A1.1 | Expand module-level docs on `RelationDef` orientation (`from_tbl` / `to_tbl` / `from_col` / `to_col`) for hand-written `impl Related` | `src/relation/def/struct_def.rs`, `src/relation/traits.rs` (`Related`) | Examples for **BelongsTo** (post→user) and **HasMany** (user→post) in rustdoc; links to `build_where_condition` |
| A1.2 | Document `build_where_condition`: value sources (`get_by_column_name`, PK fallback), panics, composite arity | `src/relation/def/condition.rs` | `cargo doc -p lifeguard --no-deps` renders clear “Panics” + “Example” or pointer to integration tests |
| A1.3 | Document `FindRelated::find_related` query shape (FROM = related entity only) | `src/relation/traits.rs` | One short example + warning not to qualify WHERE with `from_tbl` alone |
| A1.4 | Document `LazyLoader::load` as same predicate semantics as `find_related` | `src/relation/lazy.rs` | Cross-link to `build_where_condition` |

Checklist:

- [x] **A1.1** — `Related` / `RelationDef` orientation rustdoc
- [x] **A1.2** — `build_where_condition` rustdoc (values + panics)
- [x] **A1.3** — `FindRelated` rustdoc
- [x] **A1.4** — `LazyLoader` rustdoc + cross-links

#### A2 — Unit tests (`build_where_condition` + mocks)

| ID | Task | Primary locations | Done when |
|----|------|-------------------|-----------|
| A2.1 | Add unit test: **BelongsTo**-shaped `RelationDef` + mock model with FK only via `get_by_column_name` (no PK fallback) | `src/relation/def/condition.rs` (`#[cfg(test)]`) | Assert generated condition references **only** `to_tbl` + `to_col`; value matches FK |
| A2.2 | Add unit test: **HasMany**-shaped `RelationDef` + mock model using **PK fallback** (no `get_by_column_name` for `id`) | same | Assert `to_tbl.fk = pk_value` |
| A2.3 | Add unit test: **composite** `from_col` / `to_col` (arity 2) + mock with two FK/PK columns | same | Two conjuncts, both on `to_tbl` columns |
| A2.4 | Optional: build minimal `SelectQuery` + `PostgresQueryBuilder` string assert for one case | `src/relation/def/condition.rs` or `src/relation/traits.rs` tests | SQL substring checks for table-qualified related columns |

Checklist:

- [x] **A2.1** — BelongsTo mock unit test
- [x] **A2.2** — HasMany + PK fallback unit test
- [x] **A2.3** — Composite arity-2 unit test
- [ ] **A2.4** — (Optional) SQL string smoke assert *(A2.1–A2.3 already assert SQL via `Query::build`)*

#### A3 — Integration / CI note (blocked partially on Phase C)

| ID | Task | Primary locations | Done when |
|----|------|-------------------|-----------|
| A3.1 | Run and record: `cargo test -p lifeguard --test db_integration_suite related_trait:: dataloader_n_plus_one:: -- --test-threads=1` | local / CI | Logged in PR or `docs/TEST_INFRASTRUCTURE.md` as recommended command |
| A3.2 | If CI runs full `db_integration_suite` in parallel, add note: “unsafe for shared Postgres until Phase C profile” | `docs/TEST_INFRASTRUCTURE.md` or CI config comment | Prevents false failures on `max_connections` / DDL races |

Checklist:

- [x] **A3.1** — Document serial command for relation-heavy modules (`docs/TEST_INFRASTRUCTURE.md`)
- [x] **A3.2** — Document parallel/shared-Postgres caveat + `db-serial` profile (see Phase C1 early delivery)

#### A4 — `Expr::cust` vs typed column refs

| ID | Task | Primary locations | Done when |
|----|------|-------------------|-----------|
| A4.1 | Spike: replace one `build_where_condition` clause with `sea_query` `Expr::col` + table ref if API allows | `src/relation/def/condition.rs` | PR or short ADR in `docs/planning/audits/` with **Decision:** keep cust / migrate / partial |
| A4.2 | If **keep `Expr::cust`**: add comment in code + test with table name that needs quoting (e.g. reserved word) **or** document “unsupported: quoted identifiers” | `condition.rs` + test or rustdoc | Explicit product stance |
| A4.3 | If **migrate**: add regression test for quoted / schema-qualified table | tests | Green on Postgres |

Checklist:

- [x] **A4.1** — ADR recorded: [`RELATION_WHERE_EXPR_DECISION.md`](RELATION_WHERE_EXPR_DECISION.md) (**keep `Expr::cust`**)
- [x] **A4.2** — Code comment + ADR “reserved identifiers / schema” stance (quoted identifiers deferred)

#### A5 — Derive: `get_by_column_name` completeness

| ID | Task | Primary locations | Done when |
|----|------|-------------------|-----------|
| A5.1 | Audit `lifeguard-derive` generated `get_by_column_name` match arms vs struct fields | `lifeguard-derive/src/macros/life_model.rs` (or equivalent) | Codegen walks each model field → one arm each (ongoing review when adding attributes) |
| A5.2 | Add **trybuild** or **macro test** that expands a model with composite PK + FK and asserts generated `get_by_column_name` includes each DB column name used in `Identity` | `lifeguard-derive/tests/` or `lifeguard` integration | Fails if a column is missing from match |
| A5.3 | Document in derive book / rustdoc: “custom `ModelTrait` must implement `get_by_column_name` for every `from_col` used in `Related`” | `docs/planning/lifeguard-derive/` or crate-level doc | Links from `build_where_condition` rustdoc |

Checklist:

- [x] **A5.1** — Codegen audit (match arms generated per field in `life_model.rs`)
- [x] **A5.2** — `test_minimal.rs`: `get_by_column_name` for `User` + `#[column_name]` model
- [x] **A5.3** — [`AUTHORING_MODEL_TRAIT.md`](../lifeguard-derive/AUTHORING_MODEL_TRAIT.md)

#### Phase A — rollup checklist (quick scan)

- [x] **A1** — All A1.1–A1.4 complete
- [x] **A2** — A2.1–A2.3 complete (A2.4 optional)
- [x] **A3** — A3.1–A3.2 complete (+ **Phase C1** `db-serial` profile landed early)
- [x] **A4** — Decision doc + chosen hardening path
- [x] **A5** — A5.1–A5.3 complete
- [x] **Exit gate (library):** `cargo test -p lifeguard --lib relation::` (67 tests, incl. new `build_where_condition` cases)
- [ ] **Exit gate (db_integration_suite):** run with `--profile db-serial` or `--test-threads=1` when Postgres / Docker is available

---

### Phase B — §2 continuation: batched loading & `.with()`-style API (MVP)

| # | Task | Deliverable | Robustness / hardening |
|---|------|-------------|-------------------------|
| B1 | Extract **value resolution** from `RelationDef` + `ModelTrait` (shared helper next to `build_where_condition`) | Single function used by `find_related`, lazy load, and batch loader | No duplicate FK/PK logic |
| B2 | Batch API: given `Vec<ParentModel>` + `RelationDef`, compute `IN` / tuple `IN` predicate on `to_col` (composite) | One query per relation hop for list endpoints | Document max parameter count; chunk if needed |
| B3 | `SelectQuery` extension: `.with(Relation)` (name TBD) stores relation steps | Executes parent query, then batch loads children, attaches to result type or side map | Explicit error type (not panic) for missing `get_by_column_name` in batch context |
| B4 | Reuse `lifeguard::relation::eager::load_related` where possible | Less new code; consistent grouping | Property tests or golden tests on batch boundaries (empty set, single parent, many parents) |

**Exit gate:** N+1 integration test (or new one) demonstrates **two-query** (or bounded) pattern vs naive loop; documented in `docs/` or rustdoc example.

---

### Phase C — Test harness: complete robustness

| # | Task | Deliverable | Robustness / hardening |
|---|------|-------------|-------------------------|
| C1 | `.config/nextest.toml` profile `db-serial` (or equivalent): `test-threads = 1` for `binary(db_integration_suite)` | **Done (2026-03):** profile `db-serial` + override for `db_integration_suite` | Eliminates DDL race on shared DB |
| C2 | Document env vars in [`docs/TEST_INFRASTRUCTURE.md`](../../TEST_INFRASTRUCTURE.md) | `DATABASE_URL`, `TEST_DATABASE_URL`, `TEST_REDIS_URL`, `REDIS_URL`, when Docker is required | On-call clarity |
| C3 | Optional: shared `MayPostgresExecutor` / one connection per test **module** via `lazy_static` + mutex | Reduces “too many clients” against `just dev-up` | Trade-off: slower tests; document |
| C4 | CI job matrix: **testcontainers** job vs **service Postgres** job | Both paths verified weekly or on main | Catches drift in `context.rs` |
| C5 | `ctor::dtor` cleanup: document failure mode + `docker ps` hygiene | Runbook snippet | Operational hardening |

**Exit gate:** `cargo nextest run -p lifeguard --profile db-serial` (or documented `just` target) passes with `DATABASE_URL` pointing at shared dev Postgres without connection exhaustion in typical runs.

---

### Phase D — `find_linked` completion (within current 2-hop design)

| # | Task | Deliverable | Robustness / hardening |
|---|------|-------------|-------------------------|
| D1 | Integration test: SQL or `EXPLAIN` snapshot for User → Post → Comment | Prevents silent join regression | Optional: snapshot behind feature or manual review |
| D2 | Document limitation: only two hops in `via()` today | README / rustdoc on `FindLinked` | Sets expectations |
| D3 | If N-hop required later: design pass using same `build_where_condition` per hop | RFC in `docs/planning/` | Avoid one-off SQL strings |

**Exit gate:** At least one DB integration test covers `find_linked` end-to-end (if not already), with joins + filter correctness asserted on data.

---

### Phase E — §5 continuation (optional, post-foundation)

| # | Task | Deliverable | Robustness / hardening |
|---|------|-------------|-------------------------|
| E1 | Keep `stream_and_cursor` in `db_integration_suite` mandatory for releases | Regression safety | Part of Phase C profile |
| E2 | Epic (separate from foundation): server-side cursor + `may` channel per gap doc | New APIs + stress test (memory cap) | Load test in CI with row budget |

Foundation **does not require** E2 to be “complete”; E1 is sufficient to protect today’s §5 surface.

---

## 3. Robustness matrix (cross-cutting)

| Risk | Detection | Mitigation | Owner phase |
|------|-----------|------------|-------------|
| PK fallback index mismatch | Code review + composite unit test | Prefer `get_by_column_name`; fallback documented as stub-only | A |
| Panic on missing column name | Integration tests with full derives | Batch APIs return `Result` | B |
| SQL injection / quoting | `Expr::cust` audit | Move to typed column refs; test quoted identifiers | A |
| Shared DB DDL races | Parallel test failures | nextest serial profile | C |
| Connection exhaustion | `too many clients` in logs | Pool, serial tests, or dedicated CI DB | C |
| Leaked containers | CI disk / docker ps | Ryuk / documented cleanup; dtor logging | C |
| Loader N+1 regression | Benchmark or query-count assertion | Integration test with query hook if available | B |
| `save_graph` (future §1) diverges from `RelationDef` | N/A yet | Single metadata source for insert order + `find_related` | §1 epic |

---

## 4. Sections not yet started (§1, §3, §4, §4.1, §6)

| Section | When you start | Build on foundation by |
|---------|----------------|-------------------------|
| **§1** | After Phase A–B stabilize relation usage | Same `RelationDef` for insert propagation and `find_related` |
| **§3** | Independent | Generated structs compatible with `LifeModel` / `LifeRecord` |
| **§4 / §4.1** | After programmatic table metadata | Schema manager consumes same metadata as relations |
| **§6** | Feature-gated | Resolvers use `ModelTrait` + batch loaders from Phase B |

---

## 5. Verification checklist (CI / local)

| Check | Command or action |
|-------|-------------------|
| Relation invariants | `cargo test -p lifeguard --lib relation::` |
| Workspace nextest (default dev; no parallel `db_integration_suite`) | `just nt` |
| DB suite (serial, shared DB safe) | `just nt-db-suite` **or** `cargo nextest run -p lifeguard --profile db-serial -E 'binary(db_integration_suite)'` |
| Full package sanity | `cargo test -p lifeguard` (as CI allows) |
| With service Postgres | Set `DATABASE_URL`, run serial profile; monitor connection count |

---

## 6. Suggested sequencing (summary)

```text
Phase A (contract + tests + Expr audit)
    ↓
Phase B (batch loader + .with MVP)     Phase C (nextest + docs + connections)  ← parallel after A starts
    ↓
Phase D (find_linked integration + docs)
    ↓
Phase E (optional §5 epic — server cursor / channels)
```

---

## Related documents

- [`LIFEGUARD_GAP_ANALYSIS.md`](../../../LIFEGUARD_GAP_ANALYSIS.md) — full SeaORM parity gaps  
- [Foundation work §](../../../LIFEGUARD_GAP_ANALYSIS.md#foundation-work-aligned-with-this-audit-not-gap-closure) — short mapping of recent changes  
- [`docs/TEST_INFRASTRUCTURE.md`](../../TEST_INFRASTRUCTURE.md) — env vars, `db_integration_suite`, `db-serial` profile  
- [`RELATION_WHERE_EXPR_DECISION.md`](RELATION_WHERE_EXPR_DECISION.md) — `Expr::cust` vs typed columns (Phase A4)  
- [`../lifeguard-derive/AUTHORING_MODEL_TRAIT.md`](../lifeguard-derive/AUTHORING_MODEL_TRAIT.md) — custom `ModelTrait` / `get_by_column_name` (Phase A5)  
