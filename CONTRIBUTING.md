# Contributing

Welcome to **Lifeguard**! This document summarizes the project structure and offers pointers for newcomers.

## Overview

The repository provides **Lifeguard**, a coroutine-friendly PostgreSQL connection pool for Rust applications that use the `may` runtime and SeaORM. The README introduces it as a high-performance pool that bridges coroutines with async code, emphasizing minimal threads, real-time metrics and batch inserts.

Key architectural ideas are captured in the `book` documentation. The `Architecture` page summarizes how the `DbPoolManager` runs queries on a Tokio runtime through a worker loop while metrics are collected.

## Crate layout

```
src/
  lib.rs            # re-exports, macros, metrics and pool modules
  macros/           # lifeguard_execute!, lifeguard_go!, lifeguard_txn!, etc.
  pool/             # configuration loader, DbPoolManager, worker loop
  metrics.rs        # OpenTelemetry/Prometheus metrics
  test_helpers.rs   # utilities for tests
  tests_cfg/        # (ARCHIVED - legacy SeaORM entities moved to .archive/legacy-petstore/)
examples/           # database schema and generated SeaORM models
book/               # mdBook documentation
config/             # Prometheus/Grafana/otel configs
grafana/            # dashboards and alert rules
scripts/            # helper scripts (e.g. import Grafana dashboards)
```

`DbPoolManager` is the central type. It spawns a worker thread with a Tokio runtime that processes jobs sent through a `crossbeam_channel`. Its `execute` method enqueues a callback, waits for the result and records metrics such as queue depth and query duration. The worker loop performs actual database operations when it receives `LifeguardJob` messages.

Metrics are defined in `metrics.rs`, providing counters and histograms for query statistics that can be scraped by Prometheus and viewed through Grafana dashboards. Sample dashboards and alert rules are located in `grafana/`.

Macros in `src/macros/` offer ergonomic helpers. For instance, `lifeguard_execute!` runs a block inside a coroutine and returns its result synchronously, `lifeguard_query!` wraps an awaited query, and `lifeguard_txn!` handles transactions with automatic commit/rollback. `lifeguard_go!` even spawns a coroutine, executes a query, and stores the result in a named variable.

## Configuration & usage

Default database settings live in `config/config.toml`. A `justfile` provides tasks to start the Docker stack, apply migrations, run tests and seed example data.

The usage guide in the book demonstrates how to create a pool, spawn coroutines and run queries through Lifeguard.

## Testing & examples

Legacy SeaORM models have been archived to `.archive/legacy-petstore/`. Integration tests cover configuration, initialization, and query behavior. Helpers in `test_helpers.rs` manage temporary tables for tests.

## Next steps for learning

1. **Explore the macros** – review each macro in `src/macros/` to understand how coroutine code interacts with SeaORM queries.
2. **Check the worker design** – read through `DbPoolManager` and `worker.rs` to see how requests are queued and executed on the async runtime.
3. **Run the test infrastructure** – use the provided `just` commands (`just dev-up`) to spin up a Kind cluster with PostgreSQL for testing. See `docs/TEST_INFRASTRUCTURE.md` for details.
4. **Look into performance tuning** – see `book/src/performance.md` for tips on pool size and batch insert options.

Overall the repository combines a lightweight coroutine model with SeaORM for efficient PostgreSQL access while offering integrated metrics and a ready-to-run observability stack. This makes it suited for Rust microservices that require high throughput with good visibility into database operations.

