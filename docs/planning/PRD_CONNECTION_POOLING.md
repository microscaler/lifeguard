# PRD: Production-grade Lifeguard connection pooling (Hikari-shaped)

**Status:** **Draft** — **P0 complete** (2026-03-30): as below + R1.3 / R2.2 / R2.3 (defaults + `CHANGELOG`, `LIFEGUARD__DATABASE__*` parity + tests, constructor rustdoc). **P1:** R5.x + **R4.1 / R4.2** complete (2026-03-30); R3+ / full R7 / R8 open.  
**Audience:** Lifeguard maintainers, runtime integrators, and operators sizing Postgres.  
**References:** [systemPatterns.md](../../.agent/memory-bank/systemPatterns.md) (pooling architecture boundary); `src/pool/pooled.rs`, `src/pool/wal.rs`, `src/connection.rs`, `src/config.rs` / `src/pool/config.rs`; [HikariCP configuration](https://github.com/brettwooldridge/HikariCP#gear-configuration-knobs-baby) (conceptual analogue, not API copy).

---

## 0. Progress at a glance

**How to use:** Check boxes as work lands on `main` (or your release branch). Link GitHub issues in commit messages or inline HTML comments if helpful. The **master checklist** is in [§12](#12-master-implementation-checklist-all-requirement-ids).

### 0.1 Milestones

- [x] PRD published (`PRD_CONNECTION_POOLING.md`)
- [ ] Optional design doc `DESIGN_CONNECTION_POOLING.md` (state machine, error taxonomy, metric names)
- [x] **Phase P0** complete (see §7.1)
- [x] **Phase P1** complete (see §7.2)
- [ ] **Phase P2** complete (see §7.3)
- [ ] **Phase P3** complete (see §7.4)
- [ ] [§10 Success criteria](#10-success-criteria-prd-closure) satisfied

### 0.2 Workstream rollup

| Workstream | Done |
|------------|------|
| Acquire timeout + typed overload error | [x] |
| Config merged / wired; no dead `DatabaseConfig` fields | [x] |
| Bounded queues + overload behavior | [x] |
| Slot heal + connectivity classification | [x] |
| Keepalive docs + idle liveness probe | [x] |
| `max_connection_lifetime` (+ jitter) / idle policy | [ ] |
| `WalLagMonitor` retry + tunables + give-up observability | [ ] *partial: initial connect retry only* |
| Metrics + tracing hooks | [ ] |
| Public rustdoc + operator tuning + changelog | [ ] *partial: `CHANGELOG.md`, `LifeguardPool` / `DatabaseConfig::load` rustdoc; operator tuning book TBD* |

---

## 1. Executive summary

`LifeguardPool` today provides **fixed-size** primary and optional replica **worker** tiers (`may` coroutine per slot), **round-robin** dispatch, and **read routing** when `WalLagMonitor` allows. It does **not** yet implement several behaviors operators expect from a **mature in-process pool** (analogous to [HikariCP](https://github.com/brettwooldridge/HikariCP)): **bounded wait** when saturated, **connection rotation** aligned with database and network limits, **liveness** for idle TCP sessions, **recovery** of broken backend sessions, and a **single honest configuration surface** wired into the pool.

This PRD defines **product requirements** to close those gaps while **explicitly not** building PgBouncer-style multiplexing (see §5).

---

## 2. Problem statement

| ID | Gap | User / operator impact |
|----|-----|-------------------------|
| P1 | **No acquire timeout** — `dispatch` blocks indefinitely on `recv()` | Stuck threads/coroutines under load or slow queries; no clear overload signal |
| P2 | **Config not wired** — `DatabaseConfig::{max_connections, pool_timeout_seconds}` unused by `LifeguardPool::new` | TOML/env appear to configure the pool but do not |
| P3 | **Duplicate config types** — `src/config.rs` vs `src/pool/config.rs` | Drift, confusion, inconsistent `LIFEGUARD__` behavior |
| P4 | **No `maxLifetime` / idle retirement** | Sessions live for process lifetime; DB/firewall idle kills, credential rotation, and plan cache bloat harder to manage |
| P5 | **No keepalive / idle liveness** | Half-open TCP; “pool looks full” or mysterious query failures after idle |
| P6 | **No dead-connection recovery** | One bad `Client` in a slot fails until process restart |
| P7 | **Unbounded per-worker job queue** | Memory growth and unbounded latency under spike |
| P8 | **`WalLagMonitor` exits on initial connect failure** | Replica reads never preferred for process lifetime after transient startup failure |
| P9 | **Lag policy hardcoded** (interval, byte threshold) | Operators cannot tune for RPO/RTO or network |
| P10 | **Weak observability** | Hard to prove saturation, timeouts, evictions, heal events |

**Design principle (from architecture decision):** **Connection lifecycle must not be driven by arbitrary SQL failure**; eviction and reopen target **connectivity / lifetime / idle policy**, not statement errors (see existing `wal.rs` comment and Hikari’s separation of validation from business SQL).

---

## 3. Goals

| ID | Goal |
|----|------|
| G1 | **Bounded acquisition:** every pooled dispatch path that waits for a worker must respect a configurable **maximum wait** (default aligned with today’s documented 30s intent), then fail with a **typed, actionable error**. |
| G2 | **Honest configuration:** one **documented** configuration model (file + `LIFEGUARD__` env) maps to **primary size**, **replica size**, **URLs**, and **pool timeouts** without dead fields. |
| G3 | **Connection rotation:** support **maximum connection age** (with jitter) and optional **idle above minimum** retirement so long-lived processes stay within infra and DBA policy. |
| G4 | **Liveness:** idle connections are periodically validated or TCP keepalive is documented and optionally configured so silent disconnects are detected before work is assigned. |
| G5 | **Slot heal:** when a worker determines a session is **not usable** (connectivity-class errors per policy), it may **replace** that slot’s `Client` without restarting the process. |
| G6 | **Back-pressure:** job submission does not grow **unbounded** queues; behavior under overload is **defined** (timeout, reject, or drop-oldest — **decision in design doc**). |
| G7 | **`WalLagMonitor` resilience:** initial connect failure **retries** with backoff; poll interval and lag threshold **configurable**; long-term dead monitor state is **observable**. |
| G8 | **Observability:** metrics or hooks for wait time, timeouts, active/healing slots, evictions, monitor state (feature-gated where appropriate). |
| G9 | **Documentation:** crate and book docs describe limits, tuning, and **non-goals** (PgBouncer); migration notes for API/config changes. |

### 3.1 Goals delivery checklist

Track at milestone reviews; each goal may span multiple PRs.

- [x] **G1** — Bounded acquisition + distinct timeout error
- [x] **G2** — Single honest config surface (file + env) wired to pool
- [ ] **G3** — Connection rotation (`max_connection_lifetime`, jitter; idle policy as designed)
- [x] **G4** — Liveness (docs + optional probes / TCP hooks)
- [x] **G5** — Slot heal on connectivity-class failures only
- [x] **G6** — Bounded queues; defined overload behavior
- [ ] **G7** — `WalLagMonitor` retry, tunables, observable give-up *— retry done; tunables / give-up TBD*
- [ ] **G8** — Metrics / tracing for pool + monitor
- [ ] **G9** — Public docs, tuning guide, migration notes

---

## 4. Non-goals

| ID | Non-goal | Rationale |
|----|----------|-----------|
| NG1 | **PgBouncer-style** client↔server multiplexing, transaction/statement pool modes, fleet-wide queueing | Stand up **PgBouncer** beside the app ([systemPatterns.md](../../.agent/memory-bank/systemPatterns.md)) |
| NG2 | **Prepared statement cache** inside Lifeguard | Prefer driver / Postgres server behavior; Hikari explicitly avoids pool-level statement cache |
| NG3 | **Global query cancel** or **statement_timeout** as pool core | Server-side `statement_timeout` + app policy; pool may document interaction only |
| NG4 | **Changing correctness** of read/write routing semantics | This PRD improves **robustness and operability**, not replica routing rules (covered elsewhere) |
| NG5 | **XA / 2PC** pool | Out of scope for `may_postgres` pool v1 |

---

## 5. Functional requirements

### 5.1 Acquisition and overload

| Req ID | Requirement | Acceptance hint |
|--------|-------------|-----------------|
| R1.1 | Configurable **maximum wait** for obtaining a worker to run a job (per pool or global pool config). | Under artificial stall, caller receives error within `wait + epsilon` |
| R1.2 | Error on timeout is **distinct** from query errors (e.g. `LifeError::Pool` variant or dedicated enum). | Callers/tests can match without string parsing |
| R1.3 | Default wait **matches or documents** migration from `pool_timeout_seconds` (30s default today in config). | Doc + changelog |

**Implementation — §5.1**

- [x] **R1.1** — Maximum wait configurable; enforced on every dispatch wait path
- [x] **R1.2** — Timeout error type distinct from query/`LifeError::Other` paths
- [x] **R1.3** — Default documented; `CHANGELOG` / migration if default changes

### 5.2 Configuration

| Req ID | Requirement | Acceptance hint |
|--------|-------------|-----------------|
| R2.1 | **Single** pool configuration type (or clearly layered types) consumed by pool construction; **remove or merge** duplicate `DatabaseConfig` definitions. | One load path; no conflicting defaults |
| R2.2 | Environment prefix **`LIFEGUARD__`** (or documented successor) maps to the same fields as file config. | Parity test or doc table |
| R2.3 | Pool constructor **either** takes a config struct **or** documents that low-level `::new` is expert-only; avoid two conflicting stories. | Public rustdoc |

**Implementation — §5.2**

- [x] **R2.1** — Duplicate `DatabaseConfig` removed or single source of truth; macros updated
- [x] **R2.2** — `LIFEGUARD__` env parity with file (table test or doc + manual QA checklist)
- [x] **R2.3** — `LifeguardPool` rustdoc: recommended vs expert construction path

### 5.3 Connection lifetime

| Req ID | Requirement | Acceptance hint |
|--------|-------------|-----------------|
| R3.1 | **`max_connection_lifetime`**: optional; `0` = disabled; when set, connections are retired **after** use closes, with **jitter** to avoid thundering herd | Unit/integration test with shortened lifetime + fake clock if available, or observable metric |
| R3.2 | **`idle_timeout`**: optional; only applies above a **minimum idle** floor if a dynamic pool is introduced; if pool stays **fixed-size workers**, document equivalent behavior (e.g. “retire and reconnect slot” vs “shrink pool”) | Doc + test for chosen model |

*Note:* Fixed worker model may implement lifetime as **replace client inside same worker** rather than Hikari-style shrink.

**Implementation — §5.3**

- [ ] **R3.1** — `max_connection_lifetime` (+ jitter); `0` = off; tests or metrics prove rotation
- [ ] **R3.2** — `idle_timeout` / fixed-worker equivalent documented and tested

### 5.4 Liveness and TCP

| Req ID | Requirement | Acceptance hint |
|--------|-------------|-----------------|
| R4.1 | **Keepalive policy**: document OS/driver TCP keepalive for Postgres URLs; optionally expose connection string parameters or builder hooks supported by `may_postgres` / underlying stack | Doc + example config |
| R4.2 | **Idle liveness probe**: periodic check on **idle** slots (interval configurable), using a cheap query or driver equivalent of `isValid()` | Test: idle disconnect simulation recovers via R5.x |

**Implementation — §5.4**

- [x] **R4.1** — Operator doc: TCP keepalive / libpq URL params; code hooks if supported
- [x] **R4.2** — Idle probe interval configurable; integration test or harness for dead TCP

### 5.5 Dead connection and slot heal

| Req ID | Requirement | Acceptance hint |
|--------|-------------|-----------------|
| R5.1 | Classify **connectivity** failures (documented list: e.g. broken pipe, specific codes) vs **application** SQL errors | Table in design doc |
| R5.2 | On connectivity failure, worker **replaces** `Client` for that slot (with retry limit / backoff) without treating generic SQL errors as disconnect | Integration or unit test with injected failure |
| R5.3 | Do **not** use “query failed” alone as heal trigger | Code review checklist |

**Implementation — §5.5**

- [x] **R5.1** — Connectivity vs SQL error taxonomy documented (`src/pool/connectivity.rs`)
- [x] **R5.2** — Worker replaces `Client` on connectivity errors; capped attempt (`POOL_HEAL_MAX_ATTEMPTS`); unit tests for classifier
- [x] **R5.3** — Heal only on `PostgresError` path matching connectivity heuristic (not `QueryError` / `Other`)

### 5.6 Queue and back-pressure

| Req ID | Requirement | Acceptance hint |
|--------|-------------|-----------------|
| R6.1 | Per-worker or global queue **bounded**; behavior when full defined (prefer: fail fast with pool timeout path) | Load test or deterministic test |
| R6.2 | No unbounded memory growth under spike | Stress test / Miri not required; logical cap demonstrated |

**Implementation — §5.6**

- [x] **R6.1** — Bounded queue(s); full-queue behavior implemented and documented
- [x] **R6.2** — Spike test or reasoning documented (no unbounded `Vec` growth per worker)

### 5.7 Wal lag monitor

| Req ID | Requirement | Acceptance hint |
|--------|-------------|-----------------|
| R7.1 | Initial connect **retries** with backoff until success or explicit “give up” policy that is **documented** | Test: reject-then-accept |
| R7.2 | **Configurable** poll interval and lag threshold (bytes or time-based policy per design) | Config round-trip test |
| R7.3 | If monitor gives up, metric or log **explains** replica routing is permanently primary-only | Observable in logs/metrics |

**Implementation — §5.7**

- [ ] **R7.1** — Initial connect retry + backoff; optional give-up policy documented *— retry implemented; give-up / test TBD*
- [ ] **R7.2** — Poll interval + lag threshold from config; round-trip test
- [ ] **R7.3** — Log/metric when monitor stops retrying (primary-only reads)

### 5.8 Observability

| Req ID | Requirement | Acceptance hint |
|--------|-------------|-----------------|
| R8.1 | Counters/histograms for acquire waits, timeouts, slot heal, connection age eviction (behind `metrics` feature where applicable) | Documented metric names |
| R8.2 | Optional **tracing** spans for acquire and heal | Span presence in tests or examples |

**Implementation — §5.8**

- [ ] **R8.1** — Metric names documented; counters/histograms behind `metrics` where needed
- [ ] **R8.2** — Tracing spans for acquire (and heal if applicable); example or test assertion

---

## 6. Non-functional requirements

| NFR | Requirement |
|-----|-------------|
| NFR1 | **Backward compatibility:** existing `LifeguardPool::new(url, n, replicas, m)` call sites continue to compile unless a deprecation window is explicitly documented. |
| NFR2 | **Determinism in tests:** new time-based behavior must be testable (injected clock, short durations in tests, or feature flags). |
| NFR3 | **Documentation:** update `lib.rs` pool section, and add operator tuning subsection (max lifetime vs Postgres `idle_session_timeout`, etc.). |
| NFR4 | **Performance:** default paths must not add measurable overhead vs today’s dispatch on the hot path (probes on **idle** only, or amortized). |

### 6.1 NFR verification checklist

- [ ] **NFR1** — `cargo check` on `examples/` + in-repo callers; deprecation plan in CHANGELOG if breaking
- [ ] **NFR2** — Time-based tests use short durations, fake time, or feature gates; CI stable
- [ ] **NFR3** — `lib.rs` + book/operator doc updated; cross-link this PRD
- [ ] **NFR4** — Hot-path review or micro-bench note; idle-only probes confirmed in code review

---

## 7. Phased delivery (recommended)

| Phase | Scope | Outcome |
|-------|--------|---------|
| **P0** | R1.x + R6.1 + R2.1 (merge config, wire timeout) | No infinite wait; bounded queues |
| **P1** | R5.x + R4.x | Survives dead connections and idle TCP |
| **P2** | R3.x | Lifetime rotation matches DBA/firewall policy |
| **P3** | R7.x + R8.x | Tunable replica monitor + operability |

Phases may be reprioritized if production incidents dictate (e.g. P0+P1 first).

### 7.1 Phase P0 checklist

- [x] R1.1 — acquire timeout
- [x] R1.2 — distinct error variant
- [x] R1.3 — default + changelog
- [x] R6.1 — bounded queue + full behavior
- [x] R6.2 — no unbounded growth (validated)
- [x] R2.1 — single config / merge duplicates
- [x] R2.2 — env parity (`LIFEGUARD__DATABASE__*`)
- [x] R2.3 — constructor rustdoc

### 7.2 Phase P1 checklist

- [x] R5.1 — error taxonomy (`src/pool/connectivity.rs` rustdoc table)
- [x] R5.2 — slot heal (one reconnect attempt per job; `exec_with_optional_heal` in `pooled.rs`) + unit tests for classifier
- [x] R5.3 — review gate (only `LifeError::PostgresError` + connectivity heuristic; no heal on `QueryError`/`Other`)
- [x] R4.1 — keepalive / connection doc (`docs/POOL_TCP_KEEPALIVE.md`, `connect` rustdoc)
- [x] R4.2 — idle liveness probe + test (`idle_liveness_interval_ms`, `tests/db_integration/pool_idle_liveness.rs`)

### 7.3 Phase P2 checklist

- [ ] R3.1 — max lifetime + jitter
- [ ] R3.2 — idle policy + doc/test

### 7.4 Phase P3 checklist

- [ ] R7.1 — monitor connect retry *— initial retry landed early; tunables / give-up remain*
- [ ] R7.2 — monitor tunables
- [ ] R7.3 — give-up observable
- [ ] R8.1 — metrics
- [ ] R8.2 — tracing spans
- [ ] G9 items tied to P3 release (docs/changelog)

---

## 8. Risks and mitigations

| Risk | Mitigation |
|------|------------|
| `may` scheduler + blocking `recv_timeout` interaction | Design doc: explicit thread/coroutine boundaries; tests under `may` runtime |
| Heal loops flapping on persistent outage | Cap retries; circuit-break to primary-only for replica tier |
| Config migration breaks embedders | Deprecation attributes, changelog, dual support for one release |
| `may_postgres` lacks TCP keepalive knobs | Document libpq params in URL; fallback to OS sysctl |

---

## 9. Open questions (resolve in design doc)

1. **API:** `LifeguardPool::new` vs `LifeguardPool::from_config(PoolConfig)` — deprecation strategy?
2. **Queue policy:** when full, reject vs block-with-timeout only?
3. **Replica heal:** same connectivity policy as primary workers or stricter (prefer primary-only)?
4. **Minimum idle:** do we introduce **dynamic** pool size, or keep fixed workers and only **replace** clients inside slots?

---

## 10. Success criteria (PRD closure)

- [ ] **§0.1** — Phases P0–P3 checked off, or deferred items listed below with issue URLs
- [ ] **§12** — All requirement IDs R1.x–R8.x checked or explicitly **Deferred** with rationale + issue
- [ ] **§3.1** — All goals G1–G9 checked
- [ ] **§6.1** — All NFR1–NFR4 checked
- [ ] No duplicate unused `pool_timeout_seconds` / `max_connections` in public config
- [ ] Automated tests: **timeout**, **queue bound**, **≥1 heal path** (integration or unit with injectable failure)
- [ ] Public docs: **non-goals** (PgBouncer), **tuning** (pool lifetime vs Postgres `idle_session_timeout` / firewall)

### 10.1 Explicit deferrals (optional)

Use when scope is intentionally postponed; do not leave boxes ambiguous.

| Item deferred | Issue | Target |
|---------------|-------|--------|
| (none yet) | | |

---

## 11. Related documents

- [PRD_READ_REPLICA_TESTING.md](./PRD_READ_REPLICA_TESTING.md) — replica **testing** and CI (orthogonal to pool robustness).
- Optional future: `DESIGN_CONNECTION_POOLING.md` — state machines, error taxonomy, metric names.

---

## 12. Master implementation checklist (all requirement IDs)

Single list for copy-paste into issues or sprint boards. Sub-bullets are optional split tasks.

### Acquisition & overload
- [x] R1.1
- [x] R1.2
- [x] R1.3

### Configuration
- [x] R2.1
- [x] R2.2
- [x] R2.3

### Connection lifetime
- [ ] R3.1
- [ ] R3.2

### Liveness & TCP
- [x] R4.1
- [x] R4.2

### Slot heal
- [x] R5.1
- [x] R5.2
- [x] R5.3

### Queues
- [x] R6.1
- [x] R6.2

### Wal lag monitor
- [ ] R7.1 *partial*
- [ ] R7.2
- [ ] R7.3

### Observability
- [ ] R8.1
- [ ] R8.2

### Non-functional (§6)
- [ ] NFR1
- [ ] NFR2
- [ ] NFR3
- [ ] NFR4
