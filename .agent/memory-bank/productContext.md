# Product Context

## Purpose
Lifeguard provides a complete ORM solution for Rust's coroutine runtime, enabling high-performance database access without async/await overhead.

## Target Use Cases
- BRRTRouter: High-throughput API routing (100,000+ requests/second)
- High-scale microservices requiring millions of requests/second
- Low-latency systems needing predictable p99 latency (< 5ms)

## Key Differentiators
- Native coroutine support (not async/await)
- Simpler API than SeaORM
- Optimized for coroutines
- JSON support as core feature (no feature flags)

## Current Capabilities
- Entity and Model generation
- Query builder
- ModelTrait with dynamic column access
- JSON column support
- Type-safe operations
