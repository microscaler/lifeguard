# Dashboards

Grafana is preconfigured to monitor:

- `lifeguard_queries_total` — Total queries run
- `lifeguard_query_duration_seconds` — Histogram of execution time
- `lifeguard_pool_queue_depth` — Gauge of queued queries
- `lifeguard_coroutine_wait_seconds` — Total coroutine blocking time

## Alerts

- p99 query latency > 100ms
- Pool queue depth > 50

Dashboards are defined in JSON and auto-importable via:

```bash
./scripts/import-grafana-dashboards.sh
```

