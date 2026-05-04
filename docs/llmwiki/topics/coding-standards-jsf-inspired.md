# JSF AV rules — how we apply them in Lifeguard

> **Status:** ACTIVE
> **Last-synced:** 2026-04-18 — against [`../references/jsf-writeup.md`](../references/jsf-writeup.md), [`../references/jsf-audit-opinion.md`](../references/jsf-audit-opinion.md), [`../references/jsf-compliance.md`](../references/jsf-compliance.md) (copied from BRRTRouter’s JSF distillation).
> **Authority:** Root [`clippy.toml`](../../../clippy.toml) (numeric thresholds) + [`DEVELOPMENT.md`](../../../DEVELOPMENT.md) (clippy / pre-commit workflow) + crate-level `#![deny(...)]` in [`src/lib.rs`](../../../src/lib.rs). References are the *why*; lint config and reviews are the *what*.
> **Related:** [`pragmatic-rust-guidelines.md`](./pragmatic-rust-guidelines.md), [`observability-and-logging.md`](./observability-and-logging.md), [`../../../AGENT.md`](../../../AGENT.md).

## What this page covers

The [Joint Strike Fighter Air Vehicle C++ Coding Standards](https://www.stroustrup.com/JSF-AV-rules.pdf) target safety-critical avionics: predictable performance, bounded complexity, no surprise failures. BRRTRouter distilled a subset into a **BRRTRouter-SAFE** profile for HTTP hot paths. Lifeguard is not an HTTP router, but the same **discipline** applies wherever we touch query execution, connection pooling, derive macros, and migration generation: bounded functions, explicit errors, strong types, and tests that cover dispatch paths.

Raw sources live under [`docs/references/`](../references/); this page is the **Lifeguard-specific** synthesis.

## The six JSF principles we apply here

### 1. Bounded complexity (JSF AV Rule 1, 3)

- **Intent:** Reviewers can reason about a function in one pass; cyclomatic complexity stays bounded.
- **Lifeguard:** `clippy.toml` aligns with the platform stack (`cognitive-complexity-threshold`, `too-many-lines-threshold`, `too-many-arguments-threshold`). Hot paths include SQL generation, `SelectQuery` execution, and pool checkout — keep them shallow and test-covered.

### 2. Allocation discipline (JSF AV Rule 206)

- **Intent:** Avoid heap churn in steady-state hot paths where it harms latency predictability.
- **Lifeguard:** The executor and query builder are allocation-heavy by nature; the rule here is **intentional** allocation (no accidental `format!` / `to_string` in inner loops without profiling). Prefer reusable buffers and `Cow` where the type system allows.

### 3. No exceptions (JSF AV Rule 208)

- **Intent:** Recoverable failures are typed `Result`; control flow stays explicit.
- **Lifeguard:** Library code must not `unwrap` recoverable paths. Crate-level denies in `src/lib.rs` are the backstop; integration tests prove the error surface. See also **Core rule §5** in [`AGENT.md`](../../../AGENT.md) (raw SQL last resort).

### 4. Data and type rules (JSF AV Rule 148, 209, 215)

- **Intent:** Enums and newtypes for finite sets; no magic integer codes for domain state.
- **Lifeguard:** PostgreSQL `UUID` → `uuid::Uuid`; timestamps per [`UUID_AND_POSTGRES_TYPES.md`](../../UUID_AND_POSTGRES_TYPES.md) and [`topics/postgres-scalars-uuid-chrono.md`](./postgres-scalars-uuid-chrono.md). Derive macros must emit predictable SQL — see [`topics/derive-macros-and-attributes.md`](./derive-macros-and-attributes.md).

### 5. Flow control (JSF AV Rule 119)

- **Intent:** No unbounded recursion on attacker-influenced structures.
- **Lifeguard:** Schema / relation graphs are walked with explicit depth or known DAG properties; migration ordering uses the FK graph, not ad-hoc recursion without caps.

### 6. Testing discipline (JSF AV Rule 219-221)

- **Intent:** Every dispatch path gets a test; regressions ship with a test that would have failed.
- **Lifeguard:** [`TEST_INFRASTRUCTURE.md`](../../TEST_INFRASTRUCTURE.md), `tests-integration/`, and per-crate unit tests. New derive or migrate behaviour lands with tests in the same PR.

## What Lifeguard does not copy literally from JSF

- **“No heap after init”** for the entire ORM is not realistic; we interpret Rule 206 as *avoid accidental hot-path allocation*, not zero heap.
- **C++-specific rules** (templates, `goto`) are N/A; see [`../references/jsf-audit-opinion.md`](../references/jsf-audit-opinion.md).

## Cross-repo alignment

| Repo | Role |
|------|------|
| [`microscaler-observability`](../../../../microscaler-observability/) | OTEL adapter crate; same reference bundle under `docs/references/`; owns global telemetry install per its PRD (sibling checkout under `microscaler/`). |
| [`BRRTRouter`](../../../../BRRTRouter/) | HTTP adapter; JSF hot-path focus (sibling checkout). |
| **Hauliage** | [`../../../../hauliage/docs/llmwiki/topics/coding-standards-jsf-inspired.md`](../../../../hauliage/docs/llmwiki/topics/coding-standards-jsf-inspired.md) — composition root and microservices (sibling checkout under `microscaler/`). |

When BRRTRouter or `microscaler-observability` updates shared thresholds or references, update Lifeguard’s [`clippy.toml`](../../../clippy.toml) and this page in the same maintenance pass where practical.

## Open questions

> **Open:** Module-level `#![deny(clippy::unwrap_used)]` on the narrowest hot modules vs workspace-wide policy — follow `docs/planning/audits/PRD_JSF_PANIC_SAFETY.md` when revisiting.
