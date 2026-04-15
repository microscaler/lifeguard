#!/bin/bash
# Delete the shared Kind cluster named `kind` (kubectl context kind-kind).
# `just dev-down` does not run this — use only when you intend to remove the cluster.

set -euo pipefail

CLUSTER_NAME="kind"

echo "🧹 Deleting Kind cluster (${CLUSTER_NAME})..."

if kind get clusters 2>/dev/null | grep -q "^${CLUSTER_NAME}$"; then
    kind delete cluster --name "${CLUSTER_NAME}"
    echo "✅ Cluster ${CLUSTER_NAME} deleted"
else
    echo "ℹ️  Cluster ${CLUSTER_NAME} does not exist"
fi
