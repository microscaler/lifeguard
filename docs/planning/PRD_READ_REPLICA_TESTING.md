# PRD: Read-replica testing and CI coverage

**Status:** **In progress** — CI + Compose + Toxiproxy + `pool_read_replica`; R3.3 / G2 covered when `TOXIPROXY_API` is set (see `TEST_INFRASTRUCTURE.md`).  
**Audience:** Lifeguard maintainers, CI owners, and integrators using `LifeguardPool`.  
**References:** [DESIGN_READ_REPLICA_CI_AND_HARNESS.md](./DESIGN_READ_REPLICA_CI_AND_HARNESS.md) (topology, harness, workflow); [TEST_INFRASTRUCTURE.md](../TEST_INFRASTRUCTURE.md) (env conventions); `src/pool/pooled.rs`, `src/pool/wal.rs`.

---

## 0. Progress at a glance

Use this section for a quick rollup; detailed checkboxes appear under each major section.

### Documentation

- [x] PRD published (goals, env contract, scenarios, NFRs)
- [x] Design doc published and cross-linked ([DESIGN_READ_REPLICA_CI_AND_HARNESS.md](./DESIGN_READ_REPLICA_CI_AND_HARNESS.md))
- [x] `TEST_INFRASTRUCTURE.md` points at PRD + design + **local runbook** for replica tests

### Implementation

- [x] Repo-local topology: [`.github/docker/docker-compose.yml`](../../.github/docker/docker-compose.yml) (primary + streaming replica + Redis for CI)
- [x] CI: compose up → `--wait` → migrations on primary → tests → `compose down -v` (`if: always()`)
- [x] `tests/context.rs`: `TEST_REPLICA_URL` → `replica_pg_url: Option<String>`
- [x] Replication sync helper: `tests/db_integration/replication_sync.rs` (LSN replay wait)
- [x] Integration module `pool_read_replica` (R3.1–R3.3); R3.3 uses Toxiproxy to disable the replica proxy (`pooled_read_falls_back_to_primary_when_replica_lagging`) when `TOXIPROXY_API` is set
- [ ] Optional: unit / injectable tests for `WalLagMonitor` / `read_tier` (follow-up to design §6)

---

## 1. Executive summary

`LifeguardPool` routes writes to a **primary** connection tier and optionally routes **read** queries (`LifeExecutor::query_one_values` / `query_all_values` via `PooledLifeExecutor`) to **replica** workers when replica URLs are configured and `WalLagMonitor` considers the standby safe. **Integration coverage:** `tests/db_integration/pool_read_replica.rs` runs when `TEST_REPLICA_URL` is set (CI with Docker Compose). There are still **no** dedicated unit tests in `pooled.rs` / `wal.rs`.

This PRD defines **product-level requirements** for closing that gap: a **known primary + replica topology** in GitHub Actions, a **replication sync gate** before assertions that depend on the replica seeing writes, **documented test environment variables**, and a **minimum scenario set** so regressions in routing or lag handling are caught in CI.

---

## 2. Problem statement

| Risk | Description |
|------|-------------|
| Silent misconfiguration | Integrators pass empty replica lists or wrong URLs; all traffic stays on primary with no failing test. |
| Incorrect routing | Reads hit replica when lagging, or never hit replica when healthy—both harm correctness or cost. |
| Untested `WalLagMonitor` | Lag threshold, polling, and reconnect paths in `wal.rs` are not validated against a real `pg_is_in_recovery()` standby. |
| CI blind spot | **Mitigated (2026-03-28):** `test` job uses Compose primary+replica + `TEST_REPLICA_URL`. Fork PRs without secrets remain degraded (unchanged). |

---

## 3. Goals

| ID | Goal |
|----|------|
| G1 | **Prove** that `LifeguardPool` + `PooledLifeExecutor` send mutations to the primary tier and read queries to the replica tier when the pool is configured with replica URLs and the lag monitor reports the replica as acceptable. |
| G2 | **Prove** fallback behavior when the replica is unsafe (lag over policy, connection/query errors): reads use the primary tier without panicking. |
| G3 | **Exercise** `WalLagMonitor` against a real PostgreSQL standby (in recovery) in at least one automated test environment (CI). |
| G4 | **Document** how developers and CI provide primary vs replica URLs and how tests wait for replication before making assertions. |

**Goal delivery (implementation)**

- [x] **G1** — `pool_read_replica::pooled_pool_construct_write_read_with_replica` (CI + local when env set)
- [x] **G2** — `pooled_read_falls_back_to_primary_when_replica_lagging` (CI: `TOXIPROXY_API` + `TEST_REPLICA_URL` :6547)
- [x] **G3** — `WalLagMonitor` polled in integration test on real standby (`is_replica_lagging` warmup)
- [x] **G4** — `TEST_INFRASTRUCTURE.md` runbook + design doc Compose path

---

## 4. Non-goals (v1)

| ID | Non-goal | Rationale |
|----|----------|-----------|
| NG1 | **Redis** read-through or cache coherence tests | Handled at application / LifeReflector layer; out of scope for this pool-focused PRD. |
| NG2 | **Multiple read replicas** and load-balancing policy | Pool already round-robins URLs; v1 tests one replica URL or a small fixed set; no HA product requirements here. |
| NG3 | **Production HA** (Patroni, failover, automatic promotion) | Test topology is **static** primary + standby for correctness checks, not failover drills. |
| NG4 | Changing the public **`LifeguardPool::new(primary, n_write, replica_urls, n_read)`** signature | Already decided; this PRD assumes the current API. |
| NG5 | **perf-idam** replica benchmarks | Optional follow-up; `perf_orm` may remain primary-only unless explicitly extended. |

---

## 5. Current baseline (coverage)

| Component | Automated coverage today |
|-----------|-------------------------|
| `LifeguardPool`, `PooledLifeExecutor`, `read_tier` / dispatch | **Integration:** `pool_read_replica` (no `pooled.rs` unit tests yet) |
| `WalLagMonitor` | **Integration:** exercised via pool test on standby (no `wal.rs` unit tests yet) |
| `pool/owned_param.rs` | Unit tests exist (parameter encoding); **not** routing |
| `db_integration_suite` | **`pool_read_replica`** with `LifeguardPool` + replica URL when `TEST_REPLICA_URL` set |
| `examples/perf-idam` | `LifeguardPool::new(..., vec![], 0)` — primary-only |

**Coverage closure (check when automated tests land)**

- [x] `LifeguardPool` / `PooledLifeExecutor` / `read_tier` — integration (`pool_read_replica`); unit tests still open
- [x] `WalLagMonitor` — real standby (integration); injectable lag optional
- [x] `db_integration_suite` — `LifeguardPool` with replica URL(s)
- [ ] Optional: extend `perf-idam` or separate bench for replica read path (NG5 — not required for PRD v1)

---

## 6. Test environment contract

### 6.1 Environment variables

| Variable | Semantics |
|----------|-----------|
| `TEST_DATABASE_URL` | Connection string for the **primary** (read-write). Same meaning as today for `tests/context.rs`. |
| `TEST_REPLICA_URL` | **New (v1):** Connection string for a **physical streaming standby** (hot standby) of that primary. Optional for local runs; **required** when CI runs the replica test job/step (see design doc). |

Rules:

- When `TEST_REPLICA_URL` is **unset** or empty, tests that **require** a replica are **skipped** (not failed), unless a dedicated “replica-only” CI job explicitly sets it (then absence is a configuration error for that job).
- Replica URL must use credentials and database name consistent with replication setup (typically same `postgres` DB as primary, read-only on standby).

**Contract implementation checklist**

- [x] `TEST_REPLICA_URL` read in harness; empty → replica tests skipped (or job fails when job is replica-only)
- [x] Documented semantics match behavior in `tests/context.rs` (and CI env)

### 6.2 Safety

- Integration helpers must continue to treat URLs as **test-only**; no fallback from `TEST_DATABASE_URL` to production `DATABASE_URL` in `tests/context.rs` (existing policy preserved).

**Safety checklist**

- [x] Policy stated in this PRD (existing code: no `DATABASE_URL` fallback in `tests/context.rs`)
- [ ] Re-verify after harness changes (no regression introducing `DATABASE_URL` for integration binaries)

---

## 7. Functional requirements

### 7.1 CI topology

| Req ID | Requirement | Acceptance criteria (future implementation) |
|--------|-------------|---------------------------------------------|
| R1.1 | CI provides a **single primary** and **at least one streaming replica** reachable from the test runner (e.g. mapped host ports). | Workflow documentation matches reality; health checks pass before tests. |
| R1.2 | Schema and DDL for shared integration tests run against the **primary** only; replica receives changes via replication (no independent DDL on replica for shared schema). | Migration / `psql` steps documented to target primary URL only. |

### 7.2 Replication sync gate

| Req ID | Requirement | Acceptance criteria (future implementation) |
|--------|-------------|---------------------------------------------|
| R2.1 | Before tests assert row visibility on the replica, the harness **waits** until replication satisfies a defined predicate within a **timeout**. | Predicate documented in design doc (e.g. LSN alignment or `pg_stat_replication` / replay lag below threshold). |
| R2.2 | On timeout, fail with a **clear error** (which URL, last observed lag/LSN, timeout value). | Logs aid CI debugging without reading Rust sources. |

### 7.3 Behavioral coverage (minimum scenarios)

| Req ID | Scenario | Acceptance criteria (future implementation) |
|--------|----------|-----------------------------------------------|
| R3.1 | **Pool construction** with non-empty replica URLs and `replica_pool_size >= 1` against real primary + replica. | `LifeguardPool::new` succeeds in CI; optional: `pg_is_in_recovery()` true on replica connection. |
| R3.2 | **Write then read:** insert/update via `PooledLifeExecutor` (write path), then read via read path after sync gate. | Row visible; test documents whether it asserts replica routing explicitly or primary fallback only (design doc chooses assertion strategy). |
| R3.3 | **Lag / safety path:** force or simulate condition where monitor treats replica as lagging (or inject error) and assert reads still succeed via primary. | No panic; stable `Result` behavior. |

Exact test names and modules are left to implementation; this PRD requires the **behaviors** above to exist and run in CI when replica env is configured.

### 7.4 Local developer parity

| Req ID | Requirement | Acceptance criteria |
|--------|-------------|---------------------|
| R4.1 | Documented way to run the same topology locally (e.g. Docker Compose) with the same env vars. | `TEST_INFRASTRUCTURE.md` links to design doc; compose file path recorded in design doc. |

### 7.5 Requirement delivery checklist

Track each requirement ID as work lands (mirror of §7.1–§7.4).

**CI topology (§7.1)**

- [x] **R1.1** — Primary + streaming replica reachable from runner in CI
- [x] **R1.2** — Migrations / DDL on primary only; replica via WAL

**Replication sync gate (§7.2)**

- [x] **R2.1** — Harness waits for defined predicate within timeout before replica assertions
- [x] **R2.2** — Timeout failures include actionable context (URLs redacted, lag/LSN hints)

**Behavioral coverage (§7.3)**

- [x] **R3.1** — Pool construction with replica URLs + `replica_pool_size >= 1` in CI
- [x] **R3.2** — Write path then read path after sync; row visible (strategy per design doc)
- [x] **R3.3** — Toxiproxy disables replica proxy → monitor marks lagging → reads still succeed via primary (`pooled_read_falls_back_to_primary_when_replica_lagging` when `TOXIPROXY_API` set)

**Local parity (§7.4)**

- [x] **R4.1** — Same topology runnable locally; compose path + env vars documented

---

## 8. Non-functional requirements

| ID | Requirement | Target / note |
|----|-------------|----------------|
| N1 | **Job time** for compose bootstrap + wait | Budget TBD in design doc (e.g. under 2–3 minutes added to job, subject to runner variance). |
| N2 | **Flakes** | Replication wait must tolerate slow CI; bounded retries with backoff; document flake escalation (rerun policy). |
| N3 | **Secrets** | Reuse existing **`PGPASSWORD`** (or equivalent) pattern; avoid new repository secrets unless necessary. |
| N3.1 | **Fork PRs** | Same limitation as today: secrets are not available to workflows from forked repositories; replica CI may be **skipped** or **degraded** on forks—document explicitly (no false “green” claim for replica coverage on forks). |

**NFR delivery checklist**

- [ ] **N1** — Job time budget agreed and documented (compose + sync wait); monitor CI duration
- [x] **N2** — Flake handling: `wait_replica_replayed_at_least` polls with 45s timeout; escalation = re-run job
- [x] **N3** — No unnecessary new secrets; `PGPASSWORD` reused for Compose + URLs
- [x] **N3.1** — Fork PRs: without `PGPASSWORD`, Compose/tests fail or skip as today; replica coverage not claimed on forks without secrets

---

## 9. Acceptance criteria (documentation phase — this PRD)

- [x] Goals, non-goals, and baseline coverage are recorded.
- [x] Env contract (`TEST_DATABASE_URL`, `TEST_REPLICA_URL`) and skip-vs-fail policy are specified.
- [x] Minimum behavioral scenarios and sync gate are specified with traceability to future tests.
- [x] Design doc exists and is linked as the engineering source for topology and harness details.

## 10. Acceptance criteria (implementation phase)

**CI / topology**

- [x] Compose file (or chosen stack) committed and referenced from design doc
- [x] Workflow step(s): `docker compose up` → wait healthy → `docker compose down -v` (`if: always()` on teardown)
- [x] `TEST_DATABASE_URL` / `TEST_REPLICA_URL` exported for test steps (primary port vs replica port)
- [x] Migration / `psql` steps unchanged in intent: target **primary** only

**Harness**

- [x] `LifeguardTestContext` exposes `replica_pg_url` when env set
- [x] Replica-required tests skip cleanly when `TEST_REPLICA_URL` unset (local / optional jobs)
- [x] Replication sync helper used before assertions that need replica visibility

**Tests**

- [x] Integration module covers R3.1–R3.3 (R3.3 via Toxiproxy + `TOXIPROXY_API`)
- [x] `cargo nextest` in CI runs replica test when topology + secrets available
- [ ] Optional follow-up: unit or injectable tests for `WalLagMonitor` / routing (see design §6)

**Docs**

- [x] `TEST_INFRASTRUCTURE.md` — copy-paste runbook: compose up, env exports, command to run replica tests only
- [x] PRD **§0** and **§7.5** checkboxes updated with implementation status

---

## 11. Revision history

| Date | Change |
|------|--------|
| 2026-03-28 | Initial PRD (pre-code). |
| 2026-03-28 | Progress checklists: §0 at-a-glance, §5 coverage closure, G1–G4, §6 contract/safety, §7.5 R*, §8 N*, expanded §10. |
| 2026-03-28 | Implementation: Compose (`bitnamilegacy/postgresql:15`), CI job, `replica_pg_url`, `replication_sync`, `pool_read_replica`, docs. |
