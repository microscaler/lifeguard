# ORM performance harness (`examples/perf-idam`)

This repository includes an **IDAM-shaped** example crate and a `perf-orm` binary that measures end-to-end Lifeguard ORM operations against PostgreSQL (single connection). It is intended for local tuning and CI artifacts, not as a strict latency gate on shared runners.

## Layout choice

`examples/perf-idam` is a **standalone workspace** (`[workspace]` with no members), same pattern as `examples/entities`. That keeps an optional `Cargo.lock` in the example tree and avoids pulling perf-only dependency resolution into the root workspace’s default `cargo check` path.

## Tables

- `perf_tenants` — small tenant cardinality  
- `perf_users` — UUID PK, composite unique `(tenant_id, email)`  
- `perf_sessions` — UUID PK, unique `token_fingerprint`, `last_seen_at` for update scenarios  

Schema source: [`examples/perf-idam/migrations/schema.sql`](../examples/perf-idam/migrations/schema.sql).

## Running locally

From the repo root:

```bash
cd examples/perf-idam
export PERF_DATABASE_URL="postgres://USER:PASS@HOST:5432/DB"
# Optional:
export PERF_TENANT_COUNT=10      # default 10
export PERF_USER_ROWS=5000       # default 5000
export PERF_SESSION_ROWS=5000    # default 5000
export PERF_WARMUP=200           # default 200
export PERF_ITERATIONS=2000      # default 2000
export PERF_OUTPUT=/tmp/perf-results.json   # default: print JSON to stdout

cargo run --release --bin perf-orm
```

`PERF_DATABASE_URL` overrides `DATABASE_URL` and `TEST_DATABASE_URL` if set.

The JSON report includes **`connections": 1`**. When Lifeguard ships a real connection pool (Epic 04), extend the harness with `connections=N` and a concurrent scenario; compare runs only at the same `connections` value.

## CI

The **`perf_orm`** job in [`.github/workflows/ci.yaml`](../.github/workflows/ci.yaml) runs **after** the main `test` job succeeds on `push` to `main` and on `pull_request`. It uploads `perf-results.json` as an artifact. Configure the repository secret **`PGPASSWORD`**: the Postgres service and `PERF_DATABASE_URL` both use it (same as the `test` job). GitHub-hosted runners are noisy; use artifacts for **trends** or compare to a baseline from `main`, not hard millisecond limits on PRs.

## Baseline comparison (optional)

To detect large regressions later:

1. Store the artifact from a known-good `main` run (e.g. `baseline-perf-results.json`).
2. In a follow-up workflow job, download that artifact and compare `scenarios[].p95_us` per scenario with a generous threshold (for example 30–50% after accounting for runner variance).

This is not implemented in-repo yet to avoid false failures on shared hardware.
