#!/bin/bash
# Streaming-replica Postgres URL for Lifeguard tests / perf-orm (read pool, optional).
#
# **Shared Kind (microscaler/shared-kind-cluster, namespace `data`):** the Service in front of the first
# standby is **`postgres-replica-0`** (matches the Tilt resource name). Older stacks used
# **`postgresql-replica-0`**. This script picks whichever exists when `kubectl` can see the cluster.
#
# **Host dev:** the URL targets **127.0.0.1:${LIFEGUARD_REPLICA_PORT:-6544}**. **microscaler/shared-kind-cluster** Tilt
# port-forwards replica-0 to that port (see that repo’s `Tiltfile`). Without it, run
# `kubectl port-forward -n data svc/postgres-replica-0 6544:5432` (or set `LIFEGUARD_REPLICA_URL_OVERRIDE`).
#
# **In-cluster:** emits DNS **`${LIFEGUARD_REPLICA_SVC:-postgres-replica-0}.data.svc.cluster.local:5432`**.
#
# **CI / Compose:** set **`LIFEGUARD_REPLICA_URL_OVERRIDE`** (or export **`TEST_REPLICA_URL`** / **`PERF_REPLICA_URL`**
# yourself) — this script is for local Kind-style defaults.

set -euo pipefail

LG_PG_OPTS="?options=-c%20search_path%3Dlifeguard"
REPLICA_PORT="${LIFEGUARD_REPLICA_PORT:-6544}"
REPLICA_HOST="${LIFEGUARD_REPLICA_HOST:-127.0.0.1}"
# Default credentials match local Kind / dev; set PGPASSWORD for clusters that use secrets.
PG_USER="${POSTGRES_USER:-postgres}"
PG_PASS="${PGPASSWORD:-postgres}"

if [ -n "${LIFEGUARD_REPLICA_URL_OVERRIDE:-}" ]; then
    echo "${LIFEGUARD_REPLICA_URL_OVERRIDE}"
    exit 0
fi

replica_svc_name() {
    local primary="${LIFEGUARD_REPLICA_SVC:-}"
    if [ -n "$primary" ]; then
        echo "$primary"
        return
    fi
    local s
    for s in postgres-replica-0 postgresql-replica-0; do
        if kubectl get svc "$s" -n data &>/dev/null; then
            echo "$s"
            return
        fi
    done
    echo "postgres-replica-0"
}

if [ -n "${KUBERNETES_SERVICE_HOST:-}" ]; then
    svc="$(replica_svc_name)"
    echo "postgresql://${PG_USER}:${PG_PASS}@${svc}.data.svc.cluster.local:5432/postgres${LG_PG_OPTS}"
    exit 0
fi

# shellcheck disable=SC2001
echo "postgresql://${PG_USER}:${PG_PASS}@${REPLICA_HOST}:${REPLICA_PORT}/postgres${LG_PG_OPTS}"
