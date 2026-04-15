#!/bin/bash
# Primary Postgres URL for Lifeguard tests/examples.
#
# **Default:** shared-kind-cluster forwards `data/postgres` to **localhost:${LIFEGUARD_PG_PORT:-5432}** (see that repo’s Tiltfile).
# **Legacy:** namespace `lifeguard-test` / `postgresql-primary` on **6543** if still deployed.
# **CI compose:** set `LIFEGUARD_PG_PORT=6543` to match `.github/docker/docker-compose.yml`.
#
# Replica/Redis URLs are not emitted here — use `justfile` TEST_REPLICA_URL / TEST_REDIS_URL (optional port-forwards from `data`).

set -euo pipefail

LG_PG_OPTS="?options=-c%20search_path%3Dlifeguard"
PG_PORT="${LIFEGUARD_PG_PORT:-5432}"

if [ -n "${KUBERNETES_SERVICE_HOST:-}" ]; then
    echo "postgresql://postgres:postgres@postgres.data.svc.cluster.local:5432/postgres${LG_PG_OPTS}"
elif kubectl get svc postgres -n data &> /dev/null; then
    echo "postgresql://postgres:postgres@127.0.0.1:${PG_PORT}/postgres${LG_PG_OPTS}"
elif kubectl get svc postgresql-primary -n lifeguard-test &> /dev/null; then
    echo "postgresql://postgres:postgres@127.0.0.1:6543/postgres${LG_PG_OPTS}"
else
    echo "postgresql://postgres:postgres@127.0.0.1:${PG_PORT}/postgres${LG_PG_OPTS}"
fi
