# Design: Read-replica CI topology and test harness

**Status:** **Partially implemented** — Compose at [`.github/docker/docker-compose.yml`](../../.github/docker/docker-compose.yml) (Postgres primary + replica + Redis; CI `test` / `perf_orm` use this only, no GitHub `services:` for DB/Redis). `tests/context.rs` `replica_pg_url`, `tests/db_integration/replication_sync.rs`, `pool_read_replica`, `TEST_INFRASTRUCTURE.md` runbook. R3.3 lag fallback remains `#[ignore]` (fault injection follow-up). Images: `bitnamilegacy/postgresql:15` (public `bitnami/postgresql:15` tag unavailable on Docker Hub).  
**Audience:** Lifeguard maintainers implementing CI and tests.  
**References:** [PRD_READ_REPLICA_TESTING.md](./PRD_READ_REPLICA_TESTING.md); [TEST_INFRASTRUCTURE.md](../TEST_INFRASTRUCTURE.md); `.github/workflows/ci.yaml`; `tests/context.rs`; `tests/db_integration_suite.rs`; `src/pool/pooled.rs`; `src/pool/wal.rs`.

---

## 1. Context

### 1.1 Singleton behavior today

The database integration tests run as **one** Cargo integration binary (`tests/db_integration_suite.rs`). `tests/context.rs` uses a process-wide `LazyLock` (`TEST_CONTEXT`) so **all modules in that binary share one** Postgres URL and one Redis URL for the process lifetime. This is already the right granularity for adding a **shared replica URL**: extend the context once per process, not per `#[test]`.

When `TEST_DATABASE_URL` is **unset**, testcontainers starts **one** disposable Postgres per **process** (the first access to `get_test_context()`). That instance is **not** a replica and cannot stand in for standby testing.

### 1.2 Why not two `services:` Postgres containers only?

GitHub Actions `services` containers start **independently** with no built-in primary→standby bootstrap. Streaming replication requires ordered startup, shared replication user/password, `pg_hba` / `postgresql.conf` adjustments, base backup or replication slot setup, and often a **custom command** or entrypoint. Coupling two `postgres:15` service definitions does not, by itself, produce a valid standby.

**Recommendation:** use a **Docker Compose** file (or a single custom image) invoked from a workflow **step**, not two naive `services:` entries.

---

## 2. Recommended CI topology: Docker Compose

### 2.1 Choice

**Primary recommendation:** a **Docker Compose** stack under **`.github/docker/`** (kept next to workflows; Postgres primary + replica + Redis in one file) using images and environment variables that **document replication in one place**.

**Preferred image family for v1:** **Bitnami PostgreSQL** (or equivalent) with explicit `REPLICATION_MODE` / master-slave env pairs, because:

- Replication user and `postgresql.conf` / `pg_hba` are pre-wired for common master→slave flows.
- Startup order can be expressed with `depends_on` and healthchecks.
- Same file can be used **locally** and in GHA.

**Alternative:** Official `postgres` image with custom `command:` and mounted `pg_hba.conf` / init scripts. Valid but higher maintenance; document if chosen over Bitnami.

### 2.2 Port mapping (runner-visible)

Expose Postgres and Redis on **high host ports** so **5432 / 5433 / 6379** stay available for Kind/Tilt on the same workstation:

| Role | Host port | Container |
|------|-----------|-----------|
| Primary | `6543` | `postgresql-primary` → `5432` |
| Replica | `6544` | `postgresql-replica` → `5432` |
| Redis | `6545` | `redis` → `6379` |

CI steps set:

- `TEST_DATABASE_URL=postgres://…@127.0.0.1:6543/postgres`
- `TEST_REPLICA_URL=postgres://…@127.0.0.1:6544/postgres`
- `REDIS_URL` / `TEST_REDIS_URL` → `redis://127.0.0.1:6545` when using this Compose file

Use the **same password** as today’s `PGPASSWORD` secret where possible so URLs stay consistent with `psql` migration steps (which must target the **primary** only).

### 2.3 Lifecycle in `ci.yaml`

1. **Checkout** and toolchain (unchanged).
2. **`docker compose up -d`** (or `docker compose up --wait` if supported) for the replica stack.
3. **Wait** until both containers report healthy (Compose healthchecks + optional explicit `pg_isready` loop).
4. **Apply migrations** with `psql` against **127.0.0.1:6543** only (primary). Replica applies WAL.
5. **Replication sync gate** (see §4) once before running replica-dependent tests, or from test harness init.
6. Run **nextest** / `cargo test` with `TEST_DATABASE_URL` and `TEST_REPLICA_URL` set.
7. **`docker compose down -v`** in `if: always()` or final step to free disk and avoid cross-job pollution.

**Ordering note:** Regenerate-and-apply migrations (current CI step) remains on the **primary**. Do not run destructive DDL against the replica connection.

### 2.4 No GitHub `services:` for Postgres or Redis

**Decision (implemented):** The **`test`** and **`perf_orm`** jobs do **not** use `jobs.*.services` for Postgres or Redis. A single [`.github/docker/docker-compose.yml`](../../.github/docker/docker-compose.yml) provides **primary + replica + Redis**, avoiding duplicate containers and port clashes (e.g. two primaries on 5432). Replication still cannot be expressed with naive `services:` alone; mixing `services.postgres` with Compose for replica was redundant and error-prone.

---

## 3. Local developer parity

- Check in the **same** `docker-compose.yml` used in CI.
- Document in [TEST_INFRASTRUCTURE.md](../TEST_INFRASTRUCTURE.md):

  ```bash
  docker compose -f .github/docker/docker-compose.yml up -d --wait
  export TEST_DATABASE_URL=...
  export TEST_REPLICA_URL=...
  export TEST_REDIS_URL=redis://127.0.0.1:6545
  cargo nextest run -p lifeguard --profile db-serial -E 'binary(db_integration_suite)' pool_read_replica::
  ```

- Optional: a `just` recipe `dev-replica-up` / `dev-replica-down` wrapping compose (follows repo conventions if added later).

---

## 4. Replication sync gate

### 4.1 Purpose

After a write on the primary, physical replication may lag. Tests that assert “read path sees the row” on the **replica connection** must **wait** until the standby has replayed sufficiently.

### 4.2 Suggested predicate (PostgreSQL 15)

Implement a small helper (Rust or shell for CI-only smoke) that loops until **any** of the following holds (pick one primary strategy; document the chosen one in code comments):

**Strategy 1 — Primary-side (preferred for “catch up after write”):**

- Query primary: `SELECT pg_current_wal_lsn() AS lsn;` (or `pg_wal_lsn_diff` against a known insert).
- Query replica: `SELECT pg_last_wal_replay_lsn() AS lsn;`
- Wait until `replay_lsn >= write_lsn` observed after the write (capture `pg_current_wal_lsn()` **after** commit on primary, then poll replica until `pg_last_wal_replay_lsn() >= target`).

**Strategy 2 — Replica-side lag bytes:**

- On replica: `SELECT pg_is_in_recovery();` must be true.
- Poll `pg_wal_lsn_diff(pg_last_wal_receive_lsn(), pg_last_wal_replay_lsn())` until below a small threshold (aligns with `WalLagMonitor` semantics in `wal.rs`).

**Strategy 3 — `pg_stat_replication` on primary:**

- After write, poll primary for `application_name` / `state = 'streaming'` and `replay_lag` below threshold.

### 4.3 Parameters

| Parameter | Suggested default | Notes |
|-----------|-------------------|-------|
| Timeout | 30–60 s | CI runners vary; tune after first flakes |
| Poll interval | 100–250 ms | Balance CPU vs latency |
| Failure mode | `panic!` in test setup or `Err` from helper | Prefer `Result` with formatted context for nextest output |

### 4.4 Where to run the gate

| Location | Pros | Cons |
|----------|------|------|
| **`LazyLock` init in `tests/context.rs`** | Fails entire binary fast if replica never syncs | Slow or flaky if run on every init; may be overkill before tests that skip replica |
| **`#[ctor]` / `Once` in a dedicated replica test module** | Only pay cost when replica tests run | Requires careful ordering |
| **Per-test `setup` helper** | Precise per scenario | Verbose; duplicate waits |

**Recommendation:** **Once per module** or **first test in `pool_read_replica` module** via `std::sync::Once` that runs “replica is in recovery + caught up to baseline” after compose health, and **per test** call a lightweight `wait_until_replica_has_lsn(primary, replica, target_lsn)` after writes.

---

## 5. Harness changes (`tests/context.rs`)

### 5.1 `LifeguardTestContext` extension

```text
pub struct LifeguardTestContext {
    pub pg_url: String,           // primary (existing)
    pub redis_url: String,        // existing
    pub replica_pg_url: Option<String>,  // new: Some when TEST_REPLICA_URL set
}
```

- Read `TEST_REPLICA_URL` via same trimming rules as other env vars (non-empty string).
- When using **testcontainers** (no `TEST_DATABASE_URL`): **do not** attempt to start a replica in-process for v1; `replica_pg_url` stays `None` unless we later add a compose-backed dev script. Document that **replica tests require env-provided URLs** or Compose started externally.

### 5.2 CI path

When `TEST_DATABASE_URL` and `TEST_REPLICA_URL` are both set (CI with Compose), context initialization **does not** start testcontainers for Postgres. Redis behavior unchanged (env or container).

---

## 6. Test layering

### 6.1 Unit tests (`wal.rs` / `pooled.rs`)

| Approach | Description |
|----------|-------------|
| **A. Integration-first (v1)** | No refactor; all replica routing tests live in `db_integration_suite` with real Postgres. Fastest to ship. |
| **B. Inject lag state** | Introduce a trait or test-only hook for “is lagging” to avoid flakiness and test `read_tier` branches without real lag. Cleaner long-term. |

**Recommendation:** **v1 = A**; file a follow-up issue for **B** if `WalLagMonitor` stability or CI time becomes painful.

### 6.2 Integration module

Add a new module, e.g. `tests/db_integration/pool_read_replica.rs`, included from `db_integration_suite.rs`:

- Uses `get_test_context()`; **early-return skip** if `replica_pg_url.is_none()` (unless CI job guarantees presence—then `assert!` in CI-only cfg is optional).
- Builds `Arc<LifeguardPool::new(primary, n, vec![replica_url], m>)` with small `n`, `m`.
- Uses `PooledLifeExecutor` for `execute_values` + `query_one_values` / `query_all_values`.
- Avoids **concurrent DDL** with other modules: use a **dedicated schema** (`CREATE SCHEMA IF NOT EXISTS pool_replica_test`) and tables inside it, or run replica tests in a **separate nextest filter** after other modules (still `RUST_TEST_THREADS=1` for the binary).

### 6.3 Asserting “read used replica”

Options:

- **Weak assertion:** After sync gate, read returns correct data (true for primary or replica).
- **Strong assertion:** Enable `log_statement` / `application_name` discrimination (fragile), or use **separate table only on replica** (invalid). Practical strong check: run `SELECT pg_is_in_recovery()` **on the connection used by read path** if the executor exposes a hook—**not** available today without test-only introspection.

**Recommendation for v1:** **Weak assertion** plus optional **direct** `may_postgres::connect(replica_url)` query to prove standby visibility after sync; separately assert `!pool.is_replica_lagging()` when healthy (subject to monitor warmup time—may need short sleep or poll).

---

## 7. `WalLagMonitor` and test stability

`WalLagMonitor::start_monitor` spawns a coroutine with a **500 ms** poll and **1 MB** byte lag threshold (`wal.rs`). Integration tests should:

- Allow **≥500 ms** (or several ticks) after pool creation before asserting `is_replica_lagging()` is false, **or** poll with timeout.
- Recognize that on a quiet replica, `receive_lsn` / `replay_lsn` may be null in edge cases—design tests to handle “monitor conservative” behavior (falls back to primary).

---

## 8. Workflow touchpoints summary

| File / area | Status |
|-------------|--------|
| `.github/workflows/ci.yaml` | **`test`** / **`perf_orm`**: `.github/docker/docker-compose.yml` up/`--wait` + `down -v`; no `services:` for Postgres/Redis |
| `.github/docker/docker-compose.yml` | Primary + replica + Redis |
| `tests/context.rs` | `replica_pg_url`; skip replica tests when unset |
| `tests/db_integration_suite.rs` | `replication_sync` + `pool_read_replica` |
| Optional `src/pool/wal.rs` | Test hooks or unit tests in follow-up |

**`perf_orm` job:** Uses the same Compose file; optional replica URLs in harness (see PRD NG5 for scope).

---

## 9. Security and credentials

- Reuse **`PGPASSWORD`** in connection strings for both primary and replica where Bitnami/replication user matches.
- Replication often uses a dedicated user (`replicator`); document password source (same secret vs derived) in compose comments.
- Do not log full URLs with passwords in test output (redact as `perf_orm` does for hosts).

---

## 10. Open decisions (explicit)

| Topic | Decision owner | Notes |
|-------|----------------|-------|
| Bitnami vs official `postgres` image | Implementer | Bitnami legacy image for v1 |
| Strong vs weak “replica served read” proof | Implementer | §6.3 |

---

## 11. Revision history

| Date | Change |
|------|--------|
| 2026-03-28 | Initial design doc (pre-code). |
