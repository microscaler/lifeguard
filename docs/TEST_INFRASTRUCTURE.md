# Test Infrastructure - Kind/Kubernetes Setup

This document describes the test infrastructure setup for Lifeguard using Kind (Kubernetes in Docker).

## Overview

Lifeguard uses Kind (Kubernetes in Docker) for integration testing instead of Docker Compose. This provides:
- Isolated test environments
- Kubernetes-native service discovery
- Better alignment with production deployments
- Consistent test infrastructure across environments

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

This will forward `localhost:5432` to the PostgreSQL service in the cluster.

### Teardown

```bash
just dev-down
```

### Rust crate integration tests (`db_integration_suite`)

The `lifeguard` package runs database-backed tests from a **single** integration binary (`tests/db_integration_suite.rs`) that shares one Postgres URL (and a Redis URL in context) per process.

| Variable | Role |
|----------|------|
| `DATABASE_URL` or `TEST_DATABASE_URL` | If set, **skips** starting Postgres via testcontainers; must point at a reachable Postgres. |
| `TEST_REDIS_URL` or `REDIS_URL` | Optional; defaults to `redis://127.0.0.1:6379` when Postgres comes from env. |

**Shared Postgres (e.g. Kind + `just dev-up`):** parallel test threads can exhaust connections (`too many clients`) or race on `CREATE TABLE`. Prefer:

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

**`just nt`:** runs nextest on the workspace but **skips** the `db_integration_suite` binary (it would hammer your shared Kind Postgres in parallel). Run DB integration separately:

```bash
just nt-db-suite
# alias: just nt-db
```

See also [`docs/planning/audits/LIFEGUARD_FOUNDATION_CONTINUATION.md`](planning/audits/LIFEGUARD_FOUNDATION_CONTINUATION.md) (Phase A / C).

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

The test infrastructure supports multiple ways to get the connection string:

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

# Check PostgreSQL deployment
kubectl get deployment postgres -n lifeguard-test

# Check PostgreSQL pods
kubectl get pods -n lifeguard-test

# Check PostgreSQL service
kubectl get svc postgres -n lifeguard-test
```

### View Logs

```bash
# PostgreSQL logs
kubectl logs -n lifeguard-test deployment/postgres -f
```

### Access PostgreSQL Shell

```bash
# Exec into PostgreSQL pod
kubectl exec -it -n lifeguard-test deployment/postgres -- psql -U postgres
```

## Troubleshooting

### Cluster Creation Fails

- Ensure Docker is running
- Check if port conflicts exist
- Try deleting existing cluster: `kind delete cluster --name lifeguard-test`

### PostgreSQL Not Ready

```bash
# Check pod status
kubectl describe pod -n lifeguard-test -l app=postgres

# Check events
kubectl get events -n lifeguard-test --sort-by='.lastTimestamp'
```

### Connection String Issues

- Verify service exists: `kubectl get svc postgres -n lifeguard-test`
- Check service DNS: `postgres.lifeguard-test.svc.cluster.local`
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
