#!/usr/bin/env bash
# Run the perf suite against the compose stack. Usage: ./run.sh <label> [extra env...]
# Results: target/perf-results/<label>.{md,json}
set -euo pipefail
cd "$(dirname "$0")"
label="${1:-run}"
docker compose -p lifeguard-perf up -d --wait 2>/dev/null || docker compose -p lifeguard-perf up -d
for _ in $(seq 1 60); do
  docker compose -p lifeguard-perf exec -T postgres pg_isready -U postgres >/dev/null 2>&1 && break
  sleep 1
done
mkdir -p target/perf-results
export PERF_DATABASE_URL="${PERF_DATABASE_URL:-postgresql://postgres:perf@127.0.0.1:55440/postgres}"
export PERF_REDIS="${PERF_REDIS:-127.0.0.1:56380}"
export PERF_RESET=1 PERF_LABEL="$label" PERF_OUTPUT_JSON="target/perf-results/$label.json"
cargo build --release --bin lifeguard-perf ${PERF_NO_DEFAULT_FEATURES:+--no-default-features}
./target/release/lifeguard-perf | tee "target/perf-results/$label.md"
