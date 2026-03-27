# Story 10: Cache Statistics & Monitoring

## Description

Implement comprehensive cache statistics and monitoring that tracks cache effectiveness, performance, and health. This enables SREs to monitor and optimize cache usage.

## Acceptance Criteria

- [ ] Cache hit/miss counters (per model, per operation type)
- [ ] Cache latency histograms (Redis GET, SET operations)
- [ ] Cache size metrics (total keys, memory usage)
- [ ] Cache eviction metrics (TTL expirations, manual evictions)
- [ ] Cache key distribution (keys per table, keys per model)
- [ ] Per-model cache statistics
- [ ] Prometheus metrics export
- [ ] Unit tests demonstrate statistics collection

## Technical Details

- Metrics to track:
  ```
  lifeguard_cache_hits_total{table="users", operation="find_by_id"}
  lifeguard_cache_misses_total{table="users", operation="find_by_id"}
  lifeguard_cache_hit_rate{table="users"}
  lifeguard_cache_latency_seconds{operation="get", quantile="0.5|0.9|0.99"}
  lifeguard_cache_size_bytes
  lifeguard_cache_keys_total{table="users"}
  lifeguard_cache_evictions_total{reason="ttl|manual"}
  ```
- Per-model statistics:
  - Hit rate per model
  - Average TTL per model
  - Cache size per model
- Statistics collection:
  - Increment counters on cache operations
  - Record latencies in histograms
  - Track cache size periodically
- Export via Prometheus metrics endpoint

## Dependencies

- Story 04: Redis Integration for Transparent Caching
- Story 05: Basic Metrics and Observability (Epic 01)

## Notes

- Essential for cache optimization
- Should be low-overhead (async metrics collection)
- Consider sampling for high-throughput scenarios

