#!/bin/bash
# Teardown Kind cluster for Lifeguard test infrastructure

set -euo pipefail

CLUSTER_NAME="lifeguard-test"

echo "🧹 Tearing down Kind cluster..."

if kind get clusters | grep -q "^${CLUSTER_NAME}$"; then
    kind delete cluster --name "${CLUSTER_NAME}"
    echo "✅ Cluster ${CLUSTER_NAME} deleted"
else
    echo "ℹ️  Cluster ${CLUSTER_NAME} does not exist"
fi
