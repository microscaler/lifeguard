# Lifeguard `postgres` Helm chart

Bitnami PostgreSQL **primary** Deployment + optional streaming **replicas**.  
App traffic always uses the primary Service (no Pgpool).

## Install

```bash
# Defaults (NodePort 30432, emptyDir-sized 1Gi PVCs, cluster default SC)
helm upgrade --install postgres ./charts/postgres -n data --create-namespace

# Shared Kind / iSCSI
helm upgrade --install postgres ./charts/postgres -n data \
  -f charts/postgres/values/dev-iscsi.yaml

# GKE Persistent Disk
helm upgrade --install postgres ./charts/postgres -n data \
  -f charts/postgres/values/gke.yaml
```

## Persistence (PVC / PV)

| Goal | Values |
|------|--------|
| Dynamic iSCSI (dev) | `primary.persistence.storageClass: zfs-iscsi` |
| Dynamic GKE PD | `primary.persistence.storageClass: premium-rwo` |
| Cluster default SC | leave `storageClass` empty |
| Force empty SC (static) | `storageClass: "-"` |
| Bind existing PV | `volumeName: <pv-name>` (size must match PV capacity) |
| Use existing PVC | `existingClaim: <pvc-name>` (chart skips PVC create) |
| Per-replica static bind | `replica.persistence.volumeNames: ["pv-a", "pv-b"]` |

Rebind runbook (Retain PV after old claim deleted):

1. Ensure PV `persistentVolumeReclaimPolicy: Retain` and `claimRef` cleared.
2. Set `primary.persistence.volumeName` + matching `size`.
3. Upgrade the release; PVC binds that PV.

## Auth

- Dev: chart creates Secret when `auth.existingSecret` is empty.
- Shared/Flux: set `auth.existingSecret: postgres-credentials` (SOPS profile).

## Flux

`shared-gitops-k8s-cluster` stack `postgres` → `HelmRelease` + profile values.
