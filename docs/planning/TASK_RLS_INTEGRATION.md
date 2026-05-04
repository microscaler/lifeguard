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
- **Goal:** Add `session_context: Option<SessionContext>` field. Add `with_session_context(ctx)` builder. Override `execute`, `query_one`, `query_all` to run `SET LOCAL` injection if context is present.
- **Files:** `src/executor.rs`
- **Test Coverage Check:**
  - [ ] Identify gaps: Existing executor tests only cover error display and empty queries. No tests for field mutation or builder pattern.
  - [ ] Write/add prerequisite tests:
    - Unit test: `MayPostgresExecutor::new()` initializes `session_context` to `None`.
    - Unit test: `with_session_context()` sets field correctly and returns modified struct.
    - Unit test: Verify zero-regression path (`session_context == None`) compiles and runs identically to baseline.
  - [ ] Verify prerequisite tests pass (`cargo test executor`)
- **Implementation Tasks:**
  - [ ] Add `session_context` field to struct
  - [ ] Add `with_session_context()` builder method
  - [ ] Implement `run_set_session()` helper
  - [ ] Override `execute`, `query_one`, `query_all` to conditionally call `run_set_session()` before delegating
  - [ ] Add doc comments explaining the injection behavior
- **Verification:** `cargo test` passes. `cargo clippy` clean. Zero regression on existing executor tests.

---

## Story 3: `Transaction` augmentation + `begin_with_session`
- **Surface:** `src/transaction.rs`, `src/executor.rs`
- **Goal:** Inject session context at `BEGIN` time. `Transaction` stores context reference. `MayPostgresExecutor` exposes `begin_with_session()`.
- **Files:** `src/transaction.rs`, `src/executor.rs`
- **Test Coverage Check:**
  - [ ] Identify gaps: Existing transaction tests are unit-only (error types, isolation levels). No actual DB transaction tests. No tests for session context fields.
  - [ ] Write/add prerequisite tests:
    - Unit test: `Transaction::new_with_session()` constructs struct correctly with `Some` and `None` contexts.
    - Unit test: `MayPostgresExecutor::begin_with_session()` returns correct error type on failure.
    - Unit test: Verify nested savepoint creation does not duplicate session injection (documented as expected: `SET LOCAL` is transaction-scoped).
  - [ ] Verify prerequisite tests pass (`cargo test transaction`)
- **Implementation Tasks:**
  - [ ] Add `session_context` field to `Transaction`
  - [ ] Implement `new_with_session(client, ctx)` that runs `BEGIN` then conditionally `SELECT rls_set_session(...)`
  - [ ] Add `begin_with_session(ctx)` on `MayPostgresExecutor`
  - [ ] Add doc comments explaining transaction-scoped injection
- **Verification:** `cargo test` passes. `cargo clippy` clean.

---

## Story 4: `PooledLifeExecutor` worker job extension (data structures)
- **Surface:** `src/pool/pooled.rs` (`WorkerJob` enum)
- **Goal:** Add `session: Option<SessionContext>` to all `WorkerJob` variants. Update `with_enqueued_at` to preserve session. Pure data change, no behavior change yet.
- **Files:** `src/pool/pooled.rs`
- **Test Coverage Check:**
  - [ ] Identify gaps: `pooled.rs` only has `lifetime_effective_limit_tests`. Zero tests for `WorkerJob` construction or `with_enqueued_at`.
  - [ ] Write/add prerequisite tests:
    - Unit test: `WorkerJob::Execute/QueryOne/QueryAll` construct with `session: None` (backwards compatible).
    - Unit test: `WorkerJob::with_enqueued_at()` preserves existing `session` field across all variants.
    - Unit test: Verify `SessionContext` implements `Send` + `Clone` (required for channel dispatch).
  - [ ] Verify prerequisite tests pass (`cargo test pool`)
- **Implementation Tasks:**
  - [ ] Add `session: Option<SessionContext>` to `WorkerJob` variants
  - [ ] Update `with_enqueued_at` match arms to pass `session` through
  - [ ] Add `#[cfg(test)]` module with construction/invariance tests
- **Verification:** `cargo test` passes. `cargo clippy` clean. Enum compiles. No functional changes.

---

## Story 5: `PooledLifeExecutor` dispatch + worker thread injection
- **Surface:** `src/pool/pooled.rs` (`PooledLifeExecutor`, `dispatch_worker_job`)
- **Goal:** Add `session_context` field to executor. Update dispatch closures to append context. Worker thread runs `SET LOCAL` if `session` is present.
- **Files:** `src/pool/pooled.rs`
- **Test Coverage Check:**
  - [ ] Identify gaps: Pool tests are minimal. No tests for executor builder or dispatch path. Worker injection is inherently integration-heavy.
  - [ ] Write/add prerequisite tests:
    - Unit test: `PooledLifeExecutor::with_session_context()` sets field correctly.
    - Unit test: Verify dispatch closure construction compiles and captures context by value (closure takes `Option<SessionContext>`).
    - Integration test harness: Set up a mock pool channel to verify context flows through `dispatch` to the reply channel (or verify via channel payload inspection).
  - [ ] Verify prerequisite tests pass (`cargo test pool`)
- **Implementation Tasks:**
  - [ ] Add `session_context` field to `PooledLifeExecutor`
  - [ ] Add `with_session_context()` builder
  - [ ] Add `dispatch_with_session()` method that appends `self.session_context.clone()` to closure
  - [ ] Update `execute_values`, `query_one_values`, `query_all_values` to use `dispatch_with_session`
  - [ ] Modify `dispatch_worker_job` to extract `session`, run `SET LOCAL` if `Some`, then proceed normally
- **Verification:** `cargo test` passes. `cargo clippy` clean. Dispatch path verified. Worker injection verified.

---

## Story 6: Integration tests — end-to-end RLS propagation
- **Surface:** `tests-integration/` (new or existing test module)
- **Goal:** Full integration tests using real Postgres with RLS policies enabled. Verify direct executor, transaction, and pool worker isolation.
- **Files:** `tests-integration/rls_integration_tests.rs` (new) or append to existing integration test runner
- **Test Coverage Check:**
  - [ ] Identify gaps: First story requiring real DB integration. No existing RLS test infrastructure.
  - [ ] Write/add prerequisite tests (test-first even for integration):
    - Set up test container / dedicated test DB with `ENABLE ROW LEVEL SECURITY` on a test table.
    - Test A: Direct executor verifies RLS filters rows correctly.
    - Test B: Fail-closed path (`None` context) returns 0 rows.
    - Test C: Transaction `begin_with_session` injects at `BEGIN`, subsequent queries inherit context.
    - Test D: Pool workers maintain correct isolation across different session contexts.
  - [ ] Verify prerequisite tests pass (`cargo test --test rls_integration`)
- **Implementation Tasks:**
  - [ ] Create integration test module with testcontainer setup
  - [ ] Implement 4 test scenarios above
  - [ ] Add migration/DDL setup in test fixture for RLS policies
- **Verification:** All integration tests pass against live Postgres. `cargo test` clean.

---

## Story 7: Documentation + examples
- **Surface:** `docs/`, `README`, doc comments
- **Goal:** Usage examples, architecture notes, scoping documentation.
- **Files:** `docs/` or `README.md`, doc comments on exported types
- **Test Coverage Check:**
  - [ ] Identify gaps: Doc comments need to compile. Examples must be syntactically valid.
  - [ ] Write/add prerequisite tests:
    - Run `cargo test --doc` to verify all doc examples compile.
    - Run `cargo test` to ensure no regressions.
  - [ ] Verify prerequisite tests pass
- **Implementation Tasks:**
  - [ ] Add doc comments to `SessionContext`, `with_session_context`, `begin_with_session`
  - [ ] Add usage example in `README.md` or `docs/rls-integration-v2-example.md`
  - [ ] Document `SET LOCAL` scoping behavior (per-query, per-transaction, per-job)
  - [ ] Note that SQL functions and RLS policies are app-owned
- **Verification:** `cargo test --doc` passes. `cargo doc --no-deps` builds without warnings.

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
