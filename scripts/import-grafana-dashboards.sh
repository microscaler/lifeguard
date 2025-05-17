#!/bin/env bash

#
# GRAFANA_HOST=http://grafana.local GRAFANA_USER=me GRAFANA_PASS=secret ./scripts/import-grafana-dashboards.sh
#

GRAFANA_HOST=${GRAFANA_HOST:-http://localhost:3000}
GRAFANA_USER=${GRAFANA_USER:-admin}
GRAFANA_PASS=${GRAFANA_PASS:-admin}
DASHBOARD_DIR="./grafana/dashboards"

for file in "$DASHBOARD_DIR"/*.json; do
  echo "📥 Importing dashboard: $file"
  curl -s -u "$GRAFANA_USER:$GRAFANA_PASS" \
    -X POST "$GRAFANA_HOST/api/dashboards/db" \
    -H "Content-Type: application/json" \
    -d @"$file"
  echo ""
done
