#!/bin/bash
# Get connection string for test database
#
# This script detects the environment and returns the appropriate connection string:
# - If running locally (host machine): uses localhost:5432 (Tilt port-forward)
# - If running in cluster: uses service DNS name
# - Fallback: uses localhost:5432

set -euo pipefail

NAMESPACE="lifeguard-test"

# Check if we're running inside a Kubernetes pod
# If KUBERNETES_SERVICE_HOST is set, we're in a pod
if [ -n "${KUBERNETES_SERVICE_HOST:-}" ]; then
    # Running inside cluster - use service DNS
    echo "postgresql://postgres:postgres@postgres.${NAMESPACE}.svc.cluster.local:5432/postgres"
elif kubectl config current-context | grep -q "kind-lifeguard-test"; then
    # Using Kind cluster from host machine - use localhost (Tilt port-forward)
    # Tilt automatically port-forwards PostgreSQL to localhost:5432
    echo "postgresql://postgres:postgres@localhost:5432/postgres"
elif kubectl get svc postgres -n "${NAMESPACE}" &> /dev/null; then
    # Service exists but not Kind - try to use localhost (may need port-forward)
    echo "postgresql://postgres:postgres@localhost:5432/postgres"
else
    # Fallback to localhost
    echo "postgresql://postgres:postgres@localhost:5432/postgres"
fi
