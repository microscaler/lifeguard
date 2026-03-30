# Test Infrastructure - Kind/Kubernetes Setup

This document describes the test infrastructure setup for Lifeguard using Kind (Kubernetes in Docker).

## Overview

Lifeguard uses Kind (Kubernetes in Docker) for local development. The manifests under `config/k8s/test-infrastructure/` mirror **CI’s** [`.github/docker/docker-compose.yml`](../.github/docker/docker-compose.yml): **Bitnami legacy PostgreSQL 15** primary, **two streaming replicas**, and **Redis 7** (same images/env shape as Compose).

This provides:
- Isolated test environments
- Kubernetes-native service discovery
- **Host ports aligned with CI** when using Tilt (see table below)
- **Kind Postgres primary** also provisions a **`pact`** login role (password `pact`, `pg_monitor` granted) via `postgres-pact-role.sql` so borrowed observability snippets do not fail with `FATAL: role "pact" does not exist`.

### Tilt port-forwards (localhost → Kind)

| Host port | Service | Matches CI compose |
|-----------|---------|-------------------|
| **6543** | `postgresql-primary` | Primary Postgres |
| **6544** | `postgresql-replica-0` | First replica (CI maps single replica here) |
| **6545** | `redis` | Redis |
| **6546** | `postgresql-replica-1` | Second replica (extra vs single-replica CI) |

Use `export TEST_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:6543/postgres`, `TEST_REPLICA_URL=…:6544` (or `:6546`), and `TEST_REDIS_URL=redis://127.0.0.1:6545` when testing against the Kind stack from the host. Passwords match `config/k8s/test-infrastructure/postgresql-credentials-secret.yaml` (default `postgres` for local dev). The **`justfile`** defines the same values (`TEST_DATABASE_URL`, `TEST_REPLICA_URL`, `TEST_REDIS_URL`, optional `TEST_REPLICA_URL_SECOND`); run **`just kind-test-env`** to print `export …` lines, or use recipes like **`just nt-db-suite`** / **`just nt-workspace`** which set these automatically.

## Prerequisites

- [Kind](https://kind.sigs.k8s.io/) installed
- [kubectl](https://kubernetes.io/docs/tasks/tools/) installed
- Docker running

### Installation

**macOS:**
```bash
brew install kind kubectl
```

**Linux:**
```bash
# Kind
curl -Lo ./kind https://kind.sigs.k8s.io/dl/v0.20.0/kind-linux-amd64
chmod +x ./kind
sudo mv ./kind /usr/local/bin/kind

# kubectl
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
chmod +x kubectl
sudo mv kubectl /usr/local/bin/kubectl
```

## Quick Start

### Setup Test Infrastructure

```bash
# Setup Kind cluster and deploy PostgreSQL (full setup)
just dev-up

# Or step by step:
just dev-up          # Create cluster, deploy, and wait for database
just dev-wait-db     # Wait for PostgreSQL to be ready (if already deployed)
```

### Get Connection String

```bash
# Get connection string for tests
just dev-connection-string

# Or set environment variable
export TEST_DATABASE_URL=$(just dev-connection-string)
```

### Port-Forward for Local Access

If you need to access PostgreSQL from your local machine:

```bash
just dev-port-forward
```

`just dev-port-forward` forwards **primary** Postgres only (`6543:5432`). Use **`tilt up`** for the full set (6543–6546) as in the table above.

### Teardown

```bash
just dev-down
```

### cargo-nextest

Install a **pinned** binary if `cargo install cargo-nextest --locked` fails with “requires rustc 1.91 or newer” (newer nextest needs a newer compiler than this repo’s pinned nightly or your local toolchain):

```bash
cargo install cargo-nextest --locked --version 0.9.128
```

CI uses the same pin in `.github/workflows/ci.yaml`. When you upgrade the workspace `rust-toolchain` / nightly past 1.91, you can bump the nextest version in CI and the command above.

### ORM performance harness (optional)

The `examples/perf-idam` crate (standalone workspace) provides a `perf-orm` binary that benchmarks IDAM-shaped reads/writes against Postgres. See [PERF_ORM.md](./PERF_ORM.md) for environment variables and interpretation. In [`.github/workflows/ci.yaml`](../.github/workflows/ci.yaml), the **`perf_orm`** job runs after **`test`** and uploads JSON artifacts; it uses the same **`PGPASSWORD`** repository secret for Postgres, sets **`PERF_DATABASE_URL`**, and **`PERF_RESET=1`** (required opt-in for destructive `perf_*` DDL).

### GitHub Actions: entity migrations on Postgres

In `.github/workflows/ci.yaml`, **before** workspace tests and `db_integration_suite`, the **`test`** job:

1. Starts **primary + replica Postgres and Redis** via [`.github/docker/docker-compose.yml`](../.github/docker/docker-compose.yml) (Bitnami legacy images). **Host** ports are **`6543`** (primary), **`6544`** (replica), **`6545`** (Redis). Local **Kind/Tilt** uses the **same** host port scheme (**6543–6546**) for primary, two replicas, and Redis — see table above — so you can reuse the same `TEST_*` URLs when switching between CI Compose and Kind on one machine (do not run both stacks at once on overlapping ports).
2. Deletes `migrations/generated/` (so CI does not rely on committed SQL artifacts).
3. Runs `cargo run --bin generate-migrations` from `examples/entities` (standalone crate; regenerates SQL from inventory entities).
4. Applies SQL to the **primary** only (`psql -h 127.0.0.1 -p 6543`): paths come from `migrations/generated/apply_order.txt` when present (FK-safe order from `write_apply_order_file`), otherwise `find … | sort` over `*.sql`.

That validates the migration-generation path against a real database before other steps use the same Postgres instance.

Configure the **`PGPASSWORD`** [repository secret](https://docs.github.com/en/actions/security-guides/using-secrets-in-github-actions) so it matches the password embedded in `DATABASE_URL` / `TEST_DATABASE_URL` / `TEST_REPLICA_URL`, in [`.github/docker/docker-compose.yml`](../.github/docker/docker-compose.yml) (Compose interpolates **`${PGPASSWORD}`** for Bitnami `POSTGRESQL_*` and replication), and in `psql` (via `PGPASSWORD`). Avoid raw `@`, `:`, `/`, `#`, or `%` in that password unless you percent-encode them for the URL (the `psql` client still uses the raw secret via `PGPASSWORD`).

### Rust crate integration tests (`db_integration_suite`)

The `lifeguard` package runs database-backed tests from a **single** integration binary (`tests/db_integration_suite.rs`) that shares one Postgres URL (and a Redis URL in context) per process.

| Variable | Role |
|----------|------|
| `TEST_DATABASE_URL` | If set, **skips** starting Postgres via testcontainers; must point at a **dedicated test** Postgres (not `DATABASE_URL` — integration code can run destructive DDL). |
| `TEST_REPLICA_URL` | Streaming standby URL for the same cluster as `TEST_DATABASE_URL`; enables `pool_read_replica` tests. **Unset** → those tests no-op (skip). CI sets this to `localhost:6544`. See [PRD_READ_REPLICA_TESTING.md](planning/PRD_READ_REPLICA_TESTING.md). |
| `LIFEGUARD_POOL_TEST_TIMING` | Optional; if non-empty and not `0`/`false`, `pool_read_replica` prints phase timings (setup, pool open, replay wait, reads, batch load) to **stderr**. |
| `TEST_REDIS_URL` or `REDIS_URL` | Optional; defaults to `redis://127.0.0.1:6379` when Postgres comes from env. For **CI Compose** or **Kind/Tilt** on the host ports above, set **`TEST_REDIS_URL=redis://127.0.0.1:6545`**. |

**CI Compose vs Kind/Tilt:** Both use **6543 / 6544 / 6545** on the host for primary, first replica, and Redis. Kind adds a **second** replica on **6546**. Do not run Compose and Kind/Tilt port-forwards on the same host ports at the same time.

**Shared Postgres (e.g. Kind + `just dev-up`):** the `db_integration_suite` binary uses **fixed table names** (`test_users`, hook tables, etc.) on a single `TEST_DATABASE_URL`. Running many of its tests **in parallel** (nextest’s default `test-threads = num-cpus`) starts **one process per test**, all hitting the same tables — that produces **row-count flakes** (e.g. `active_model_crud` expecting 2 rows and seeing 5) and can also stress connections or DDL.

**Mitigation (in-repo):** `.config/nextest.toml` defines a nextest **[test group](https://nexte.st/docs/configuration/test-groups/)** `lifeguard-shared-postgres` with `max-threads = 1` applied to `binary(db_integration_suite)`. Only **one** test from that binary runs at a time, while other workspace packages can still run in parallel. CI keeps a **separate** `db-serial` step for clarity and timeouts; either approach is valid.

Prefer **`db-serial`** when running **only** that binary (still one global test thread for the whole run):

```bash
export TEST_DATABASE_URL="$(just dev-connection-string)"
cargo nextest run -p lifeguard --profile db-serial -E 'binary(db_integration_suite)'
```

Or, with plain Cargo:

```bash
cargo test -p lifeguard --test db_integration_suite -- --test-threads=1
```

Targeted modules (faster feedback):

```bash
cargo test -p lifeguard --test db_integration_suite related_trait:: dataloader_n_plus_one:: -- --test-threads=1
```

**`just nt`:** runs nextest on the workspace but **skips** the `db_integration_suite` binary so the default dev loop stays fast (DB integration is slower). The skip is **not** required for correctness anymore — the test-group mutex fixes parallel races if you include the binary. For a full run: `just nt-complete` or `just nt-ci-parity`, or:

```bash
just nt-db-suite
# alias: just nt-db
```

See also [`docs/planning/audits/LIFEGUARD_FOUNDATION_CONTINUATION.md`](planning/audits/LIFEGUARD_FOUNDATION_CONTINUATION.md) (Phase A / C).

### Read-replica testing (local)

CI’s **`test`** job starts primary + replica Compose and sets `TEST_REPLICA_URL` so [`pool_read_replica`](../tests/db_integration/pool_read_replica.rs) runs against a real standby.

**Local runbook** (requires Docker; default passwords below match Compose defaults when env is unset):

```bash
cd /path/to/lifeguard
# Set PGPASSWORD in your shell to match your DB (CI uses the repository secret of the same name; do not commit values).
docker compose -f .github/docker/docker-compose.yml up -d --wait

export TEST_DATABASE_URL="postgres://postgres:${PGPASSWORD}@127.0.0.1:6543/postgres"
export TEST_REPLICA_URL="postgres://postgres:${PGPASSWORD}@127.0.0.1:6544/postgres"
export TEST_REDIS_URL="${TEST_REDIS_URL:-redis://127.0.0.1:6545}"

cargo nextest run -p lifeguard --all-features --profile db-serial -E 'binary(db_integration_suite)' pool_read_replica:: --no-fail-fast

docker compose -f .github/docker/docker-compose.yml down -v
```

If **6543 / 6544 / 6545** are already in use, stop the conflicting service or override published ports in a local override file. **5432 / 5433 / 6379** are left free for Kind/Tilt by design.

Product requirements: [`docs/planning/PRD_READ_REPLICA_TESTING.md`](planning/PRD_READ_REPLICA_TESTING.md). Engineering design: [`docs/planning/DESIGN_READ_REPLICA_CI_AND_HARNESS.md`](planning/DESIGN_READ_REPLICA_CI_AND_HARNESS.md).

## Architecture

### Cluster Configuration

- **Cluster Name:** `lifeguard-test`
- **Namespace:** `lifeguard-test`
- **Pod Subnet:** `10.206.0.0/16` (unique to avoid conflicts)
- **Service Subnet:** `10.207.0.0/16` (unique to avoid conflicts)

### Services

- **PostgreSQL:** `postgres.lifeguard-test.svc.cluster.local:5432`
  - Image: `postgres:15-alpine`
  - Database: `postgres`
  - User: `postgres`
  - Password: `postgres`
  - Persistent storage: 1Gi PVC

## Using in Tests

### Environment Variables

**`db_integration_suite` (`tests/context.rs`):** only **`TEST_DATABASE_URL`**. Do not rely on `DATABASE_URL`; helpers may issue destructive SQL.

**`TestDatabase::get_connection_string` (library / unit tests):**

1. **`TEST_DATABASE_URL`** (highest priority)
2. **`DATABASE_URL`** (fallback)
3. **Kubernetes service discovery** (if running in cluster)
4. **Default localhost** (fallback)

### Example Integration Test

```rust
use lifeguard::test_helpers::TestDatabase;
use lifeguard::{LifeExecutor, LifeError};

#[test]
fn test_connection() -> Result<(), LifeError> {
    let mut db = TestDatabase::new()?;
    let executor = db.executor()?;
    
    // Execute a test query
    let row = executor.query_one("SELECT 1 as test", &[])?;
    let result: i32 = row.get(0);
    assert_eq!(result, 1);
    
    Ok(())
}
```

### Waiting for Database

```rust
use lifeguard::test_helpers::TestDatabase;

#[test]
fn test_with_wait() -> Result<(), lifeguard::test_helpers::TestError> {
    let mut db = TestDatabase::new()?;
    
    // Wait up to 30 seconds for database to be ready
    db.wait_for_ready(10, 3)?;
    
    let executor = db.executor()?;
    // ... use executor
    Ok(())
}
```

## Manual kubectl Commands

### Check Cluster Status

```bash
# List clusters
kind get clusters

# Check cluster nodes
kubectl get nodes

# Check Postgres / Redis deployments
kubectl get deployment -n lifeguard-test

# Check pods
kubectl get pods -n lifeguard-test

# Services (primary, replicas, redis)
kubectl get svc -n lifeguard-test
```

### View Logs

```bash
# Postgres primary logs
kubectl logs -n lifeguard-test deployment/postgresql-primary -f
```

### Access PostgreSQL Shell

```bash
# Exec into PostgreSQL pod
kubectl exec -it -n lifeguard-test deployment/postgresql-primary -- /bin/sh -c 'export PGPASSWORD=postgres; /opt/bitnami/postgresql/bin/psql -U postgres -h 127.0.0.1 -d postgres'
```

## Troubleshooting

### Cluster Creation Fails

- Ensure Docker is running
- Check if port conflicts exist
- Try deleting existing cluster: `kind delete cluster --name lifeguard-test`

### PostgreSQL Not Ready

```bash
# Check pod status
kubectl describe pod -n lifeguard-test -l app=postgresql-primary

# Check events
kubectl get events -n lifeguard-test --sort-by='.lastTimestamp'
```

### Connection String Issues

- Verify services: `kubectl get svc -n lifeguard-test`
- Primary DNS: `postgresql-primary.lifeguard-test.svc.cluster.local`
- Use port-forward for local access: `just dev-port-forward`

## Commands

```bash
just dev-up                # Setup Kind cluster (full setup)
just dev-down              # Teardown Kind cluster
just dev-wait-db            # Wait for PostgreSQL
just dev-connection-string  # Get connection string
just dev-port-forward       # Port-forward for local access
```

## CI/CD Integration

For CI/CD pipelines, ensure:

1. Kind is installed
2. Docker is available
3. Run `just dev-up` before tests
4. Set `TEST_DATABASE_URL` environment variable
5. Run `just dev-down` after tests

Example GitHub Actions:

```yaml
- name: Setup Kind cluster
  run: just dev-up

- name: Run tests
  env:
    TEST_DATABASE_URL: $(just dev-connection-string)
  run: cargo test --all

- name: Teardown
  if: always()
  run: just dev-down
```
