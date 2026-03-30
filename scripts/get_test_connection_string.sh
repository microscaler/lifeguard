#!/bin/bash
# Connection string for the Kind **primary** Postgres (matches CI compose host port 6543 when using Tilt).
#
# Replicas: postgresql://postgres:postgres@127.0.0.1:6544/postgres (replica-0), :6546 (replica-1)
# Redis: redis://127.0.0.1:6545

set -euo pipefail

NAMESPACE="lifeguard-test"

if [ -n "${KUBERNETES_SERVICE_HOST:-}" ]; then
    echo "postgresql://postgres:postgres@postgresql-primary.${NAMESPACE}.svc.cluster.local:5432/postgres"
elif kubectl config current-context 2>/dev/null | grep -q "kind-lifeguard-test"; then
    echo "postgresql://postgres:postgres@localhost:6543/postgres"
elif kubectl get svc postgresql-primary -n "${NAMESPACE}" &> /dev/null; then
    echo "postgresql://postgres:postgres@localhost:6543/postgres"
else
    echo "postgresql://postgres:postgres@localhost:6543/postgres"
fi
