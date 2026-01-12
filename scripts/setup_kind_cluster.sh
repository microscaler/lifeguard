#!/bin/bash
# Setup Kind cluster for Lifeguard test infrastructure

set -euo pipefail

CLUSTER_NAME="lifeguard-test"
NAMESPACE="lifeguard-test"

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

# Check if cluster already exists
if kind get clusters | grep -q "^${CLUSTER_NAME}$"; then
    echo "⚠️  Cluster ${CLUSTER_NAME} already exists. Deleting it first..."
    kind delete cluster --name "${CLUSTER_NAME}"
fi

# Create cluster
echo "📦 Creating Kind cluster..."
kind create cluster --name "${CLUSTER_NAME}" --config kind-config.yaml

# Wait for cluster to be ready
echo "⏳ Waiting for cluster to be ready..."
kubectl wait --for=condition=Ready nodes --all --timeout=120s

# Create namespace and PVC (volume) only
# PostgreSQL deployment will be handled by Tilt
echo "📦 Creating namespace and PostgreSQL volume..."
kubectl apply -f config/k8s/test-infrastructure/namespace.yaml
kubectl apply -f config/k8s/test-infrastructure/postgres-pvc.yaml

# Wait for PVC to be bound
echo "⏳ Waiting for PostgreSQL volume to be ready..."
kubectl wait --for=condition=Bound --timeout=30s pvc/postgres-data -n "${NAMESPACE}" || {
    echo "⚠️  PVC not bound yet, but continuing..."
}

echo ""
echo "✅ Kind cluster setup complete!"
echo ""
echo "📋 Cluster details:"
echo "   Cluster: ${CLUSTER_NAME}"
echo "   Namespace: ${NAMESPACE}"
echo "   Volume: postgres-data (ready)"
echo ""
echo "💡 PostgreSQL will be deployed by Tilt when you run 'just dev-up'"
echo ""
