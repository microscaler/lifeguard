# Security assessment prompt (reusable)

**Purpose:** Copy the block below into an AI assistant, internal review ticket, or external penetration-test brief when you want a **structured security review** of the Lifeguard workspace. The audience for the **output** should be **CTOs, engineering leads, and CSOs**—executive clarity, evidence-backed, and actionable.

**Scope note:** This repository is the **Lifeguard** PostgreSQL ORM/data-access platform for Rust (`may` / `may_postgres`), with optional **Redis** (cache / LifeReflector), **migrations**, **GraphQL** (optional), and **CI/Compose** test infrastructure. Adjust scope in your paste if you audit only a subdirectory or a release tag.

---

## Prompt to submit (copy from here)

```text
As an expert in IT security and vulnerability assessment, analyse the Lifeguard codebase (Rust PostgreSQL ORM / data platform: lifeguard crate, lifeguard-derive, lifeguard-migrate, lifeguard-reflector, examples, tests).

Write the assessment in the prose of an external security audit consultancy. The audience is CTOs, engineering leads, and CSOs. Use clear executive summaries where appropriate.

For each security concern, provide a table with:
- Location (file path and, where helpful, symbol or feature name)
- Concern (what is risky)
- Potential exploit (realistic attacker scenario or failure mode)
- Possible remediation (controls, design changes, or operational mitigations)

Where possible, cite hyperlinks to:
- Relevant CWE entries
- OWASP categories or project pages
- Known CVEs or vendor advisories **only when directly applicable** (e.g. a dependency with a tracked CVE); avoid speculative CVE linkage.

Cover at minimum:
1. **Injection and query construction** — raw SQL APIs, string-built SQL, SeaQuery/ORM paths, migration SQL
2. **Secrets and configuration** — connection strings, env handling, logging of sensitive data
3. **Memory safety and `unsafe`** — any `unsafe` blocks, `unsafe impl`, FFI
4. **Concurrency and TOCTOU** — pool dispatch, session/identity map, replica/WAL routing
5. **Supply chain** — `cargo`/git dependencies, pinned revisions, optional features
6. **Denial of service** — unbounded work, pool exhaustion, channel backpressure
7. **Cache / Redis / LifeReflector** (if present) — cache poisoning, invalidation, NOTIFY abuse
8. **GraphQL surface** (if enabled) — introspection, depth/complexity, authz (consumer responsibility)

Explicitly state **consumer responsibilities** (e.g. this library does not implement application authentication).

Do **not** modify source code in your response; recommendations only.

Deliver:
- Executive summary (1–2 paragraphs)
- Detailed findings in tabular form as specified
- Residual risk and suggested next steps (e.g. SAST, dependency scanning cadence, threat model for deployment)
```

---

## How to use this file

| Step | Action |
|------|--------|
| 1 | Pin a **commit SHA** or **release tag** in your audit ticket so results are reproducible. |
| 2 | Paste the prompt into your tool of choice; attach or allow read access to the repo. |
| 3 | For **external** auditors, add: deployment topology (internet-facing or not), data classification, and whether GraphQL/Redis/reflector are in use. |
| 4 | Store the generated report under version control (e.g. `docs/security/`) with **date** and **scope**. |

---

## Appendix A — Representative themes observed in this workspace (non-exhaustive)

*This appendix is **illustrative** for scoping future runs. It is **not** a substitute for a full assessment on your revision. Locations may shift as the code evolves.*

### A.1 Themes to review (mapping to code areas)

| Theme | Example locations / notes |
|-------|---------------------------|
| **Raw / unprepared SQL** | `src/raw_sql.rs` — `execute_unprepared` passes through to `executor.execute(sql, &[])`; consumer misuse enables classic SQL injection if user input is concatenated into `sql`. |
| **Parameterized vs embedded values** | `src/relation/eager.rs` — generated SQL fragments use `value_to_sql_string` / `Expr::cust` in places; code comments note embedding values is not ideal. Review for any path where **untrusted** data influences SQL text. |
| **`unsafe`** | `src/session/identity_model_cell.rs` — `unsafe impl Send`; `lifeguard-reflector` may use `unsafe` for FFI/cache — review soundness and thread contracts. |
| **Migration / lock SQL** | `src/migration/lock.rs` — `format!` with `LOCK_VERSION` constant (not user input); still verify no future refactor introduces interpolation of untrusted input. |
| **Dependency posture** | Root `Cargo.toml` — git-pinned `may_postgres`; direct `protobuf = "3.7.2"` with comment referencing **CVE-2025-53605** — keep `cargo audit` / `cargo deny` in CI. |
| **Cryptography** | `sha2` for migration checksums — appropriate for integrity, not for passwords; ensure no misuse as a KDF. |
| **Test-only mocks** | `src/macros/mock.rs` — `format!` building SQL for tests; must not ship to production paths. |

### A.2 Reference links (generic classes; not product-specific CVEs unless tied to a dependency)

| Topic | Reference |
|-------|-----------|
| SQL injection (class) | [CWE-89: Improper Neutralization of Special Elements used in an SQL Command](https://cwe.mitre.org/data/definitions/89.html) |
| Injection (general) | [OWASP Top 10 A03:2021 – Injection](https://owasp.org/Top10/A03_2021-Injection/) |
| Deserialization | [CWE-502](https://cwe.mitre.org/data/definitions/502.html) / secure serde usage |
| Protobuf advisory (if using protobuf) | Track vendor advisories for the **pinned** `protobuf` crate version in `Cargo.toml` |

### A.3 Sample findings table (illustrative — **re-validate on each audit**)

| Location | Concern | Potential exploit | Possible remediation |
|----------|---------|-------------------|----------------------|
| `src/raw_sql.rs` (`execute_unprepared`, etc.) | Unprepared execution of caller-supplied SQL strings | Attacker-controlled string concatenated into SQL → **CWE-89** | Prefer `execute_statement` / parameterized APIs; static analysis for call sites; never pass untrusted input into `execute_unprepared` |
| Consumer applications using Lifeguard | No built-in authn/authz | Broken access control at app layer | Enforce authz in application; use least-privilege DB roles; row-level security in PostgreSQL where appropriate |
| `Cargo.toml` / lockfile | Transitive vulnerabilities | Known CVEs in dependencies | `cargo audit`, Dependabot, pin upgrades; review git deps (`may_postgres`) |
| Optional Redis / NOTIFY / reflector | Cache and messaging trust boundaries | Cache poisoning or stale reads if misconfigured | TLS to Redis where required, auth secrets, network segmentation; document trust model |
| `unsafe impl Send` (`SessionIdentityModelCell`) | Protocol contract not enforced by types | Data races / UB if session + record used across threads incorrectly | Document (done); long-term: `Arc<Mutex<>>` or API split if multi-threaded session is required |

---

## Appendix B — Document control

| Field | Value |
|-------|--------|
| Template version | 1.0 |
| Intended use | Lifeguard repository security assessments |
| Code changes | **None** required by this file; it is a prompt + guidance only |

When you complete an audit, add a row here or in `docs/security/`:

| Date | Scope (commit/tag) | Report location | Owner |
|------|----------------------|-----------------|-------|
| *—* | *—* | *—* | *—* |
