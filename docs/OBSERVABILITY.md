# Observability in Lifeguard

Lifeguard provides comprehensive observability through Prometheus metrics and OpenTelemetry tracing. These features are optional and can be enabled via feature flags.

## Kubernetes (Kind / Tilt): apply and refresh dashboards

The repo ships a **Grafana + Prometheus + Loki + OTEL** stack under [`config/k8s/observability/`](https://github.com/microscaler/lifeguard/tree/main/config/k8s/observability) (Kustomize, namespace **`lifeguard-test`**). Use this to **re-apply** manifests after you change dashboard JSON, datasources, or scrape configs.

From the **repository root**, with your Kind (or other) cluster selected in `kubectl`:

```bash
kubectl apply -k config/k8s/observability
```

Dashboards are provisioned from a **ConfigMap** (see `kustomization.yaml` → `grafana-dashboard-lifeguard-kind.json`). After you edit that JSON, `kubectl apply` updates the ConfigMap, but Grafana only sees the new files after the pod **reloads the mount**. Restart Grafana:

```bash
kubectl rollout restart deployment/grafana -n lifeguard-test
```

Then open Grafana (e.g. port **3000** if Tilt forwards it) and **hard-refresh** the browser so the UI does not show a stale panel.

**Tilt:** [`Tiltfile`](https://github.com/microscaler/lifeguard/blob/main/Tiltfile) runs the same `kustomize` path on `tilt up`. If the UI does not pick up dashboard edits automatically, run the two commands above.

**Shortcut:** `kubectl delete pod -n lifeguard-test -l app=grafana` also forces a new pod with fresh mounts (same effect as `rollout restart` for a single replica).

## Feature Flags

### Default Behavior

By default, both metrics and tracing are enabled:

```toml
[dependencies]
lifeguard = { version = "0.1", features = ["default"] }
```

Or explicitly:

```toml
[dependencies]
lifeguard = { version = "0.1", features = ["metrics", "tracing"] }
```

### Disable Observability

To disable all observability features:

```toml
[dependencies]
lifeguard = { version = "0.1", default-features = false }
```

### Enable Only Metrics

```toml
[dependencies]
lifeguard = { version = "0.1", default-features = false, features = ["metrics"] }
```

### Enable Only Tracing

```toml
[dependencies]
lifeguard = { version = "0.1", default-features = false, features = ["tracing"] }
```

## Prometheus Metrics

When the `metrics` feature is enabled, Lifeguard exposes the following Prometheus metrics:

### Metrics

| Metric Name | Type | Description |
|------------|------|-------------|
| `lifeguard_pool_size` | Gauge | Current connection pool size |
| `lifeguard_active_connections` | Gauge | Number of active connections |
| `lifeguard_connection_wait_time_seconds` | Histogram | Time spent waiting for a connection |
| `lifeguard_query_duration_seconds` | Histogram | Query execution time |
| `lifeguard_query_errors_total` | Counter | Total number of query errors |
| `lifeguard_wal_monitor_replica_routing_disabled` | Gauge | 1 if the WAL lag monitor gave up connecting to the replica (PRD R7.3) |
| `lifeguard_pool_acquire_timeout_total` | Counter | Pool acquire timeouts (`LifeError::PoolAcquireTimeout`) |
| `lifeguard_pool_slot_heal_total` | Counter | Slot heal reconnects after connectivity-class errors |
| `lifeguard_pool_connection_rotated_total` | Counter | Connections rotated after `max_connection_lifetime` (PRD R3.1) |

### Accessing Metrics

The metrics exporter is available via `lifeguard::metrics::METRICS.exporter`:

```rust
use lifeguard::metrics::METRICS;

// Get the Prometheus registry
let registry = METRICS.exporter.registry();

// Export metrics in Prometheus format
let encoder = prometheus::TextEncoder::new();
let metric_families = registry.gather();
let mut buffer = Vec::new();
encoder.encode(&metric_families, &mut buffer).unwrap();
let output = String::from_utf8(buffer).unwrap();
println!("{}", output);
```

### Example: HTTP Metrics Endpoint

```rust
use lifeguard::metrics::METRICS;
use std::io::Write;

fn serve_metrics() -> std::io::Result<()> {
    let registry = METRICS.exporter.registry();
    let encoder = prometheus::TextEncoder::new();
    let metric_families = registry.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    
    // Serve via HTTP (example)
    // println!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n{}", 
    //          String::from_utf8(buffer).unwrap());
    
    Ok(())
}
```

### Programmatic Access

You can also update metrics programmatically:

```rust
use lifeguard::metrics::METRICS;

// Update pool size
METRICS.set_pool_size(10);

// Update active connections
METRICS.set_active_connections(5);

// Record connection wait time
METRICS.record_connection_wait(std::time::Duration::from_millis(100));
```

## OpenTelemetry Tracing

When the `tracing` feature is enabled, Lifeguard creates OpenTelemetry spans for database operations:

### Spans

- **`lifeguard.acquire_connection`**: Created when establishing a new database connection
- **`lifeguard.execute_query`**: Created for each query execution (includes the query string)
- **`lifeguard.release_connection`**: Created when releasing a connection (future pool implementation)
- **`lifeguard.pool_slot_heal`**: Created when a pool worker replaces a `Client` after a connectivity-class error (PRD R8.2)

### Setting Up Tracing

To use tracing, you need to initialize a tracing subscriber. Here's an example using `tracing-subscriber`:

```rust
use tracing_subscriber;

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
}
```

### Example: Using Tracing

```rust
use lifeguard::connection::connect;
use lifeguard::executor::{MayPostgresExecutor, LifeExecutor};

// Initialize tracing
tracing_subscriber::fmt::init();

// Connection acquisition will create a span
let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")?;

let executor = MayPostgresExecutor::new(client);

// Query execution will create a span with the query string
let row = executor.query_one("SELECT COUNT(*) FROM users", &[])?;
```

### Viewing Traces

Tracing output can be viewed:

1. **Console**: Via `tracing-subscriber` with `fmt` layer
2. **Jaeger**: Export to Jaeger using OpenTelemetry exporters
3. **Other backends**: Use OpenTelemetry exporters for your preferred backend

## Integration

### Automatic Instrumentation

Metrics and tracing are automatically integrated into:

- **Connection Module** (`lifeguard::connection`): Tracks connection acquisition
- **Executor Module** (`lifeguard::executor`): Tracks query execution and errors

No code changes are required - instrumentation happens automatically when features are enabled.

### Manual Instrumentation

For custom instrumentation:

```rust
use lifeguard::metrics::{METRICS, tracing_helpers};

// Create a custom span
#[cfg(feature = "tracing")]
let span = tracing_helpers::execute_query_span("SELECT custom_query").entered();

// Record custom metrics
#[cfg(feature = "metrics")]
METRICS.record_query_duration(std::time::Duration::from_millis(50));
```

## Configuration

### Environment Variables

When using `tracing-subscriber`, you can control log levels via `RUST_LOG`:

```bash
# Show all tracing events
RUST_LOG=info cargo run

# Show only lifeguard spans
RUST_LOG=lifeguard=info cargo run

# Show debug-level tracing
RUST_LOG=lifeguard=debug cargo run
```

### Disabling in Production

For minimal overhead, disable observability features:

```toml
[dependencies]
lifeguard = { version = "0.1", default-features = false }
```

This removes all observability code at compile time, resulting in zero overhead.

## Best Practices

1. **Enable observability in production**: Use metrics and tracing to monitor your application
2. **Use feature flags**: Only enable what you need to reduce binary size
3. **Export metrics**: Set up a metrics endpoint for Prometheus to scrape
4. **Configure tracing**: Use appropriate log levels and export to your tracing backend
5. **Monitor errors**: Watch `lifeguard_query_errors_total` for database issues
6. **Track performance**: Use `lifeguard_query_duration_seconds` histograms for performance analysis

## Example: Complete Setup

```rust
use lifeguard::connection::connect;
use lifeguard::executor::{MayPostgresExecutor, LifeExecutor};
use lifeguard::metrics::METRICS;

// Initialize tracing
#[cfg(feature = "tracing")]
tracing_subscriber::fmt::init();

// Connect (automatically creates tracing span)
let client = connect("postgresql://postgres:postgres@localhost:5432/mydb")?;

let executor = MayPostgresExecutor::new(client);

// Execute query (automatically records metrics and creates span)
let row = executor.query_one("SELECT COUNT(*) FROM users", &[])?;
let count: i64 = row.get(0);

// Access metrics
#[cfg(feature = "metrics")]
{
    let registry = METRICS.exporter.registry();
    // Export or serve metrics...
}

Ok(())
```

## PostgreSQL replication lag: time, bytes, and dashboards

Lifeguard’s [`WalLagMonitor`](https://github.com/microscaler/lifeguard/blob/main/src/pool/wal.rs) (used by [`LifeguardPool`](https://github.com/microscaler/lifeguard/blob/main/src/pool/pooled.rs) for read routing) evaluates **byte** lag on the standby (receive vs replay LSN) and can optionally treat **apply lag** in wall-clock time (`wal_lag_max_apply_lag_seconds` / [`WalLagPolicy`](https://github.com/microscaler/lifeguard/blob/main/src/pool/wal.rs)) as lagging. **PostgreSQL** also exposes **time-based** lag on the **primary** via `pg_stat_replication` (`write_lag`, `flush_lag`, `replay_lag` as `interval` values). For operations and SRE work, **chart both** infra metrics and Lifeguard’s policy: bytes explain *queue depth*; **replay lag** (time) on the primary is often the clearest “staleness” signal.

### Postgres views to know

| Where | What to use |
|-------|-------------|
| **Primary** | `pg_stat_replication` — per standby: `application_name`, `state`, `sync_state`, **`write_lag`**, **`flush_lag`**, **`replay_lag`** (time), plus LSNs (`sent_lsn`, `write_lsn`, `flush_lsn`, `replay_lsn`) for byte/LSN math. |
| **Standby** | `pg_last_wal_replay_lsn()`, recovery state; `pg_stat_wal_receiver` for receiver health. |

`replay_lag` can be **NULL** if the standby has not reported recent activity; dashboards and alerts should treat NULL explicitly.

### Dashboard stacks (pick what matches your platform)

These are common ways teams get **time-based** replica lag without writing SQL by hand every incident:

| Stack / product | Notes |
|-----------------|--------|
| **Grafana + Prometheus + [postgres_exporter](https://github.com/prometheus-community/postgres_exporter)** | Open-source default: scrape `pg_stat_replication` (or query mappings). Build one panel per replica: `replay_lag` seconds, optional `flush_lag` / `write_lag` to see *where* delay accumulates. |
| **Grafana + [pgwatch2](https://github.com/cybertec-postgresql/pgwatch2)** | Postgres-focused collector + Grafana dashboards; good when you want opinionated Postgres DBA views out of the box. |
| **Percona Monitoring and Management (PMM)** | Postgres + OS + query analytics; replication dashboards are a first-class use case. |
| **Datadog** (Postgres integration / Database Monitoring) | Hosted: replica lag, connections, slow queries in one product; good for teams already on Datadog. |
| **New Relic**, **Dynatrace**, **Splunk Observability** | Similar pattern: agent or remote integration + prebuilt DB dashboards; verify **replay** / **lag seconds** panels exist for your Postgres flavor. |
| **AWS RDS / Aurora** | **CloudWatch** `ReplicaLag` (seconds for Aurora replicas) and **RDS Performance Insights** for wait/load; still correlate with application routing if you use `LifeguardPool` with a replica URL. |
| **Google Cloud SQL** | **Query insights** + monitoring metrics for replication lag (metric names vary by edition; use the console dashboards). |
| **Azure Database for PostgreSQL** | Azure Monitor metrics for replication / lag (flexible server vs single server differ—use the product’s “replication” blade). |
| **CockroachDB / AlloyDB / other forks** | If you are not on stock Postgres, use **that** vendor’s replication metrics; the concepts (apply lag vs send lag) still apply. |
| **[pganalyze](https://pganalyze.com/)** | Postgres-specific SaaS: replication, query stats, explain plans—low friction for “why is replay lag spiking?” without building Grafana from scratch. |
| **Grafana Cloud** (with Postgres data source or remote Prometheus) | Same Grafana UX as self-hosted; useful when you want managed Grafana + alerts without running the stack. |
| **VictoriaMetrics / Mimir** | Drop-in Prometheus long-term storage; keep **one** Grafana—replication panels stay identical, retention gets cheaper. |
| **Zabbix / Icinga / Sensu** (Postgres plugins) | Still common in enterprises: ensure plugins expose **time** lag, not only “behind by X bytes,” or add custom `pg_stat_replication` checks. |

Nothing in this list replaces **your** runbook: one **golden** dashboard per environment beats ten overlapping tools.

### Panels that reduce SRE cognitive load

Design the **primary + replicas** row so a tired on-call can answer in one glance:

1. **Per replica (primary perspective):** `replay_lag` as **seconds** (or the same as a heatmap) — primary “read your writes” vs this replica.
2. **Same row:** WAL **bytes** behind (LSN diff) — ties to queue depth and disk/network.
3. **Breakdown:** `write_lag` vs `flush_lag` vs `replay_lag` — distinguishes network/send issues from apply bottlenecks on the standby.
4. **Health:** `state = streaming`, `sync_state` if you use sync replicas, replication **slot** lag if you use slots (logical or physical).
5. **Saturation:** primary WAL generation rate, standby I/O, disk space — lag often follows load, not the other way around.
6. **Application:** Lifeguard pool metrics (`lifeguard_*`) on the same board or linked — so you can see “replica lag high” next to “queries routed to primary”.

**Alerts:** Prefer **replay lag seconds** (and/or SLO-based thresholds) alongside byte thresholds; bytes alone mislead when WAL volume is low or bursty.

### Incident-friendly habits (lower cognitive load)

- **One URL** for “database health” pinned in the incident channel; avoid hunting across three vendors mid-outage.
- **Same layout** in staging and prod (fewer surprises); only thresholds differ.
- **Annotations:** mark deploys, failover drills, and maintenance on the lag chart so correlation is obvious.
- **Runbook links** in Grafana panel descriptions (e.g. “if `replay_lag` ↑ and `write_lag` flat → check network path to standby”).
- **Logs next to metrics:** ship Postgres logs (or `log_min_duration_statement` samples) to the same observability stack so “apply slow” has context without SSH.
- **NULL-safe alerts:** do not page on `replay_lag IS NULL` alone—combine with `state <> 'streaming'` or missing scrapes.

### Relationship to Lifeguard

- **Lifeguard** uses its own WAL poll for **routing** when you configure a replica URL; see [PRD_CONNECTION_POOLING.md](../planning/PRD_CONNECTION_POOLING.md) and `WalLagMonitor` for behavior and roadmap (e.g. time-based policy in requirements).
- **Postgres dashboards** above are **infrastructure** truth; keep them even if the app exposes metrics — they catch issues before the app notices.

## See Also

- [Prometheus Documentation](https://prometheus.io/docs/)
- [OpenTelemetry Rust](https://opentelemetry.io/docs/instrumentation/rust/)
- [Tracing Documentation](https://docs.rs/tracing/)
- [PostgreSQL: Monitoring replication](https://www.postgresql.org/docs/current/monitoring-stats.html#MONITORING-STATS-VIEWS) (`pg_stat_replication`)
