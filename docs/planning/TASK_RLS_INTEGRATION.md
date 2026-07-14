# RLS Integration Implementation Tracker

> **Rule: Tests First.** Before implementing any story, inspect the target surface. If existing test coverage is insufficient, write/add prerequisite tests *first*. Do not add RLS augmentation code until the prerequisite tests pass. This prevents regressions and ensures the augmentation path is verified alongside the existing zero-RLS path.

---

## Story 1: `SessionContext` struct + serialization
- **Surface:** `src/executor.rs` (new type)
- **Goal:** Define `SessionContext` with `#[derive(Debug, Clone, PartialEq)]`. All fields optional except `permissions: Vec<String>`. Implement `to_sql_args() -> Result<Vec<Box<dyn ToSql + '_>>, LifeError>`.
- **Files:** `src/executor.rs`, `src/lib.rs` (export)
- **Test Coverage Check:**
  - [x] Identified gaps: No existing tests for `SessionContext`.
  - [x] Written/prerequisite tests:
    - Unit tests for `to_sql_args()` with empty context (all `None`).
    - Unit tests for `to_sql_args()` with full context.
    - Unit test verifying `serde_json` permissions serialization correctness.
    - Verify `Clone` and `PartialEq` compile and behave correctly.
  - [x] Prerequisite tests pass (`cargo test --lib --workspace` — 585 tests)
- **Implementation Tasks:**
  - [x] Added `SessionContext` struct to `src/executor.rs`
  - [x] Implemented `to_sql_args()` with proper error handling
  - [x] Export via `src/lib.rs` (exported — `SessionContext` is pub in `executor.rs` which is already `pub mod executor;` in lib.rs)
- **Verification:** `cargo test` passes. `cargo clippy` clean.

---

## Story 2: `MayPostgresExecutor` augmentation
- **Surface:** `src/executor.rs` (`MayPostgresExecutor` impl)
- **Goal:** Add `session_context: Option<SessionContext>` and a builder. Contextual one-shot operations bind transaction-local injection and the application statement in one short transaction.
- **Files:** `src/executor.rs`
- **Test Coverage Check:**
  - [x] Identify gaps: Existing executor tests only covered error display and empty queries.
  - [x] Write/add prerequisite tests:
    - Unit test: `MayPostgresExecutor::new()` initializes `session_context` to `None`.
    - Unit test: `with_session_context()` sets field correctly and returns modified struct.
    - Unit test: Verify zero-regression path (`session_context == None`) compiles and runs identically to baseline.
  - [x] Verify prerequisite and live PostgreSQL tests pass
- **Implementation Tasks:**
  - [x] Add `session_context` field to struct
  - [x] Add `with_session_context()` builder method
  - [x] Implement `run_set_session()` helper
  - [x] Override `execute`, `query_one`, `query_all` to run contextual work in one short transaction
  - [x] Add doc comments explaining the injection behavior
- **Verification:** `cargo test` passes. `cargo clippy` clean. Zero regression on existing executor tests.

---

## Story 3: `Transaction` augmentation + `begin_with_session`
- **Surface:** `src/transaction.rs`, `src/executor.rs`
- **Goal:** Inject session context at `BEGIN` time. `Transaction` stores context reference. `MayPostgresExecutor` exposes `begin_with_session()`.
- **Files:** `src/transaction.rs`, `src/executor.rs`
- **Test Coverage Check:**
  - [x] Identify gaps: Existing transaction tests were unit-only.
  - [x] Write/add prerequisite and integration tests:
    - Unit test: `Transaction::new_with_session()` constructs struct correctly with `Some` and `None` contexts.
    - Unit test: `MayPostgresExecutor::begin_with_session()` returns correct error type on failure.
    - Unit test: Verify nested savepoint creation does not duplicate session injection (documented as expected: `SET LOCAL` is transaction-scoped).
  - [x] Verify transaction, isolation, nested-savepoint, and rollback cleanup tests pass
- **Implementation Tasks:**
  - [x] Add `session_context` field to `Transaction`
  - [x] Implement `new_with_session(client, ctx)` that runs `BEGIN` then conditionally `SELECT public.rls_set_session(...)`
  - [x] Add `begin_with_session(ctx)` on `MayPostgresExecutor`
  - [x] Add doc comments explaining transaction-scoped injection
- **Verification:** `cargo test` passes. `cargo clippy` clean.

---

## Story 4: `PooledLifeExecutor` worker job extension (data structures)
- **Surface:** `src/pool/pooled.rs` (`WorkerJob` enum)
- **Goal:** Add `session: Option<SessionContext>` to all `WorkerJob` variants. Update `with_enqueued_at` to preserve session. Pure data change, no behavior change yet.
- **Files:** `src/pool/pooled.rs`
- **Test Coverage Check:**
  - [x] Identify gaps: `pooled.rs` had no `WorkerJob` construction or `with_enqueued_at` tests.
  - [x] Write/add prerequisite tests:
    - Unit test: `WorkerJob::Execute/QueryOne/QueryAll` construct with `session: None` (backwards compatible).
    - Unit test: `WorkerJob::with_enqueued_at()` preserves existing `session` field across all variants.
    - Unit test: Verify `SessionContext` implements `Send` + `Clone` (required for channel dispatch).
  - [x] Verify prerequisite tests pass (`cargo test --workspace --lib`)
- **Implementation Tasks:**
  - [x] Add `session: Option<SessionContext>` to `WorkerJob` variants
  - [x] Update `with_enqueued_at` match arms to pass `session` through
  - [x] Add `#[cfg(test)]` module with construction/invariance tests
- **Verification:** `cargo test` passes. `cargo clippy` clean. Enum compiles. No functional changes.

---

## Story 5: `PooledLifeExecutor` dispatch + worker thread injection
- **Surface:** `src/pool/pooled.rs` (`PooledLifeExecutor`, `dispatch_worker_job`)
- **Goal:** Add `session_context` to the executor and dispatched jobs. A worker binds contextual injection and the application statement in one short transaction.
- **Files:** `src/pool/pooled.rs`
- **Test Coverage Check:**
  - [x] Identified gaps: Pool tests are minimal. No tests for executor builder or dispatch path. Worker injection is inherently integration-heavy.
  - [x] Written/prerequisite tests:
    - Unit test: `PooledLifeExecutor` constructs with `session_context` and serializes `SessionContext`.
    - Unit test: Verify dispatch closure construction compiles and captures context by value (closure takes `Option<SessionContext>`).
    - Unit test: Verify `SessionContext` fields are preserved through WorkerJob round-trip (construct → match → extract).
    - Unit test: `SessionContext` implements `Send + Sync + Clone` (required for channel dispatch).
    - Unit test: `with_enqueued_at` preserves session through re-enqueue on all variants.
  - [x] Prerequisite tests pass (`cargo test --workspace --lib` — 623 tests)
- **Implementation Tasks:**
  - [x] Add `session_context: Option<SessionContext>` field to `PooledLifeExecutor`
  - [x] Add `with_session_context()` builder
  - [x] Update `execute_values`, `query_one_values`, `query_all_values` to pass `self.session_context.clone()` through dispatch closures
  - [x] Modify `dispatch_worker_job` to execute contextual jobs as `BEGIN -> public.rls_set_session -> statement -> COMMIT`
- **Verification:** `cargo test --workspace --lib` passes (623 tests). `cargo clippy` clean.

---

## Story 6: Integration tests — end-to-end RLS propagation
- **Surface:** `tests/db_integration/rls_integration.rs` (new)
- **Goal:** Full integration tests using real Postgres with RLS policies enabled. Verify direct executor, transaction, and pool worker isolation.
- **Files:** `tests/db_integration/rls_integration.rs` (new)
- **Test Coverage Check:**
  - [x] Identified gaps: First story requiring real DB integration. No existing RLS test infrastructure.
  - [x] Written/prerequisite tests:
    - Integration test module `tests/db_integration/rls_integration.rs` with 4 scenarios:
      - **Test A** — Direct executor (`MayPostgresExecutor::with_session_context`): session GUC injected via `SELECT rls_set_session(...)`, RLS policy `USING (tenant = NULLIF(current_setting('auth.tenant', true), ''))` filters to expected rows.
      - **Test B** — Fail-closed (no context): same executor without session context returns 0 rows.
      - **Test C** — Transaction `begin_with_session`: context set at `BEGIN` time via `SET LOCAL`, all subsequent queries in the transaction inherit context.
      - **Test D** — Pool worker isolation: two `PooledLifeExecutor` instances with different contexts see different row subsets.
  - [x] Prerequisite tests pass (`cargo test --test db_integration_suite rls` — 4/4 passed)
- **Implementation Tasks:**
  - [x] Create `tests/db_integration/rls_integration.rs`
  - [x] Implement `rls_test_setup()` ctor: create `rls_test_role` (LOGIN, non-superuser), grant CONNECT/USAGE/EXECUTE, create `rls_set_session` with transaction-local `set_config(..., true)`, and drop stale function overloads from prior runs.
  - [x] Wrap direct and pooled contextual one-shot operations in `BEGIN -> helper -> application statement -> COMMIT`, with rollback on any failure.
  - [x] Prove context cleanup on the same direct connection and the same single-worker pool slot after a contextual job commits.
  - [x] Fix pool test: use `rls_test_role` connection URL so pool workers authenticate as non-superuser (superusers bypass RLS by default).
  - [x] Fix test assertions: removed `WHERE tenant = $X` sub-queries (explicit WHERE bypasses RLS), replaced with full-count queries that verify RLS filtering via visible row count.
  - [x] All 18 RLS tests pass.
- **Verification:** All 18 RLS tests pass against live PostgreSQL; the complete serial DB suite passes 107/107.

---

## Story 7: Documentation + examples
- **Surface:** `docs/`, `README`, doc comments
- **Goal:** Usage examples, architecture notes, scoping documentation.
- **Files:** `src/executor.rs` (doc comments, module docs), `docs/llmwiki/log.md`
- **Test Coverage Check:**
  - [x] RLS executor doctests compile
  - [x] `cargo test --workspace --lib` — 623 unit tests pass
  - [x] Complete serial DB suite — 107/107 tests pass, including all 18 RLS tests
  - [x] `cargo clippy` clean on all changed files
- **Implementation Tasks:**
  - [x] `SessionContext` struct — full doc block covering purpose, injection patterns, fields, two usage examples
  - [x] All 6 `SessionContext` fields — field-level `///` docs with PostgreSQL variable mappings
  - [x] Added `Default` derive so `..Default::default()` pattern works
  - [x] `MayPostgresExecutor::with_session_context` — already had good doc + example (unchanged)
  - [x] `MayPostgresExecutor::begin_with_session` — already had good doc + example (unchanged)
  - [x] `MayPostgresExecutor::begin_with_isolation_session` — added full doc with error conditions and example
  - [x] `executor.rs` module doc — replaced generic Epic header with proper RLS section linking entry points
  - [x] `PooledLifeExecutor::with_session_context` — already had good doc + example (unchanged)
  - [x] `Transaction::new_with_session` — already had good doc (unchanged)
  - [x] `SessionContext::to_sql_args` — already had good doc (unchanged)
  - [x] Wiki log updated in `docs/llmwiki/log.md`
- **Verification:** `cargo check`, formatting, clippy, library tests, RLS doctests,
  and the complete serial DB suite pass. The repository-wide doctest command is
  not yet a gate: 82 non-RLS examples remain stale and are outside this story.

---

## Delivery Order & Dependencies
1. **Story 1** → 2, 3, 4, 5
2. **Story 2** → 3, 5
3. **Story 3** → 5 (depends on Story 2 — `begin_with_session()` requires `session_context` field on `MayPostgresExecutor`)
4. **Story 4** → 5
5. **Story 5** → 6
6. **Story 6** → 7
7. **Story 7**

**Parallelism:** Story 1 is the sole entry point. Story 2 must complete before Story 3 (Story 3's `begin_with_session()` on `MayPostgresExecutor` requires the `session_context` field added in Story 2). Story 3 and Story 4 are independent of each other after Story 2. Story 5 requires both Story 3 and Story 4. Story 6 requires Story 5. Story 7 is last.
**Total Effort:** ~32 hours
**Quality Gate:** No story marked complete until `cargo test`, `cargo clippy`, and story-specific tests pass. Zero regressions on non-RLS paths.
