# PRD: Schema inference, validators, session/UoW, scopes, and F() expressions

**Slug:** `schema_validators_session_and_scopes`  
**Status:** **Draft** — Requirements and acceptance criteria; design splits into follow-on `DESIGN_*.md` per workstream as implementation starts.  
**Audience:** Lifeguard maintainers, `lifeguard-derive` authors, and application teams targeting SeaORM-like ergonomics on `may`.  
**Iteration 2 (PRD follow-on):** default git branch for the next tranche of work — `feat/schema_validators_session_and_scopes_2` (v0 landed via PR #56 on `main`; this branch continues §5–§9 “still to do” items).  
**References:** [README.md](../../README.md) competitive matrix (“Not Implemented” rows); [SEAORM_LIFEGUARD_MAPPING.md](./lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md); `src/query/`, `lifeguard-derive/`, `lifeguard::LifeRecord` / `LifeModel` patterns.

---

## 0. Progress at a glance

### 0.1 Milestones

- [x] PRD published (`PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md`)
- [x] Design note(s): schema inference CLI / codegen boundary — [DESIGN_SCHEMA_INFERENCE_CLI_CODEGEN.md](./DESIGN_SCHEMA_INFERENCE_CLI_CODEGEN.md)
- [x] **Phase A — Schema inference** ([§5](#5-schema-inference-from-db--diesel-style)) — *can ship independently* — **v0 landed:** `lifeguard-migrate infer-schema` + `lifeguard_migrate::schema_infer` (see §5.7)
- [x] **Phase B — Validators** ([§6](#6-validators-field--model-level)) — **v0 landed:** trait hooks + `run_validators` + `ActiveModelError::Validation`; see [§6.7](#67-implementation-status-v0)
- [x] **Phase C — Scopes** ([§7](#7-scopes-named-query-scopes)) — **v0 landed:** `SelectQuery::scope`, `IntoScope`; see [§7.7](#77-implementation-status-v0)
- [x] **Phase D — F() expressions** ([§8](#8-f-expressions-database-level-expressions)) — **v0 landed:** `ColumnTrait::f_add` / `f_sub` / `f_mul` / `f_div`; see [§8.7](#87-implementation-status-v0)
- [x] **Phase E — Session / Unit of Work (v0 — identity map + session handle)** ([§9](#9-session--unit-of-work-identity-map-dirty-tracking)) — **v0:** `ModelIdentityMap`, `Session`, `SessionDirtyNotifier`, `attach_session` / record auto-dirty enqueue; **deferred:** pool pin-slot UoW, insert-only flush; see [§9.7](#97-implementation-status-v0--u-2-partial)
- [x] [§10 Success criteria](#10-success-criteria) satisfied for **PRD v0** (partial parity per phase; follow-on work remains in §5–§9 “still to do” bullets)

### 0.2 Workstream rollup

| Workstream | Theme | PRD section |
|------------|--------|-------------|
| Schema inference (DB → Rust / Diesel-style) | [§5](#5-schema-inference-from-db--diesel-style) | Phase A |
| Validators (field & model-level) | [§6](#6-validators-field--model-level) | Phase B |
| Scopes (named query scopes) | [§7](#7-scopes-named-query-scopes) | Phase C |
| F() expressions (database-level expressions) | [§8](#8-f-expressions-database-level-expressions) | Phase D |
| Session / UoW (identity map, dirty tracking) | [§9](#9-session--unit-of-work-identity-map-dirty-tracking) | Phase E |

**Suggested implementation order:** **A → B → C → D → E** (E last: touches lifecycle, pooling, and executor contracts most deeply). Parallelism: **A** with **D** is possible if expression work stays in query layer only.

---

## 1. Executive summary

Lifeguard’s README historically called out five **SeaORM-parity** gaps; **v0 implementations** now exist for each (see §5.7–§9.7 and [SEAORM_LIFEGUARD_MAPPING.md](./lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md) **PRD parity snapshot**), with fuller parity (flush/UoW, derive sugar, expression coverage) still tracked here. This PRD defines **product requirements** and **acceptance criteria** so work can continue incrementally without breaking the coroutine-first, `may_postgres`-backed architecture.

Success means developers can (where applicable) **generate or refresh** models from live schema, **validate** `LifeRecord` data before persistence, **reuse** named filters on `SelectQuery`, **embed** SQL expressions in updates/filters without raw string SQL, and optionally **attach** a session that tracks identity and changes across operations—**without** requiring Tokio/async.

**Reading the workstreams ([§5](#5-schema-inference-from-db--diesel-style)–[§9](#9-session--unit-of-work-identity-map-dirty-tracking)):** Each feature block follows the same pattern: **Objectives** (measurable outcomes), **Why** (problem and value), **What** (user-visible scope and deliverables), **How** (implementation approach at a high level—design docs may refine), then **Requirements** (IDs and acceptance hints).

---

## 2. Problem statement

| ID | Gap | User impact |
|----|-----|--------------|
| P1 | Models and columns are **hand-maintained** while the database evolves | Drift, migration mistakes, slower onboarding |
| P2 | Validation is **ad hoc** (manual checks, hooks only) | Inconsistent rules, harder to mirror SeaORM validator ergonomics |
| P3 | No **identity map** or **UoW** | Duplicate loads, lost change tracking across a business operation |
| P4 | Repeated **filter** / **order** chains are **copy-pasted** | Errors, no single named “scope” abstraction |
| P5 | Updates default to **literal values**; column expressions need **raw SQL** or manual SeaQuery | Verbose, error-prone, not “Django F()-like” |

---

## 3. Goals

| ID | Goal |
|----|------|
| G1 | **Schema inference:** From a configured PostgreSQL connection (and optional schema/table filter), produce **Rust source** (or intermediate IR) aligned with `LifeModel` / `LifeRecord` conventions, with a **repeatable CLI or API** suitable for CI. |
| G2 | **Validators:** Field-level and model-level validation hooks with **clear ordering** (field → model), composable errors, and integration with insert/update/save paths. |
| G3 | **Scopes:** Named, reusable **query fragments** (predicates / conditions) attached to a model type, applicable from `SelectQuery` (and documented interaction with relations). |
| G4 | **F() expressions:** Type-safe **column references** in SET/WHERE/ORDER contexts so updates can express `SET col = col + 1` without hand-written SQL strings where possible. |
| G5 | **Session / UoW:** Optional **session** bound to an executor (or pool slot abstraction) providing **identity map** (primary-key keyed) and **dirty tracking** for loaded models, with explicit flush/commit boundaries. |
| G6 | **Docs & mapping:** Update [SEAORM_LIFEGUARD_MAPPING.md](./lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md) and README matrix rows when each phase lands. |

---

## 4. Non-goals

| ID | Non-goal | Rationale |
|----|----------|-----------|
| NG1 | **Multi-database** schema inference (MySQL, SQLite) | Lifeguard is PostgreSQL-first by design |
| NG2 | **Full** Django/ActiveRecord semantic parity | Borrow patterns; APIs should be idiomatic Rust |
| NG3 | **Distributed** session or cross-process identity map | In-process only unless a separate PRD says otherwise |
| NG4 | **Implicit global** session (magic singleton) | Must be explicit in API to stay compatible with `may` and testing |
| NG5 | **GraphQL / OpenAPI** generation from inferred schema | Out of scope; may consume inferred IR later |

---

## 5. Schema inference (from DB / Diesel-style)

### 5.1 Objectives

- Reduce **manual duplication** between PostgreSQL DDL and Rust `LifeModel` / column definitions.
- Make **bootstrap and refactors** repeatable: same command in CI and on a developer laptop produces reviewable, diffable output.
- Establish a **single source of truth** flow: DB → introspection → generated or suggested Rust, with explicit policy for unknown types (no silent mismatches).

### 5.2 Why

**Problem today:** Table and column shapes are **hand-maintained** or driven only forward from migrations. There is no first-class **reverse** path from a live (or migrated) database to `LifeModel` definitions. That causes **drift** between what Postgres actually stores and what Rust encodes, slows onboarding for brownfield databases, and forces teams to mirror Diesel/SeaORM “inspect DB → generate models” workflows by hand.

**Why invest:** Schema inference is the fastest way to **onboard** existing databases and to **re-sync** models after DBA-led changes. It anchors trust: generated output must be **conservative** (unknown types explicit) so Lifeguard never claims type safety it does not have.

### 5.3 What (scope)

- A **documented** tool or subcommand that takes a PostgreSQL connection string (and filters) and emits **Rust source** (or an intermediate representation that codegen consumes).
- **PostgreSQL-only** introspection (`information_schema` / `pg_catalog` queries), aligned with Lifeguard’s PG-first stance.
- Output that matches **existing** conventions: `#[table_name]`, column types mappable to `Value` / derive attributes, primary keys, nullable columns.
- Optional: **watch** mode or “diff only” for CI is a stretch; **minimum** is one-shot generation with stable ordering.

### 5.4 How (approach)

- **Introspection:** SQL queries against PostgreSQL system catalogs; map OID/type names to a fixed Lifeguard type mapping table (versioned in docs).
- **Emission:** Either `print` Rust modules or write files; **rustfmt**-friendly layout; **stable sort** of tables and columns.
- **Placement:** Likely live in **`lifeguard-migrate`** or a sibling binary to reuse connection config and env patterns; exact split is a design decision (see §5.6).
- **Testing:** Golden tests against a **Docker/Compose** schema or embedded SQL fixtures; no secrets in repos.

### 5.5 Requirements

| Req ID | Requirement | Acceptance criteria |
|--------|-------------|---------------------|
| SI-1 | Provide a **documented** entry point (CLI under `lifeguard-migrate` or a dedicated binary, TBD in design) that connects with a standard PostgreSQL URL and **introspects** tables. | Running against a known test schema yields deterministic Rust output (golden tests). |
| SI-2 | Map PostgreSQL types to Lifeguard **Value** / Rust types **conservatively** (unknown types → documented escape hatch). | Unsupported columns are skipped or emitted with `TODO` / raw type per design policy—not silent wrong types. |
| SI-3 | Respect **schema** and **table include/exclude** filters. | Integration test: only selected tables appear. |
| SI-4 | Output must be **merge-friendly** (stable ordering, section headers) for teams using codegen in CI. | Formatter + stable sort documented. |
| SI-5 | **Safety:** No credential logging; connection string handling matches existing `DatabaseConfig` / env patterns. | Audit pass; tests use env fixtures only. |

### 5.6 Open design choices (defer to design doc)

**Resolved for v0 contract:** see [DESIGN_SCHEMA_INFERENCE_CLI_CODEGEN.md](./DESIGN_SCHEMA_INFERENCE_CLI_CODEGEN.md) — CLI lives in `lifeguard-migrate`, output is review-first Rust source, derive runs at compile time on committed code.

**Still product-open:**

- Emit `LifeModel` only vs also `LifeRecord` stubs vs `PartialModel` hints (v0 emits both derives where applicable).
- Watch mode / CI golden snapshots (PRD stretch).

### 5.7 Implementation status (v0)

**Shipped in-tree:**

- **CLI:** `cargo run -p lifeguard-migrate -- infer-schema --database-url <URL>` (or `DATABASE_URL` / `LIFEGUARD_DATABASE_URL`). Flags: `--schema` (default `public`), `--table TABLE` (repeatable) to restrict tables.
- **Library:** `lifeguard_migrate::schema_infer::{infer_schema_rust, InferOptions}` — introspects `information_schema`, maps common PostgreSQL types to Rust types conservatively, emits `#[derive(LifeModel, LifeRecord)]` structs with `#[primary_key]` when the PK is a **single** column; **composite** PKs get a `TODO` comment; unsupported types are **omitted** with `// OMITTED:` lines (SI-2).

**SI-1 / golden coverage:** deterministic output is covered by unit tests on `emit_inferred_rust` in `lifeguard-migrate/src/schema_infer.rs` against `lifeguard-migrate/tests/golden/*.expected.rs` (single table, omitted column, composite PK TODO, table filter, SQL keyword field).

**Still to do for Phase A closure:** optional **`infer-schema` CLI** subprocess e2e. **Library / CI hooks:** `lifeguard-migrate/tests/infer_schema_postgres_smoke.rs` (connect + introspect `public`); `lifeguard-migrate/tests/infer_schema_table_filter_si3.rs` (SI-3 table filter — creates two scratch tables, infers one). Both skip when no DB URL. **Docs:** `lifeguard-migrate/README.md` (`infer-schema`), `DEVELOPMENT.md` (migrate / goldens / infer smoke).

**Design:** [DESIGN_SCHEMA_INFERENCE_CLI_CODEGEN.md](./DESIGN_SCHEMA_INFERENCE_CLI_CODEGEN.md) (CLI vs codegen boundary).

---

## 6. Validators (field & model-level)

### 6.1 Objectives

- Enforce **invariants before SQL**: length, range, format, and cross-field rules without duplicating checks at every call site.
- Return **structured, actionable errors** (which field, which rule) instead of ad-hoc `LifeError::Other` strings.
- Align **ordering** with developer expectations: field-level first, then model-level, with a documented default for fail-fast vs aggregate.

### 6.2 Why

**Problem today:** `ActiveModelBehavior` **hooks** allow before-save logic, but there is no **declarative** validator layer with **field** vs **model** granularity comparable to common ORM patterns. Validation ends up scattered across services, duplicated, or encoded as one-off checks inside hooks—hard to test and hard to mirror SeaORM-style ergonomics.

**Why invest:** Centralized validation keeps **persistence rules next to the model**, improves testability (unit test validators without a DB), and reduces the chance of invalid rows reaching Postgres (clearer errors, fewer round-trips).

### 6.3 What (scope)

- **Field validators:** Run on values present or changed on `LifeRecord` for insert/update/save paths (exact operations TBD in design).
- **Model validators:** Access multiple fields; run after field validators.
- **API surface:** Minimum = **traits** + manual registration or inherent impls; **stretch** = derive attributes (`#[validate(...)]`) where macro hygiene allows.
- **Errors:** Typed error type or `LifeError` variant that carries **field paths** and **messages**; optional aggregation mode.

### 6.4 How (approach)

- **Integration point:** Call validator pipeline from **`ActiveModelTrait` save/insert/update** (or a single internal choke point) **before** building SQL.
- **Sync only:** Validators are synchronous closures or trait methods; **no** async/`await` (matches `may` stack).
- **Composition:** Small building blocks (`validate_len`, custom `Fn`) composed into a **validator list** per model; optional derive generates lists.
- **Testing:** Pure unit tests on validator functions without Postgres; integration tests optional for end-to-end rejection.

### 6.5 Requirements

| Req ID | Requirement | Acceptance criteria |
|--------|-------------|---------------------|
| V-1 | **Field validators** run for present/changed fields on `LifeRecord` save paths (insert/update as applicable). | Unit tests: failing field validator blocks persistence and returns typed error. |
| V-2 | **Model validators** run after field validators and may inspect multiple fields. | Unit test: cross-field rule works. |
| V-3 | Errors are **aggregated** or **fail-fast** per explicit policy (default documented). | Tests cover both modes if both are exposed. |
| V-4 | Opt-out / skip for specific operations if needed (e.g. `save` vs `insert`) — **if** we expose hooks; otherwise document use of hooks. | Documented in rustdoc. |
| V-5 | **Derive or macro** optional sugar (`#[validate(...)]`) is a **stretch** goal; trait-based manual impl is minimum. | At least one ergonomic path ships in Phase B. |

### 6.6 Non-requirements for v1

- Async validators (not applicable to `may` sync stack).
- Arbitrary I/O inside validators (discouraged; document “sync only”).

### 6.7 Implementation status (v0)

**Shipped in-tree:**

- **Types:** `lifeguard::ValidateOp` (`Insert` | `Update`), `lifeguard::ValidationError` (`field: Option<String>`, `message: String`, with `field` / `model` constructors).
- **Errors:** `ActiveModelError::Validation(Vec<ValidationError>)` with `Display` listing field-scoped and model-scoped messages (fail-fast; no multi-error aggregation yet).
- **Traits:** `ActiveModelBehavior::validate_fields` / `validate_model` (default no-op), `validation_strategy` (default [`ValidationStrategy::FailFast`]), invoked via `lifeguard::run_validators` in order **field → model**.
- **V-3:** `ValidationStrategy::Aggregate` collects all `Validation` errors from `validate_fields` then `validate_model`; override `validation_strategy` on the record or call `run_validators_with_strategy` directly.
- **Delete:** `ValidateOp::Delete` after `before_delete`, before SQL; same validator hooks as insert/update.
- **Integration:** `lifeguard-derive` generated `insert` / `update` / `delete` call `run_validators` **after** the corresponding `before_*` hook and **before** SQL build.
- **Tests:** Unit tests on `run_validators` ordering, fail-fast, aggregate collection, and `Delete` op; `cargo clippy` / `lifeguard-derive` tests pass.
- **V-5 (derive sugar):** `#[validate(custom = path)]` on model fields — `path` is `fn(&sea_query::Value) -> Result<(), String>`; `LifeRecord` implements `validate_fields` to run each custom validator when `ActiveModelTrait::get` is `Some` for that column. Unsupported on `#[ignore]`/`#[skip]` fields. Tests: `lifeguard-derive/tests/test_minimal.rs` (`validate_attr_tests`).
- **Built-in predicates:** `lifeguard::predicates` (`src/active_model/predicates.rs`) — `string_utf8_chars_max`, `string_utf8_chars_in_range`, `blob_or_string_byte_len_max`, `i64_in_range`, `f64_in_range` on `sea_query::Value`; unit tests in-module.

**Still to do for fuller Phase B:** README / mapping matrix polish (G6).

---

## 7. Scopes (named query scopes)

### 7.1 Objectives

- Give every entity a **single place** for common predicates (`published`, `for_tenant`, `not_deleted` beyond soft-delete) that **compose** on `SelectQuery`.
- Eliminate **copy-paste** of SeaQuery `Condition` trees across handlers.
- Keep scopes **type-checked** and **SQL-injection-safe** (no dynamic string SQL).

### 7.2 Why

**Problem today:** Teams repeat the same **filter** / **order** chains everywhere. That invites **inconsistent** business rules (one path forgets `deleted_at IS NULL`), noisy diffs, and no named abstraction comparable to Rails **scopes** or Django **QuerySet** helpers.

**Why invest:** Scopes are **documentation + reuse**: the model type exposes “how we usually query this table,” which is especially valuable for large teams and for **soft delete** + **multi-tenant** patterns.

### 7.3 What (scope)

- **Named scope** entry points (associated functions on an entity module, a trait like `Scope`, or `impl` blocks) returning something composable with **`SelectQuery`** (conditions, or a small wrapper that applies `and_where`).
- **Composition:** `AND` required; `OR` where the type system allows without losing safety.
- **Interaction** with **`#[soft_delete]`** and global default scopes: order of application must be **defined** (document + test).

### 7.4 How (approach)

- **Build on SeaQuery:** Scopes return `Condition` or a closure `FnOnce(&mut SelectQuery)`—exact signature is a design choice; must not require string SQL.
- **Discoverability:** Prefer patterns that **rustdoc** can show (e.g. `User::scope_active()`).
- **Relations:** Document how scopes interact with **`find_related`** / loaders (e.g. scope applies to root entity only unless explicitly designed for joins).

### 7.5 Requirements

| Req ID | Requirement | Acceptance criteria |
|--------|-------------|---------------------|
| SC-1 | **Named scope** API on the model side (associated functions or traits) returning a **reusable condition** or query transformer **documented** in one place. | Example in `examples/` or integration test compiles and runs. |
| SC-2 | Scopes **compose** (AND at minimum; OR where type-safe). | Unit tests for composition. |
| SC-3 | Interaction with **soft delete** / default filters is **defined** (scopes apply before/after global filters per design). | Doc + test. |
| SC-4 | No runtime string SQL; **SeaQuery**-backed or lifeguard condition types only. | Clippy / API review. |

### 7.7 Implementation status (v0)

**Shipped in-tree:**

- **API:** `lifeguard::SelectQuery::scope` and `lifeguard::IntoScope` in `src/query/scope.rs`. Any `sea_query::IntoCondition` (column expressions, `Condition`, etc.) applies as a scope; implementation delegates to `SelectQuery::filter` so predicates **AND** together.
- **Pattern:** Entity-associated functions (e.g. `UserEntity::scope_active() -> impl IntoCondition`) are composed with `User::find().scope(UserEntity::scope_active())`.
- **Soft delete:** `query::scope` module documents that `LifeModelTrait::soft_delete_column` is applied at execution time and **AND**ed with scoped predicates unless `with_trashed` is set; unit test `scope_and_soft_delete_both_anded_at_execution`.
- **Tests:** `src/query/scope.rs` — composition + soft-delete interaction + `scope_or` / `scope_any`.

**Still to do for fuller Phase C:** derive sugar (`#[scope]` / codegen). **Done in-tree:** `SelectQuery::scope_or` / `scope_any` (PRD SC-2). **`find_related` / loaders:** see [DESIGN_FIND_RELATED_SCOPES.md](./DESIGN_FIND_RELATED_SCOPES.md). README matrix (G6) updated for scopes.

---

## 8. F() expressions (database-level expressions)

### 8.1 Objectives

- Allow **increment/decrement** and simple arithmetic on columns in **`UPDATE`** (and where feasible **`WHERE` / `ORDER BY`**) without raw SQL strings.
- Preserve **type and column identity** at compile time where the query builder allows (qualified column names, correct quoting).
- Stay **PostgreSQL-correct** for arithmetic and casting on common numeric types.

### 8.2 Why

**Problem today:** Updates often need **database-side** expressions such as `SET view_count = view_count + 1`. Lifeguard/SeaQuery paths today push developers toward **raw SQL** or low-level SeaQuery APIs that are not wrapped as a **consistent, discoverable** API—unlike Django’s **`F()`** expressions.

**Why invest:** This is a **high-frequency** pattern for counters, locks, and derived fields. A small, typed surface reduces bugs (wrong column name, missing cast) and keeps **review** straightforward.

### 8.3 What (scope)

- Helpers or types (e.g. `col(...)`, `F::column`, or SeaQuery extensions) that represent **another column** or **expression** on the right-hand side of assignments and in predicates.
- **Documented** support for at least: **integer** and **numeric** **`+` `-` `*` `/`**; other types deferred with explicit “use raw SQL” escape hatch.
- **Limitations** documented: aggregates, subqueries, vendor functions beyond a small set may remain **raw SQL** by design for v1.

### 8.4 How (approach)

- **SeaQuery-first:** Extend or wrap `SimpleExpr` / `Expr` usage in **`SelectQuery`** / **`ActiveModelTrait`** update builders so generated SQL uses proper identifiers.
- **Spike early:** Validate SeaQuery can emit `SET col = col + $1` (or literal) for Postgres; if gaps exist, document **minimal** raw escape.
- **Tests:** SQL snapshot or structured assert on generated SQL strings **plus** integration test on a real table.

### 8.5 Requirements

| Req ID | Requirement | Acceptance criteria |
|--------|-------------|---------------------|
| F-1 | Provide a **typed** way to refer to **another column** (or SQL expression) in `UPDATE` SET clauses and, where feasible, in `WHERE` / `ORDER BY`. | Integration or unit tests for `col = col + literal` and similar. |
| F-2 | **PostgreSQL**-correct quoting / casting behavior for supported operations. | Regression tests for at least `+`, `-`, `*`, `/` with integer and numeric columns. |
| F-3 | Document **limitations** (e.g. unsupported nested aggregates) vs raw SQL escape hatch. | README + rustdoc. |

### 8.7 Implementation status (v0)

**Shipped in-tree:**

- **API:** `ColumnTrait` methods **`f_add`**, **`f_sub`**, **`f_mul`**, **`f_div`** in `src/query/column/column_trait.rs` — each returns `sea_query::SimpleExpr` for use with `sea_query::Query::update()` / `UpdateStatement::value`, e.g. `SET col = col + $1` via `query.value(Col::X, Col::X.f_add(1))`.
- **Rustdoc:** Limitations (aggregates/subqueries → `Expr::cust` or SeaQuery) documented on `f_add`.
- **Tests:** `src/query/column/column_trait.rs` — `test_f_add_update_sql_contains_arithmetic`, basic compile tests for `f_*`.
- **Process:** `docs/planning/DEV_RUSTDOC_AND_COVERAGE.md` and `DEVELOPMENT.md` (rustdoc + coverage checklist for feature work).

**F-3 (limitations vs raw SQL):** `ColumnTrait::f_add` rustdoc (aggregates/subqueries → `Expr::cust`); README competitive section + feature bullets (§10 / G6).

**Still to do for fuller Phase D:** deeper numeric casting docs if needed. **Done in-tree:** `LifeRecord` `set_<field>_expr` / `__update_exprs` / derived `update()`; `identity_map_key` for session bridge; `insert()` rejects non-empty `__update_exprs`; Postgres `column_f_update.rs` + `column_f_where.rs`; README G6.

### 8.8 Dependency note

**LifeRecord** `update()` / `set_*_expr` path coordinates with SeaQuery `UpdateStatement::value`. **`SelectQuery`** F-style filters use `Expr::expr` + SeaQuery’s `ExprTrait` at the call site.

---

## 9. Session / Unit of Work (identity map, dirty tracking)

### 9.1 Objectives

- Within a **bounded unit of work**, ensure **at most one** in-memory **model instance per primary key** (identity map), so references compare equal and mutations converge.
- Track **dirty state** so a single **flush** can persist multiple touched entities in a defined order (or explicit ordering API).
- Keep the session **explicit** (constructed with an executor or pool policy)—**no** hidden global state.

### 9.2 Why

**Problem today:** Longer business operations **reload** the same row twice, get **two** Rust values for the same PK, and must manually merge changes. There is no **Unit of Work** that knows “these rows were loaded here; these `LifeRecord`s are pending.” Developers coming from Hibernate, Entity Framework, or SeaORM’s session-adjacent patterns expect **identity** and **flush** semantics.

**Why invest:** Session/UoW reduces **redundant queries**, clarifies **transaction boundaries**, and supports **batch flush** patterns. It is the **highest integration risk** because it touches executor lifetime, pooling, and possibly transaction nesting.

### 9.3 What (scope)

- **`Session`** (name TBD) type created by the application, holding an **identity map** keyed by `(Entity, PK)` and references to loaded **`LifeModel`** / dirty **`LifeRecord`** handles as designed.
- **Flush** commits pending changes using the underlying **`LifeExecutor`** (or a dedicated connection policy when using **`LifeguardPool`**).
- **Explicit lifecycle:** `new` / `drop` or `close`; **no** thread-local implicit session.
- **Documented** interaction with **pooled** execution: e.g. session pins one logical connection vs dispatches per operation—**must** be one coherent story (see U-4).

### 9.4 How (approach)

- **Phase 1 design spike:** Identity map as `HashMap` + **weak references** vs **`Rc`**—Rust ownership must be resolved; **copy-on-write** vs **mutable singleton** per PK is a key decision.
- **Executor binding:** Session likely holds **`MayPostgresExecutor`** or a **pool handle** with a defined “borrow connection for this unit of work” API; align with [PRD_CONNECTION_POOLING.md](./PRD_CONNECTION_POOLING.md) so we do not deadlock or starve workers.
- **Transactions:** Optional **single transaction** for the whole session flush, or nested savepoints—pick minimal semantics for v1.
- **Concurrency:** Default to **single-threaded** session use; document **`Send`/`Sync`** expectations and `may` coroutine usage.

### 9.5 Requirements

| Req ID | Requirement | Acceptance criteria |
|--------|-------------|---------------------|
| U-1 | **Identity map:** Loading the same PK twice within a session returns the **same** model instance (or documented copy-on-write semantics—**pick one** in design). | Unit/integration test proves single identity. |
| U-2 | **Dirty tracking:** Mutations mark instances dirty; **flush** persists changes in a defined order (or explicit sort). | Tests for multi-entity flush. |
| U-3 | **Boundary:** Session is **explicitly** created and disposed (e.g. `Session::new(executor)` or pool-scoped); **no** implicit thread-local global. | API review + negative tests. |
| U-4 | **Interaction with `LifeguardPool`:** Document whether the session holds a **single** executor handle, pins a worker, or uses a dedicated connection policy. | Design doc + pool docs cross-link. |
| U-5 | **Concurrency:** Document that `may` coroutines sharing a session must follow **single-owner** or **mutex** rules if applicable. | Documented; tests for minimal serial case. |

### 9.6 Non-goals for v1

- Full Hibernate-style **lazy collections** (unless explicitly added later).
- Cross-database two-phase commit.

### 9.7 Implementation status (v0 + U-2 partial)

**Shipped in-tree:**

- **API:** `lifeguard::ModelIdentityMap` and `lifeguard::fingerprint_pk_values` in `src/session/` — identity map keyed by stable PK fingerprints (`src/session/pk.rs`); same primary key → same `Rc<RefCell<Model>>` (first registration wins; duplicate model dropped).
- **U-2 (partial):** `mark_dirty`, **`mark_dirty_key`** (fingerprint string), `unmark_dirty`, `is_marked_dirty`, `dirty_len`, `clear_dirty`, `flush_dirty` on **`ModelIdentityMap`** — dirty keys flushed in **lexicographic order of PK fingerprint** via a closure `Fn(&dyn LifeExecutor, Rc<RefCell<Model>>) -> Result<(), ActiveModelError>` (callers wire `LifeRecord::update` / `save`). Derived **`LifeRecord::identity_map_key()`** returns `Some(fingerprint)` when all PK columns are set. **`Session`** (`src/session/uow.rs`) shares an identity map and merges a **`Send`/`Sync` pending-dirty queue** at **`Session::flush_dirty`**; **`Session::flush_dirty_in_transaction`** (direct `MayPostgresExecutor` only — `BEGIN`/`COMMIT` around the flush closure). **`LifeRecord::attach_session` / `detach_session`** (PK entities): `set_*`, **`ActiveModelTrait::set` / `take` / `set_col`**, and **`set_*_expr`** enqueue dirty via **`SessionDirtyNotifier`** when the PK is set on the record. **Not** shipped: insert-only flush for unregistered rows; **pooled** multi-statement transaction around a whole flush (use **`Session::flush_dirty_in_transaction`** with a direct `lifeguard::MayPostgresExecutor` for `BEGIN`/`COMMIT` around flush).
- **Design:** `docs/planning/DESIGN_SESSION_UOW.md` — pool pinning, flush, and `may`/threading notes (U-4, U-5).
- **Rustdoc:** `session` module documents identity, dirty flush, threading (`Send`/`Sync`).
- **Tests:** `src/session/mod.rs`, `src/session/pk.rs`, `src/session/uow.rs` — identity map, fingerprint, dirty order, flush error retention, `Session` pending merge, `SessionDirtyNotifier` `Send`. **`db_integration_suite`:** `tests/db_integration/session_identity_flush.rs` — raw map flush, `mark_dirty_key` + `identity_map_key`, **`Session` + `attach_session` + record `set_*`**, **`Session::flush_dirty_in_transaction`** → `LifeRecord::update` on Postgres.
- **Process:** `docs/planning/DEV_RUSTDOC_AND_COVERAGE.md` and `DEVELOPMENT.md` (rustdoc + coverage checklist for feature work).

**Still to do for fuller Phase E:** optional pool **pin-slot** UoW (U-4) for transactional flush through `PooledLifeExecutor`, insert-only flush for new rows, auto-sync **Model** from **LifeRecord** (vs app-maintained consistency), mapping matrix row tweaks as APIs grow.

---

## 10. Success criteria

- [x] Each **phase** (A–E) has **passing tests** (unit and, where needed, integration with `TEST_DATABASE_URL`) — **v0:** phases ship unit tests; integration coverage varies by workstream (see §5.7–§9.7).
- [x] **Public rustdoc** describes the supported API surface and sharp edges — **v0:** each phase documents limitations in-module (ongoing: expand examples as APIs stabilize).
- [x] [SEAORM_LIFEGUARD_MAPPING.md](./lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md) and [README.md](../../README.md) competitive table updated: **Partial** / **Implemented** labels for schema inference, validators, scopes, F(), session/UoW; mapping doc **PRD parity snapshot** table.
- [x] No new **unwrap** in library paths without JSF policy compliance; clippy `-D warnings` on touched crates — **policy:** `#![deny(clippy::unwrap_used)]` / `expect_used` on `lifeguard` crate; run clippy on touched crates before merge.

---

## 11. Master implementation checklist (requirement IDs)

**Schema inference:** SI-1 — SI-5  
**Validators:** V-1 — V-5  
**Scopes:** SC-1 — SC-4  
**F() expressions:** F-1 — F-3  
**Session / UoW:** U-1 — U-5  

---

## 12. Risks

| Risk | Mitigation |
|------|------------|
| Session + pool **deadlocks** or connection pinning | Design session lifetime before coding; spike with `LifeguardPool` |
| SeaQuery **API gaps** for F() | Time-box spike; document raw SQL fallback |
| Generated schema **drift** from team style | Expose formatting + allow “only new tables” modes |
| Validator **ordering** surprises | Document ordering; default fail-fast vs aggregate |

---

## 13. References

- [README.md](../../README.md) — “Competitive metrics” table (Not Implemented rows).
- [PRD_CONNECTION_POOLING.md](./PRD_CONNECTION_POOLING.md) — pool semantics that Session must align with.
- PostgreSQL information schema — introspection source of truth for Phase A.
