# PRD: JSF-aligned panic safety (unwrap / expect / unreachable)

**Status:** Phase 1–3 items below implemented on branch (see §11 verification log). Phase 4 open.  
**Audience:** Lifeguard maintainers and production integrators  
**References:** [JSF AV Rules (Stroustrup)](https://www.stroustrup.com/JSF-AV-rules.pdf) AV Rule 208 (no exceptions); Microscaler `BRRTRouter/docs/JSF_COMPLIANCE.md`, `BRRTRouter/docs/JSF/JSF_WRITEUP.md` §3 (error handling); Lifeguard `src/lib.rs` crate-level Clippy denies.

---

## 1. Executive summary

Lifeguard’s main crate already denies `clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`, `clippy::todo`, and `clippy::unimplemented` at the library root. Remaining work to align with **JSF-style “no surprise panics on operational paths”** concentrates on: (1) **`unreachable!` in runtime library code**, (2) **macro-generated user code that panics** (e.g. `.expect` on required fields), (3) **public APIs that still use `expect`**, (4) **optional startup-only patterns** (metrics), and (5) **proc-macro hygiene** in `lifeguard-derive`.

This document is the **product requirements** and **phased implementation plan** for that hardening.

---

## 2. Goals

| ID | Goal |
|----|------|
| G1 | **Operational paths** (query execution, row mapping, relation building from valid defs, model conversion) must not terminate the process via `unwrap` / `expect` / `panic!` / `unreachable!` when failures are **expected or recoverable**. |
| G2 | Failures are **explicit**: `Result` / typed errors (`LifeError`, `ActiveModelError`, `may_postgres::Error`, etc.) with stable messages suitable for logs and API responses. |
| G3 | **Compile-time** macro failures prefer **`syn::Error` + `compile_error!` style output** over ad-hoc `panic!` where practical, for consistent diagnostics and grep-friendly macro code. |
| G4 | **Policy is enforceable**: extend or preserve Clippy denies so regressions are caught in CI (`cargo clippy --all-targets --all-features` with project-standard `-D warnings` / pedantic as applicable). |

---

## 3. Non-goals

| ID | Non-goal | Rationale |
|----|----------|-----------|
| NG1 | Eliminate **all** `unwrap` in **test-only** code | Already scoped with `#[cfg(test)]` / local `#[allow(...)]`; keep test ergonomics. |
| NG2 | Full **AV Rule 206** (“no heap after init”) for the ORM | Separate performance/determinism epic; this PRD is **control-flow / panic safety**. |
| NG3 | Guarantee **no abort** if the host sets `panic = abort` | Out of scope for the library; document integrator responsibilities for `catch_unwind` at FFI boundaries if needed. |
| NG4 | Change **every** proc-macro internal `unwrap` in one release | `syn`/`quote` ergonomics; phased reduction with measurable hotspots. |

---

## 4. Current baseline (summary)

| Layer | Posture |
|-------|---------|
| **`lifeguard` crate root** | `#![deny(clippy::unwrap_used, expect_used, panic, unimplemented, todo)]` — strong default. |
| **Scoped allows** | Documented exceptions: e.g. `MetricsCollector::init`, test modules. `RelationTrait::has_many_through_with_def` now returns `Result` (no `expect`). |
| **`unreachable!`** | Not covered by `clippy::panic`; **`JsonValue::from_row`** uses `unreachable!` after forced `try_get` errors — treat as **P0**. |
| **`lifeguard-derive`** | No equivalent crate-wide deny; mix of `unwrap`/`expect`/`panic!` at **expand** time and **emitted** `.expect(...)` in user binaries — **P0/P1** for emitted code. |
| **Binaries** (`lifeguard-migrate`, `lifeguard-codegen`) | `unreachable!` in matches — review for exhaustiveness or replace with `Err`. |

---

## 5. Functional requirements

### 5.1 Runtime library (`lifeguard` src)

| Req ID | Requirement | Acceptance criteria |
|--------|-------------|---------------------|
| R1.1 | **`FromRow for serde_json::Value`** must not use `unreachable!` for control flow on error paths. | **Done:** `Result` only (OOB column index for bad JSON; empty-row path); integration tests `json_value_from_row_*`; `#![deny(clippy::unreachable)]` on crate. |
| R1.2 | **Relation APIs** that today `expect` on `join_on_exprs()` should surface errors via **`Result`** when the failure is a **misconfiguration** rather than a logic bug. | **Done:** `RelationTrait::has_many_through_with_def` → `Result<SelectQuery<_>, LifeError>`; rustdoc **Errors** section; `expect` allow removed. |
| R1.3 | **`MetricsCollector::init`** either remains **documented fail-fast** (explicit allow + MSRV docs) **or** exposes **`Result`** so embedders choose policy. | **Decision (2026-03-28):** retain fail-fast `expect` + scoped allow + doc; optional `try_init` → `Result` left for a later iteration. |

### 5.2 Generated user code (`lifeguard-derive` output)

| Req ID | Requirement | Acceptance criteria |
|--------|-------------|---------------------|
| R2.1 | **Required field** materialization in `try_into_model` / insert paths must not rely on **runtime `.expect("Field … is required")`** as the only failure mode when the public API is already fallible. | **Done:** `to_model()` → `Result` + `ActiveModelError::FieldRequired`; insert uses `to_model()?`; tests in `lifeguard-derive` + integration wrapper updated. |
| R2.2 | **`graph_mut` default** remains `unimplemented!` only behind **documented** “must use derive” contract; no new default callable paths without compile failure. | Unchanged or stricter; rustdoc unchanged intent. |

### 5.3 Proc-macro crate (`lifeguard-derive` implementation)

| Req ID | Requirement | Acceptance criteria |
|--------|-------------|---------------------|
| R3.1 | **User-attributable** failures (bad paths, invalid attrs) use **`syn::Error`** and **`TokenStream::from(e.to_compile_error())`** (or project-standard pattern) instead of `panic!` where feasible. | Spot-check: relation / field parsing hotspots; no behavior change for valid crates. |
| R3.2 | Add **incremental** lint policy: e.g. `#![deny(clippy::unwrap_used)]` on modules where `syn` patterns allow it, or file-level allows with **tracked tech-debt list** in this doc’s checklist. | CI passes; remaining allows documented with owner + follow-up ID. |

### 5.4 Binaries

| Req ID | Requirement | Acceptance criteria |
|--------|-------------|---------------------|
| R4.1 | Each **`unreachable!`** in `lifeguard-migrate` / `lifeguard-codegen` is **justified** (comment + proof of exhaustiveness) **or** replaced with **`anyhow::Error` / `eprintln!` + non-zero exit** as appropriate. | Reviewer can verify match arms; no silent `unreachable` on user-driven input without prior validation. |

---

## 6. Non-functional requirements

| Req ID | Requirement |
|--------|-------------|
| N1 | **Breaking changes** (public signature changes) are listed in **`CHANGELOG.md`** (or project equivalent) with migration snippets. |
| N2 | **Test coverage** for new error branches meets project minimum (≥65% module coverage target per repo rules; new lines covered). |
| N3 | **`cargo clippy`** for affected crates passes with the **same CI profile** as `main` (including `-D warnings` where enforced). |

---

## 7. Implementation plan (phased)

Dependencies: **Phase 1** is independent; **Phase 2** may depend on error-type design; **Phase 3** can run parallel to Phase 2 after R2.1 design is agreed; **Phase 4** is ongoing hygiene.

### Phase 1 — Quick wins (library runtime)

| Task | Owner | Primary files | Done when |
|------|-------|---------------|-----------|
| P1.1 Replace `unreachable!` in `JsonValue::from_row` with proper `Err` | TBD | `src/query/traits.rs` | R1.1 satisfied |
| P1.2 Optional: `#![deny(clippy::unreachable)]` on `lifeguard` root or per-module after P1.1 | TBD | `src/lib.rs` | CI green; or deferred with written rationale |
| P1.3 Audit `grep unreachable!` in workspace crates | TBD | `lifeguard`, `lifeguard-migrate`, `lifeguard-codegen` | R4.1 checklist filled |

**Exit gate:** `cargo test -p lifeguard --lib` green; Clippy green for `lifeguard`.

---

### Phase 2 — Public API: relation + metrics (breaking / policy)

| Task | Owner | Primary files | Done when |
|------|-------|---------------|-----------|
| P2.1 Design **`has_many_through_with_def` → Result** (name, error type, deprecation of old method if dual-ship) | TBD | `src/relation/traits.rs`, call sites, rustdoc | R1.2 satisfied; allow removed |
| P2.2 Decide **`MetricsCollector::init`**: keep fail-fast **or** `Result` API | TBD | `src/metrics.rs`, callers | R1.3 satisfied |
| P2.3 Migration note for integrators | TBD | `CHANGELOG` / README observability section | Published |

**Exit gate:** `cargo test -p lifeguard` (including integration targets that use relations/metrics) green.

---

### Phase 3 — Generated code: no panic on missing required fields

| Task | Owner | Primary files | Done when |
|------|-------|---------------|-----------|
| P3.1 Inventory **all** `.expect(` / `unwrap(` emitted by `LifeRecord` / `try_into_model` / related macros | TBD | `lifeguard-derive/src/macros/life_record.rs`, etc. | Table in appendix of this doc or linked derive doc |
| P3.2 Implement **Result-based** validation consistent with `ActiveModelError` (or chosen type) | TBD | derive + any runtime helpers | R2.1 satisfied; tests added |
| P3.3 Regenerate / fix **examples** and **integration tests** that assumed panic | TBD | `examples/`, `tests/` | Green CI |

**Exit gate:** `cargo test -p lifeguard-derive` + relevant `db_integration` / entity examples green.

---

### Phase 4 — Proc-macro crate hygiene (ongoing)

| Task | Owner | Primary files | Done when |
|------|-------|---------------|-----------|
| P4.1 Replace `panic!` in **relation** expansion with `syn::Error` where not already done | TBD | `lifeguard-derive/src/macros/relation.rs` | R3.1 for that module |
| P4.2 Reduce `field.ident.as_ref().unwrap()` — use `syn::Error` for tuple struct fields where invalid | TBD | `life_model.rs`, `life_record.rs`, `from_row.rs`, … | Invalid input → compile error, not ICE |
| P4.3 Introduce **deny** or **warn** policy for `unwrap` in `lifeguard-derive` with allow list | TBD | `lifeguard-derive/src/lib.rs` or submodules | R3.2 satisfied |

**Exit gate:** Clippy policy documented; CI stable.

---

## 8. Risks and mitigations

| Risk | Mitigation |
|------|------------|
| **Breaking** `has_many_through_with_def` callers | Deprecation alias for one release cycle if policy allows; clear changelog |
| **Performance** of extra `Result` plumbing on hot path | Benchmark only if flagged; relation builder is not per-row in the same way as `FromRow` |
| **Macro ICE** when replacing `panic` with `syn::Error` | Incremental PRs per macro; compile tests in `lifeguard-derive` |
| **Test churn** from panic → Err | Update tests to `assert!(matches!(..., Err(_)))` or `unwrap_err` patterns |

---

## 9. Verification matrix (release checklist)

| Check | Command / action |
|-------|-------------------|
| Library + integration + bins | `RUST_TEST_THREADS=1 cargo test --workspace --lib --tests --bins` (single thread avoids flaky SIGSEGV with shared testcontainers in `db_integration_suite`) |
| Derive tests | `cargo test -p lifeguard-derive` |
| Integration only | `RUST_TEST_THREADS=1 cargo test -p lifeguard --test db_integration_suite` |
| Lint | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| Doc tests | `cargo test -p lifeguard --doc` (known: many doctests fail to compile today; track separately from this PRD) |
| Doc build | `cargo doc -p lifeguard --no-deps` — “Panics” / “Errors” sections updated for changed APIs |

---

## 10. Execution checklist (granular — check off as work lands)

### Phase 1 — Library runtime
- [x] **P1.1a** — Replace `unreachable!` in `JsonValue::from_row` (`src/query/traits.rs`) with `Result` paths (empty row + invalid JSON).
- [x] **P1.1b** — Add integration coverage: `tests/db_integration/json_value_from_row.rs` + `db_integration_suite` module.
- [x] **P1.1c** — Run full verification after P1.1 (see §11 row P1.1).
- [x] **P1.2a** — Add `#![deny(clippy::unreachable)]` to `lifeguard` crate root (`src/lib.rs`).
- [x] **P1.2b** — Run full verification after P1.2 (see §11 row P1.2).
- [x] **P1.3a** — Document workspace `unreachable!` sites (see §13).
- [x] **P1.3b** — Run full verification after P1.3 (see §11 row P1.3).

### Phase 2 — Public API + policy
- [x] **P2.1a** — `RelationTrait::has_many_through_with_def` → `Result<SelectQuery<R>, LifeError>`; remove `expect` allow; update rustdoc.
- [x] **P2.1b** — Run full verification after P2.1 (see §11 row P2.1).
- [x] **P2.2** — **Decision:** keep `MetricsCollector::init()` fail-fast with scoped `expect` + doc (no `Result` API in this iteration).
- [x] **P2.3** — Migration notes for breaking API (§12 below; no repo `CHANGELOG.md` today).

### Phase 3 — Generated record / model conversion
- [x] **P3.1** — Emit inventory for macro runtime panic/unwrap patterns (§14).
- [x] **P3.2a** — Add `ActiveModelError::FieldRequired(String)`; `LifeRecord` `to_model()` → `Result<Model, ActiveModelError>`; insert path uses `to_model()?`.
- [x] **P3.2b** — Derive + integration tests updated (`test_minimal`, `active_model_crud` wrapper).
- [x] **P3.2c** — Run full verification after P3.2 (see §11 row P3.2).
- [x] **P3.3** — Examples: none referenced `to_model()` directly; **N/A** beyond derive/integration tests.

### Phase 4 — Proc-macro hygiene (not started)
- [ ] **P4.1** — Relation macro: `panic!` → `syn::Error` where feasible.
- [ ] **P4.2** — Tuple-struct field names: `unwrap()` → proper errors.
- [ ] **P4.3** — `lifeguard-derive` Clippy policy (`unwrap_used` deny + scoped allows).

---

## 11. Verification log (run after each completed PRD slice)

Record commands actually used on **2026-03-28** after implementing §10 items:

| After item | Command | Result |
|------------|---------|--------|
| P1.1 | `RUST_TEST_THREADS=1 cargo test --workspace --lib --tests --bins` | pass |
| P1.2 | same + `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| P1.3 | same (doc-only delta) | pass |
| P2.1 | same | pass |
| P2.2 / P2.3 | policy/docs only; same test run as P2.1 | pass |
| P3.2 | same | pass |

**Note:** `cargo test --workspace` default parallelism hit **SIGSEGV** in `db_integration_suite` in this environment; serial tests are the reliable “full suite” until harness isolation improves.

---

## 12. Migration notes (breaking changes in this PRD tranche)

1. **`RelationTrait::has_many_through_with_def`** now returns `Result<SelectQuery<R>, LifeError>`. Callers must use `?` or `map_err` instead of assuming a `SelectQuery`.
2. **`LifeRecord::to_model`** (generated by `#[derive(LifeRecord)]`) now returns `Result<Model, ActiveModelError>`. Use `to_model()?` / `.expect(...)` in tests / match on `ActiveModelError::FieldRequired(_)`.
3. New variant **`ActiveModelError::FieldRequired(String)`** — include in any exhaustive `match` on `ActiveModelError` if you maintain one outside the crate.

---

## 13. Appendix: `unreachable!` in workspace binaries / tools

| Location | Role | Assessment |
|----------|------|------------|
| `lifeguard-migrate/src/main.rs` | Inner `match cli.command` uses `_ => unreachable!()` after arms for DB commands | Unreachable at runtime because outer `match` already routed `Generate*` elsewhere; safe but brittle if `Commands` enum gains variants — prefer listing remaining variants explicitly in a future refactor. |
| `lifeguard-codegen/src/main.rs` | `_ => unreachable!()` inside nested `match` on `field.ty` for unsigned integers | Logically exhaustive given outer match; same “add explicit arms if types expand” guidance. |

No change required for P1.3 beyond documentation unless we want to eliminate `unreachable!` in binaries for style parity.

---

## 14. Appendix: `lifeguard-derive` emit / macro inventory (P3.1)

Runtime **user** code previously emitted `.expect(...)` for required `LifeRecord` fields — **removed** in P3.2 (`to_model` → `Result`).

Remaining macro implementation sites (expand-time or **still emitted**):

| File | Pattern | Notes |
|------|---------|--------|
| `life_record.rs` | `field.ident.unwrap()`, `graph` `as_mut().unwrap()` | Expand-time / generated `graph_mut`; follow-up under P4. |
| `life_model.rs`, `from_row.rs`, `try_into_model.rs`, `partial_model.rs` | `field.ident.unwrap()` | Expand-time for named fields; tuple structs → P4.2. |
| `relation.rs` | `.expect(...)`, `panic!`, `parse_str(...).unwrap()` | Mostly compile-time / error paths; P4.1. |

---

## 15. Related internal docs

- `LIFEGUARD_GAP_ANALYSIS.md` — product scope (orthogonal but informs priority)
- `docs/planning/lifeguard-derive/DESIGN_TRY_INTO_MODEL.md` — try_into / validation design
- `docs/planning/audits/LIFEGUARD_FOUNDATION_CONTINUATION.md` — planning doc style reference

---

*PRD updated 2026-03-28 with execution checklists, verification log, and appendices. Phase 4 remains open.*
