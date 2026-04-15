#!/bin/bash
# Setup Kind cluster for Lifeguard test infrastructure.
# Reuses an existing cluster named `kind` (kubectl context `kind-kind`) when present — does not delete it.
# Creates the cluster only if missing. Apply platform namespaces + shared stack from microscaler/shared-kind-cluster.
# kind-config.yaml is a symlink to ../../shared-kind-cluster/kind-config.yaml.

set -euo pipefail

CLUSTER_NAME="kind"

echo "🔧 Setting up Kind cluster for Lifeguard tests..."

# Check if kind is installed
if ! command -v kind &> /dev/null; then
    echo "❌ Error: kind is not installed. Please install it first:"
    echo "   brew install kind  # macOS"
    echo "   or visit: https://kind.sigs.k8s.io/docs/user/quick-start/#installation"
    exit 1
fi

# Check if kubectl is installed
if ! command -v kubectl &> /dev/null; then
    echo "❌ Error: kubectl is not installed. Please install it first:"
    echo "   brew install kubectl  # macOS"
    echo "   or visit: https://kubernetes.io/docs/tasks/tools/"
    exit 1
fi

# Create shared cluster if missing (do not delete an existing cluster by default)
if ! kind get clusters 2>/dev/null | grep -q "^${CLUSTER_NAME}$"; then
    echo "📦 Creating shared Kind cluster (${CLUSTER_NAME})..."
    kind create cluster --config kind-config.yaml
else
    echo "✅ Kind cluster '${CLUSTER_NAME}' already exists; skipping create."
fi

# Wait for cluster to be ready
echo "⏳ Waiting for cluster to be ready..."
kubectl wait --for=condition=Ready nodes --all --timeout=120s

echo ""
echo "✅ Kind cluster setup complete!"
echo ""
echo "📋 Cluster details:"
echo "   Cluster: ${CLUSTER_NAME} (kubectl context: kind-kind)"
echo ""
echo "💡 Platform stack: microscaler/shared-kind-cluster (just dev-up / tilt up there). Lifeguard Tilt is builds/tests only."
echo ""
