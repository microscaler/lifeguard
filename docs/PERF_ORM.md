# ORM performance harness (`examples/perf-idam`)

This repository includes an **IDAM-shaped** example crate and a `perf-orm` binary that measures end-to-end Lifeguard ORM operations against PostgreSQL via [`LifeguardPool`](../../src/pool/pooled.rs) (primary tier; optional read replica when `PERF_REPLICA_URL` / `TEST_REPLICA_URL` is set). It is intended for local tuning and CI artifacts, not as a strict latency gate on shared runners.

## Layout choice

`examples/perf-idam` is a **standalone workspace** (`[workspace]` with no members), same pattern as `examples/entities`. That keeps an optional `Cargo.lock` in the example tree and avoids pulling perf-only dependency resolution into the root workspace’s default `cargo check` path.

## Tables

- `perf_tenants` — small tenant cardinality  
- `perf_users` — UUID PK, composite unique `(tenant_id, email)`  
- `perf_sessions` — UUID PK, unique `token_fingerprint`, per-row distinct `expires_at` (for `NaiveDateTime` predicate benchmarks), `last_seen_at` for update scenarios  

Schema source: [`examples/perf-idam/migrations/schema.sql`](../examples/perf-idam/migrations/schema.sql).

`perf-orm` applies it by stripping whole-line `--` comments and splitting on `;` **outside** single-quoted literals (Postgres `''` escapes are handled). Dollar-quoted (`$$`) or procedural SQL is not supported—keep this file as plain DDL.

## Running locally

From the repo root:

```bash
cd examples/perf-idam
export PERF_DATABASE_URL="postgres://USER:PASS@HOST:5432/DB"
export PERF_RESET=1   # required: confirms disposable DB (perf-orm DROPs/recreates perf_* tables)
# Optional:
export PERF_TENANT_COUNT=10      # default 10
export PERF_USER_ROWS=5000       # default 5000
export PERF_SESSION_ROWS=5000    # default 5000
export PERF_WARMUP=200           # default 200
export PERF_ITERATIONS=2000      # default 2000
export PERF_OUTPUT=/tmp/perf-results.json   # default: print JSON to stdout
# Optional read replica (same cluster as primary; CI uses .github/docker/docker-compose.yml):
# export PERF_REPLICA_URL="postgres://USER:PASS@HOST:6544/postgres"
# export PERF_REPLICA_POOL_SIZE=8   # default: same as PERF_POOL_SIZE

cargo run --release --bin perf-orm
```

Connection URL: **`PERF_DATABASE_URL`**, else **`TEST_DATABASE_URL`**. Generic **`DATABASE_URL` is ignored** so a shell-level app database is never targeted. **`PERF_RESET`** must be truthy (`1`, `true`, `yes`, `on`) before the harness runs destructive DDL.

Replica URL: **`PERF_REPLICA_URL`**, else **`TEST_REPLICA_URL`** (optional). When set, the pool uses **`PERF_REPLICA_POOL_SIZE`** replica-tier slots (default: same as **`PERF_POOL_SIZE`**, minimum 1).

The JSON report includes **`connections`** (primary pool width, from **`PERF_POOL_SIZE`**) and **`replica_connections`** (0 when no replica URL). Compare runs at the same pool sizes when trending latency.

## CI

The **`perf_orm`** job in [`.github/workflows/ci.yaml`](../.github/workflows/ci.yaml) runs **after** the main `test` job succeeds on `push` to `main` and on `pull_request`. It uploads `perf-results.json` as an artifact. The job starts the same [**Docker Compose**](../.github/docker/docker-compose.yml) stack as **`test`** (host **:6543** primary, **:6544** replica, **:6545** Redis — no GitHub `services:`). It sets **`PERF_DATABASE_URL`**, **`PERF_REPLICA_URL`**, **`PERF_RESET=1`**, **`REDIS_URL` / `TEST_REDIS_URL`**, and the repository secret **`PGPASSWORD`** for Compose and URLs. A final step tears down the stack (`down -v`, `if: always()`). GitHub-hosted runners are noisy; use artifacts for **trends** or compare to a baseline from `main`, not hard millisecond limits on PRs.

## Baseline comparison (optional)

To detect large regressions later:

1. Store the artifact from a known-good `main` run (e.g. `baseline-perf-results.json`).
2. In a follow-up workflow job, download that artifact and compare `scenarios[].p95_us` per scenario with a generous threshold (for example 30–50% after accounting for runner variance).

This is not implemented in-repo yet to avoid false failures on shared hardware.
