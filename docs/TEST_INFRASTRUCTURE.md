# Test Infrastructure - Kind/Kubernetes Setup

This document describes the test infrastructure setup for Lifeguard using Kind (Kubernetes in Docker).

## Overview

**Local Kubernetes:** use the shared Kind cluster from **microscaler/shared-kind-cluster** (kubectl context **`kind-kind`**). That repo’s Tilt applies Postgres (primary + replicas), Redis, observability, and other platform workloads into namespaces such as **`data`** and **`observability`**. See the README in that repo (sibling checkout next to Lifeguard).

**This Lifeguard repo does not deploy those workloads.** Its Tiltfile runs **cargo builds and tests only** (UI often `http://localhost:10350`). Run **shared-kind-cluster** `just dev-up` / `tilt up` first (UI often `:10348`), then Lifeguard `just dev-up` / `tilt up`.

**CI** still uses [`.github/docker/docker-compose.yml`](../.github/docker/docker-compose.yml) (host **6543** primary, Toxiproxy replica **6547**, Redis **6545**). For local `just` against that Compose stack, set **`LIFEGUARD_PG_PORT=6543`** (and matching replica/Redis ports if needed).

**Dedicated schema `lifeguard`:** URLs use `search_path=lifeguard`. On shared Postgres, create once: `CREATE SCHEMA IF NOT EXISTS lifeguard;` (superuser/`postgres`). CI Compose applies `postgres-lifeguard-schema.sql` on first init; optional manifests under `config/k8s/test-infrastructure/` remain for reference or manual `kubectl apply` — they are **not** applied by Lifeguard Tilt.

### Host ports (typical)

| Context | Primary | Replica | Redis | Notes |
|---------|---------|---------|-------|--------|
| **Shared cluster Tilt** (`data/postgres`) | **5432** | **6544** | **6545** | **microscaler/shared-kind-cluster** `Tiltfile` port-forwards: primary **5432**, streaming replica-0 **6544**, replica-1 **6546**, Redis **6545** → pod ports. App repos (e.g. Lifeguard) assume these host ports when `TEST_REPLICA_URL` / `TEST_REDIS_URL` use `127.0.0.1`. If you use a different stack, set URLs manually or run `kubectl port-forward` yourself. |
| **CI / local Compose** | **6543** | **6547** (Toxiproxy) | **6545** | Set `LIFEGUARD_PG_PORT=6543` when using `just` against Compose. |

The **`justfile`** defaults to **`LIFEGUARD_PG_PORT=5432`** for shared-cluster dev. Run **`just kind-test-env`** for `export …` lines.

**Lifeguard Tilt:** targets that need Postgres (`test-nextest`, `test-nextest-fast`, `test-db-suite`, `test-migration`, perf, examples) use **`auto_init=False`** so a plain `tilt up` does not run them automatically (avoids red **Update error** when the DB is down or tests fail). The **`perf`** label (`idam-perf`, `idam-perf-run`, `idam-perf-run-replica`) also uses **`trigger_mode=TRIGGER_MODE_MANUAL`** so dependency updates and file watches do not auto-run them—only the UI trigger button does. Open the Tilt UI and **trigger** those resources after shared infra is ready. **`test-nextest`** matches CI’s workspace nextest (includes `lifeguard-integration-tests`); **`test-nextest-fast`** matches **`just nt`** (excludes that crate). **`test-db-suite`** runs **`db_integration_suite`** (serial profile); pair with **`test-nextest`** for full CI parity or use **`just nt-ci-parity`** from a shell (see **`just kind-test-env`**).

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
# 1) Shared platform (Postgres, Redis, …) — from microscaler/shared-kind-cluster
cd ../shared-kind-cluster && just dev-up   # or: tilt up

# 2) Lifeguard builds/tests Tilt (same Kind context kind-kind)
cd ../lifeguard && just dev-up

# Wait for shared data plane pods (namespace data)
just dev-wait-db
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

### Kind Postgres & Redis validation (fresh `tilt up` / `just dev-up`)

After the namespace is ready, use this to confirm **primary + two replicas + Redis** before relying on integration tests or PRD work.

**1. Kubernetes**

```bash
kubectl config current-context   # expect kind-lifeguard-test
kubectl get pods -n lifeguard-test
kubectl wait --for=condition=available --timeout=300s \
  deployment/postgresql-primary deployment/postgresql-replica-0 deployment/postgresql-replica-1 deployment/redis \
  -n lifeguard-test
```

**2. Primary, standbys, Redis (host ports; default password `postgres`)**

```bash
export PGPASSWORD=postgres
psql -h 127.0.0.1 -p 6543 -U postgres -d postgres -c "SELECT version(), current_setting('max_connections')::int;"
psql -h 127.0.0.1 -p 6544 -U postgres -d postgres -c "SELECT pg_is_in_recovery();"
psql -h 127.0.0.1 -p 6546 -U postgres -d postgres -c "SELECT pg_is_in_recovery();"
redis-cli -p 6545 ping
```

**3. Streaming replication (two async standbys)**

```bash
psql -h 127.0.0.1 -p 6543 -U postgres -d postgres -c \
  "SELECT application_name, client_addr::text, state, sync_state,
          sent_lsn::text, replay_lsn::text
   FROM pg_stat_replication ORDER BY application_name;"
```

Expect **two** rows with `state = streaming` and `sent_lsn` / `replay_lsn` matching `pg_current_wal_lsn()` on the primary after idle.

**4. Automated tests (same URLs as `just kind-test-env`)**

```bash
just nt-db-suite
# Full CI-parity: workspace (excludes db_integration_suite) + db suite:
just nt-ci-parity
```

With `DATABASE_URL` / `TEST_*` exported as in the **`justfile`**, `db_integration_suite` should report **65** tests passed (nextest serial profile). Workspace nextest should pass **~800+** tests (a small number may be skipped by design).

**Coverage gaps & edge cases (intentional)**

| Area | Kind/Tilt today | Notes |
|------|-----------------|-------|
| `pooled_read_falls_back_to_primary_when_replica_lagging` | **No-op** unless `TOXIPROXY_API` is set | That path is **Toxiproxy**-driven; CI Compose sets `TOXIPROXY_API` + replica on **6547**. On Kind, `TOXIPROXY_API` is usually unset → test returns immediately. |
| Second replica (**6546**) | Not used by default `TEST_REPLICA_URL` | Pool tests use replica-0 (**6544**). To smoke-test replica-1, run `pool_read_replica` with `TEST_REPLICA_URL=…:6546`. |
| `pact` role | Provisioned on Kind primary | See overview; observability snippets expect this role. |
| Compose vs Kind | Different replica ports | **6547** + Toxiproxy is **CI-only**; do not mix Compose and Tilt on the same host ports. |

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

1. Starts **primary + replica Postgres, Toxiproxy, and Redis** via [`.github/docker/docker-compose.yml`](../.github/docker/docker-compose.yml) (Bitnami legacy images). **Host** ports are **`6543`** (primary), **`6547`** (replica through Toxiproxy to the standby), **`6545`** (Redis), **`8474`** (Toxiproxy API). Kind/Tilt still uses **6543–6546** for primary and replicas — **do not** run Compose and Tilt port-forwards on overlapping ports (see table above); **6547** is Compose-only for the proxied replica.
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
| `TEST_REPLICA_URL` | Streaming standby URL for the same cluster as `TEST_DATABASE_URL`; enables `pool_read_replica` tests. **Unset** → those tests no-op (skip). **CI** uses `localhost:6547` (replica via Toxiproxy). **Kind** uses **6544** (or **6546**). See [PRD_READ_REPLICA_TESTING.md](planning/PRD_READ_REPLICA_TESTING.md). |
| `TOXIPROXY_API` | Base URL for the [Toxiproxy](https://github.com/Shopify/toxiproxy) HTTP API (CI: `http://127.0.0.1:8474`). Enables `pooled_read_falls_back_to_primary_when_replica_lagging` to disable the `postgres_replica` proxy. **Unset** → that test returns early (no-op). Not used on Kind unless you deploy Toxiproxy yourself. |
| `LIFEGUARD_POOL_TEST_TIMING` | Optional; if non-empty and not `0`/`false`, `pool_read_replica` prints phase timings (setup, pool open, replay wait, reads, batch load) to **stderr**. |
| `TEST_REDIS_URL` or `REDIS_URL` | Optional; defaults to `redis://127.0.0.1:6379` when Postgres comes from env. For **CI Compose** or **Kind/Tilt** on the host ports above, set **`TEST_REDIS_URL=redis://127.0.0.1:6545`**. |

**CI Compose vs Kind/Tilt:** Compose publishes primary **6543**, proxied replica **6547**, Redis **6545**, Toxiproxy API **8474**. Kind/Tilt use **6543–6546** for primary and replicas. Do not run both on overlapping host ports; see the Tilt table and warning above.

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

export TEST_DATABASE_URL="postgres://postgres:${PGPASSWORD}@127.0.0.1:6543/postgres?options=-c%20search_path%3Dlifeguard"
export TEST_REPLICA_URL="postgres://postgres:${PGPASSWORD}@127.0.0.1:6547/postgres?options=-c%20search_path%3Dlifeguard"
export TEST_REDIS_URL="${TEST_REDIS_URL:-redis://127.0.0.1:6545}"
export TOXIPROXY_API="http://127.0.0.1:8474"

cargo nextest run -p lifeguard --all-features --profile db-serial -E 'binary(db_integration_suite)' pool_read_replica:: --no-fail-fast

docker compose -f .github/docker/docker-compose.yml down -v
```

If **6543 / 6547 / 6545 / 8474** are already in use, stop the conflicting service or override published ports in a local override file. **5432 / 5433 / 6379** are left free for Kind/Tilt by design. If **Tilt** is using **6543**, stop it before pointing `TEST_DATABASE_URL` at Compose’s primary.

Product requirements: [`docs/planning/PRD_READ_REPLICA_TESTING.md`](planning/PRD_READ_REPLICA_TESTING.md). Engineering design: [`docs/planning/DESIGN_READ_REPLICA_CI_AND_HARNESS.md`](planning/DESIGN_READ_REPLICA_CI_AND_HARNESS.md).

## Architecture

### Cluster Configuration

- **Kind cluster name:** `kind` (kubectl context **`kind-kind`**; created by `scripts/setup_kind_cluster.sh` if missing)
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
