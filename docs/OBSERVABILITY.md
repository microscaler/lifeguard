# Observability in Lifeguard

Lifeguard provides comprehensive observability through Prometheus metrics and OpenTelemetry tracing. These features are optional and can be enabled via feature flags.

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

## See Also

- [Prometheus Documentation](https://prometheus.io/docs/)
- [OpenTelemetry Rust](https://opentelemetry.io/docs/instrumentation/rust/)
- [Tracing Documentation](https://docs.rs/tracing/)
