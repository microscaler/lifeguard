# Lifeguard `postgres` Helm chart

Primary-direct Bitnami PostgreSQL for Kind / Tilt / shared-dev. **No Pgpool.**

This packages the topology that lived in:

- [`config/k8s/test-infrastructure/`](../../config/k8s/test-infrastructure/)
- [`.github/docker/docker-compose.yml`](../../.github/docker/docker-compose.yml)
- Historical shared Kind: `shared-kind-cluster/.../data/postgres` (`postgres` Service → primary)

## Why this chart exists

Bitnami `postgresql-ha` (repmgr + Pgpool) on the shared Kind cluster introduced instability for Lifeguard consumers (connection-slot exhaustion, write-split under load balancing, StatefulSet 2/3 Ready during Helm upgrades). Apps only need a stable primary; Lifeguard’s in-process pool is the multiplexer ([`docs/POOLING_OPERATIONS.md`](../../docs/POOLING_OPERATIONS.md)).

## Install

```bash
# Lifeguard local test namespace (primary + 2 streaming replicas)
helm upgrade --install postgresql ./charts/postgres \
  -n lifeguard-test --create-namespace \
  -f charts/postgres/values/lifeguard-test.yaml

# Shared Kind data namespace (primary only, Service name `postgres`)
helm upgrade --install postgres ./charts/postgres \
  -n data \
  -f charts/postgres/values/shared-kind.yaml
```

## Design rules

| Rule | Detail |
|------|--------|
| App path | Service always selects **primary** labels |
| Standbys | Optional; never behind the app Service |
| Init SQL | `files/11-lifeguard-schema.sql` (+ optional `10-pact-role.sql`) via postStart |
| Secrets | Prefer `auth.existingSecret` in shared clusters |

## Flux (shared Kind)

Installed by **shared-gitops-k8s-cluster** stack `postgres`:

- `GitRepository` `lifeguard-charts` → this repo, path `charts/postgres`
- `HelmRelease` `data/postgres` + profile `deployment-configuration/profiles/dev/postgres`
- Replaces retired stack `postgres-ha` (Bitnami postgresql-ha + Pgpool)

## Lint / dry-run

```bash
helm lint ./charts/postgres
helm template postgres ./charts/postgres -f charts/postgres/values/shared-kind.yaml | head
helm template postgresql ./charts/postgres -f charts/postgres/values/lifeguard-test.yaml | head
```
