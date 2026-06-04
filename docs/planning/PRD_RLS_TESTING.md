# PRD: RLS Testing — Coverage Gaps & Edge Cases

> **Date:** 2026-05-04
> **Author:** Agent
> **Scope:** All RLS-related code surfaces — `SessionContext`, executor injection, migration generation, schema inference, schema comparison, and derive attribute validation.
> **Goal:** Identify testing gaps across the full RLS feature lifecycle and define a test plan to close them before declaring RLS "production ready."

---

## 1. Current State

### 1.1 What We Have

| Surface | Test Type | Status | Notes |
|---------|-----------|--------|-------|
| `SessionContext::to_sql_args()` | Unit (`#[cfg(test)]` in executor.rs) | ✅ 3 tests | Empty, full, and partial contexts. Verifies arg count and JSON serialisation of permissions. |
| `SessionContext` derives (Clone, PartialEq, Debug) | Unit | ✅ Implicit | Covered by unit tests using the struct. |
| `SessionContext` field-level docs | Doc comments | ✅ Done | Added in Story 7. Not testable, but verified via `cargo doc`. |
| `MayPostgresExecutor::with_session_context()` | Unit | ❌ Missing | No test verifies the field starts as `None` or that the builder returns the modified struct. |
| `MayPostgresExecutor` zero-regression path | Unit | ❌ Missing | No test proves `session_context: None` makes the executor behave identically to baseline. |
| `Transaction::new_with_session()` | Unit | ❌ Missing | No test verifies context is stored or that `BEGIN` runs before the RLS call. |
| `MayPostgresExecutor::begin_with_session()` | Unit | ❌ Missing | No compile-time or unit test for this method signature. |
| `WorkerJob` variants (session field) | Unit | ❌ Missing | No test for construction with `session: None` or `session: Some(...)`. |
| `WorkerJob::with_enqueued_at()` preservation | Unit | ❌ Missing | No test verifying session is preserved across re-enqueue. |
| `SessionContext` Send + Clone + Sync | Unit | ❌ Missing | No static assert tests (`assert_send`, `assert_clone`). |
| `PooledLifeExecutor::with_session_context()` | Unit | ❌ Missing | Builder pattern not tested. |
| End-to-end RLS propagation | Integration (`rls_integration.rs`) | ✅ 4 tests | Direct executor, fail-closed, transaction, pool isolation. All pass against live Postgres. |

### 1.2 What We Do NOT Have (Migration Layer)

`lifeguard-migrate` is the tool that generates SQL from entity definitions. **It has zero RLS awareness or tests.** Specifically missing:

| Test | Surface | Why it matters |
|------|---------|----------------|
| **Entity-level RLS attributes** | `lifeguard-derive` + `sql_generator` | If we ever add `#[rls_enabled]` or `#[rls_policy = "..."]` to entity attributes, the derive must parse them and the generator must produce `ALTER TABLE ... ENABLE ROW LEVEL SECURITY` / `CREATE POLICY` statements. Currently no entity in the codebase uses RLS attributes (because they don't exist yet), and no test verifies that the migration tool ignores unknown attributes gracefully. |
| **`lifeguard-derive` unknown attribute handling** | Derive macros | If an entity accidentally uses a typo like `#[rls_enabled = true]` or `#[rls_policy = "name"]`, the derive should either emit a compile-time error or a warning. Currently there's no test for how the derive handles unknown/lifecycle attributes. |
| **`sql_generator` output for non-RLS tables** | `sql_generator.rs` | Proves that generate-from-entities produces identical output regardless of whether the table *could* be RLS-enabled. A regression here would mean RLS code accidentally modifies non-RLS table generation. |
| **`infer-schema` with RLS tables** | `schema_infer.rs` | If a table has `ENABLE ROW LEVEL SECURITY` set (visible in `pg_table` catalog), the infer tool should either note it or ignore it. No test verifies this. |
| **`compare-schema` with RLS policies** | `schema_migration_compare.rs` | Live DB tables may have `CREATE POLICY` entries in `pg_policies`. `compare-schema` currently ignores them. A test should verify this intentional gap (or flag it for future). |

---

## 2. Edge Cases That Need Tests

### 2.1 Executor / Injection Layer (lifeguard core)

| # | Edge Case | Risk | Test Type |
|---|-----------|------|-----------|
| E1 | **Empty `SessionContext`** (all fields `None`, empty permissions `Vec`) | GUCs set to `NULL`/empty. RLS policies use `NULLIF(current_setting(...), '')` — empty string becomes `NULL`, allowing all rows. This is intentional ("allow all"), but a test must verify it explicitly so it's not accidentally changed to "deny all" later. | Integration |
| E2 | **`SessionContext` with only `permissions` set** (no user_id, no org_id) | Permissions array is JSON in `$5`. When user_id is `NULL`, tenant is `NULL`, permissions are `["admin"]` — what does RLS policy do? The test should verify this is handled gracefully (policy decides). | Integration |
| E3 | **`permissions` with special characters** (e.g. `"admin:write"`, `read/write`) | `serde_json` serializes to `["admin:write"]`. If the policy does string comparison, special chars could match unexpectedly. Unit test for JSON serialisation correctness + integration test for policy interaction. | Unit + Integration |
| E4 | **Transaction re-entrancy** — `begin_nested()` inside a `begin_with_session()` transaction | The `SET LOCAL` at `BEGIN` applies to the whole transaction. Nested savepoints don't get their own `rls_set_session` call (by design). A test should verify that RLS context persists correctly through savepoint rollbacks. | Integration |
| E5 | **Rapid context switching on pooled executor** — job A (tenant 1) immediately followed by job B (tenant 2) | Pool workers process jobs sequentially. If the worker injects context for job A but doesn't reset it before job B, job B might see job A's data. The `SET LOCAL` per-job scoping should prevent this, but it needs explicit testing. | Integration (enhancement of Test D) |
| E6 | **`rls_set_session` function not present** | If the SQL function is missing from the schema, `run_set_session` should return an error. Currently `MayPostgresExecutor` just calls `client.query_one()` and propagates the error. A test should verify the error path (connection to an empty DB). | Integration |
| E7 | **Superuser bypass** — connecting as superuser ignores RLS policies entirely | The integration test (Test D) already fixes this by using `rls_test_role`. But we need to document it and potentially add a test that explicitly verifies: when connecting as superuser, RLS is bypassed. | Integration |
| E8 | **Connection URL with no password** (peer auth, or passwordless test user) | If `rls_test_role` can't authenticate, the test silently falls back. Need to verify the test setup actually creates a usable role. | Integration (setup) |

### 2.2 Transaction / Session Boundary

| # | Edge Case | Risk | Test Type |
|---|-----------|------|-----------|
| T1 | **`begin_with_session` followed by `rollback()`** | `SET LOCAL` is transaction-scoped. A rollback should discard the context, and a subsequent `begin()` should start fresh. No test verifies this. | Integration |
| T2 | **`begin_with_session` with `IsolationLevel::Serializable`** | The `SET LOCAL rls_set_session(...)` call happens before `BEGIN` (in `new_with_session`). If the isolation level is set after `BEGIN`, the `SET LOCAL` might not be in the same transaction scope. Actually, `new_with_session` runs `SET LOCAL` *after* `BEGIN`, so it should be fine. But a test should verify this. | Integration |
| T3 | **`begin_with_isolation_session` with non-ReadCommitted isolation** | Same as T2, but for `begin_with_isolation_session`. | Integration |
| T4 | **Concurrent transactions on the same connection** (shouldn't happen, but verify) | If two `MayPostgresExecutor` instances share the same underlying `Client`, their session contexts will conflict. This is a user error, but a test should document the expected failure mode. | Unit / Integration |

### 2.3 Migration Tool (`lifeguard-migrate`)

| # | Edge Case | Risk | Test Type |
|---|-----------|------|-----------|
| M1 | **Entity with no RLS attributes** (current baseline) | `generate-from-entities` should produce the exact same output as today. Proves no regression from RLS additions. | Unit (golden test) |
| M2 | **Entity file parse failure / malformed derive** | `entity_loader::load_entities()` should skip the file with a warning, not panic. | Unit |
| M3 | **`infer-schema` on a schema with RLS-enabled tables** | The tool should introspect columns and indexes as normal. RLS state (`pg_class.relrowsecurity`) is not a column or index, so it's ignored. A test should verify this is intentional. | Integration (smoke) |
| M4 | **`compare-schema` on a DB with RLS policies** | `compare-schema` currently ignores `pg_policies`. A test should verify it doesn't emit false-positive drift for policies. | Integration |
| M5 | **Migration SQL for a table that exists and has RLS enabled** | If an entity generates `CREATE TABLE IF NOT EXISTS ...` and the table already has RLS enabled from a prior run, the re-run should not fail. The `IF NOT EXISTS` handles this, but a test should verify. | Integration |
| M6 | **Entity with `#[schema_name = "secure"]` (non-public schema)** | RLS policies on tables in non-public schemas need explicit schema qualification in `CREATE POLICY`. The migration tool should handle this. No test exists. | Integration |
| M7 | **Multiple entities in the same service generating policies with the same name** | If two entities in the same service both generate policies, they must have unique names. The migration tool should deduplicate or fail. No test. | Unit / Integration |

### 2.4 Derive Macro (`lifeguard-derive`)

| # | Edge Case | Risk | Test Type |
|---|-----------|------|-----------|
| D1 | **Unknown `#[rls_...]` attributes** | If someone writes `#[rls_enabled = true]` on an entity, the derive currently **ignores** unknown attributes silently (it parses only known ones). This is OK for forward-compatibility but a test should verify that the macro *does* ignore unknown attributes rather than panicking. | Unit |
| D2 | **`#[skip]` on a column that `rls_set_session` might need** | Not applicable — `rls_set_session` only needs session data, not entity columns. But if future RLS attributes reference entity columns, this could be an issue. | N/A (future) |
| D3 | **`#[soft_delete]` on an RLS-enabled table** | Soft delete is usually implemented via a query filter. With RLS, a policy could enforce it. Need to verify the migration tool generates the soft delete column alongside the table. | Integration |
| D4 | **Entity with `#[composite_unique]` on RLS-enabled table** | Composite unique constraints interact with indexes. A test should verify that the generated SQL includes both the composite unique and the RLS enablement in the correct order. | Integration |

---

## 3. Test Plan

### 3.1 Priority 1 — Unit Tests (No Database Required)

These are quick to write, fast to run, and catch regressions immediately.

#### P1.1: `MayPostgresExecutor` Builder Tests
```
test_new_has_null_session_context()        // Verify session_context is None by default
test_with_session_context_sets_field()     // Verify the builder returns self with context set
test_with_session_context_chainable()      // Verify method chaining (with_session_context(...).with_session_context(...))
```
**File:** `src/executor.rs` (existing `#[cfg(test)]` module)

#### P1.2: `SessionContext` Static Trait Tests
```
test_session_context_is_send()             // assert: SessionContext: Send
test_session_context_is_sync()             // assert: SessionContext: Sync
test_session_context_is_clone()            // assert: SessionContext: Clone
test_session_context_is_default()          // assert: SessionContext: Default (empty context)
```
**File:** `src/executor.rs` (existing `#[cfg(test)]` module)

#### P1.3: `WorkerJob` Session Field Tests
```
test_worker_job_execute_has_session_none()          // Verify construct with session: None
test_worker_job_queryone_has_session_none()         // Same for QueryOne
test_worker_job_queryall_has_session_none()         // Same for QueryAll
test_worker_job_with_enqueued_at_preserves_session  // Verify session field is preserved
```
**File:** `src/pool/pooled.rs` (new `#[cfg(test)]` module for RLS)

#### P1.4: `PooledLifeExecutor` Builder Tests
```
test_pooled_executor_new_has_null_session_context()
test_pooled_executor_with_session_context_sets_field()
```
**File:** `src/pool/pooled.rs`

#### P1.5: `lifeguard-migrate` Golden Test — Non-RLS Baseline
```
test_generate_from_entities_non_rls_baseline()  // Generate SQL from a known entity, compare against golden file. Proves no regression from adding RLS attributes later.
```
**File:** `lifeguard-migrate/tests/test_sql_generation.rs` (new test case)

### 3.2 Priority 2 — Integration Tests (Database Required)

These require a live PostgreSQL instance and extend the existing `rls_integration.rs`.

#### P2.1: Direct Executor — Edge Cases
```
test_direct_executor_empty_context_allows_all_rows()  // E1: All fields None → policy treats NULL as "allow all"
test_direct_executor_permissions_only()               // E3: Only permissions set, no user/org → policy decision
test_direct_executor_no_rls_function_returns_error()  // E6: Drop rls_set_session, verify query_one returns error
```

#### P2.2: Transaction — Edge Cases
```
test_transaction_rollback_clears_session_context()    // T1: rollback() then begin() should not carry context
test_transaction_serializable_with_session()          // T2: begin_with_session with Serializable isolation
test_transaction_nested_savepoint_preserves_context() // E4: savepoint rollback inside RLS transaction
```

#### P2.3: Pool Worker — Edge Cases
```
test_pool_rapid_context_switching()                   // E5: Two consecutive requests with different tenants, verify no cross-contamination
test_pool_worker_fails_when_rls_function_missing()    // E6: Pool worker error when rls_set_session not found
```

#### P2.4: Schema Inference & Comparison — RLS Aware
```
test_infer_schema_ignores_rls_state()                 // M3: infer-schema on a table with ENABLE ROW LEVEL SECURITY produces same Rust sketch
test_compare_schema_ignores_policies()                // M4: compare-schema on a DB with policies emits no false drift
```

### 3.3 Priority 3 — Migration Tool Tests

#### P3.1: `lifeguard-derive` Attribute Ignorance
```
test_derive_ignores_unknown_attributes()              // D1: Entity with #[unknown_attr = "foo"] compiles without error
```
**File:** `lifeguard-derive/src/` (existing derive tests)

#### P3.2: `entity_loader` Robustness
```
test_entity_loader_skips_malformed_entity()           // M2: Entity with broken derive is skipped, not panic
```
**File:** `lifeguard-migrate/tests/test_entity_loader.rs`

---

## 4. What We Will NOT Test (Out of Scope)

| Item | Reason |
|------|--------|
| **`rls_set_session` SQL function correctness** | It's app-owned migration code, not Lifeguard code. Hauliage owns the SQL migration. |
| **RLS policy definitions** | `CREATE POLICY USING (...)` is schema-specific and app-owned. |
| **JWT claim extraction in middleware** | That's the consuming application's responsibility. Lifeguard only receives `SessionContext`. |
| **`compare-schema` full policy drift detection** | `pg_policies` is tracked as a future roadmap item, not this PRD. |

---

## 5. Summary

### Current Coverage

| Layer | Unit | Integration | Migration |
|-------|------|-------------|-----------|
| `SessionContext` | ✅ 3 tests | — | — |
| `MayPostgresExecutor` | ❌ 0 tests | ✅ (via integration) | — |
| `Transaction` | ❌ 0 tests | ✅ (via integration) | — |
| `PooledLifeExecutor` | ❌ 0 tests | ✅ (via integration) | — |
| `WorkerJob` | ❌ 0 tests | — | — |
| `lifeguard-derive` | ❌ | — | ❌ |
| `lifeguard-migrate` | ❌ | ❌ | ❌ |

### Effort Estimate

| Priority | Tests | Est. Time |
|----------|-------|-----------|
| P1 (Unit) | ~12 tests | 2 hours |
| P2 (Integration) | ~9 tests | 4 hours |
| P3 (Migration) | ~3 tests | 1 hour |
| **Total** | **~24 tests** | **~7 hours** |

### Decision Points

1. **Should `lifeguard-derive` emit compile-time warnings for unknown `rls_*` attributes?** Currently unknown attributes are silently ignored. A future `#[rls_enabled]` attribute could conflict. **Recommendation:** No change now. Track as a follow-up.
2. **Should `lifeguard-migrate` generate `ALTER TABLE ... ENABLE ROW LEVEL SECURITY` automatically?** Currently no RLS attributes exist on entities, so the answer is "not yet." **Recommendation:** Block this decision until Hauliage defines the entity-level RLS annotation.
3. **Should `compare-schema` detect `pg_policies` drift?** Currently ignored. **Recommendation:** Low priority. Log as a roadmap item.
