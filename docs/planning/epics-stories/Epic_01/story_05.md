# Story 05: Basic Metrics and Observability

## Description

Add basic Prometheus metrics and OpenTelemetry tracing to Lifeguard for observability. This enables monitoring of connection pool health, query performance, and system behavior.

## Acceptance Criteria

- [ ] Prometheus metrics exposed for: pool size, active connections, wait times, query durations
- [ ] OpenTelemetry tracing for database operations
- [ ] Metrics endpoint available (if applicable)
- [ ] Tracing spans cover: connection acquisition, query execution, connection release
- [ ] Documentation explains how to enable/configure observability

## Technical Details

- Use `prometheus` crate for metrics
- Use `opentelemetry` and `tracing` crates for tracing
- Metrics to track:
  - `lifeguard_pool_size` (gauge): Current pool size
  - `lifeguard_active_connections` (gauge): Active connections
  - `lifeguard_connection_wait_time` (histogram): Time waiting for connection
  - `lifeguard_query_duration` (histogram): Query execution time
  - `lifeguard_query_errors` (counter): Query errors
- Tracing spans: `lifeguard.acquire_connection`, `lifeguard.execute_query`, `lifeguard.release_connection`

## Dependencies

- Story 04: Redesign LifeguardPool for may_postgres

## Notes

- Metrics should be optional (feature flag)
- Tracing should be optional (feature flag)
- Consider adding structured logging as well
- Metrics should follow Prometheus naming conventions

