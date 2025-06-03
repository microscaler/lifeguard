<p align="center">
  <img src="/docs/images/Lifeguard_cropped.png" alt="Lifeguard logo" />
</p>

# 🛟 Lifeguard: Coroutine-Driven Database Runtime for Rust

**Lifeguard** is a coroutine-first, high-performance PostgreSQL connection pool built for Rust using [SeaORM](https://www.sea-ql.org/SeaORM/) and the [may](https://github.com/Xudong-Huang/may) coroutine runtime.

It provides predictable, low-latency execution for async workloads while reducing thread and memory overhead.

---

## 🔥 Why Lifeguard?

- ⚡ Built from scratch using coroutine workers (`may::go!`) for efficiency.
- ✅ No thread-per-connection overhead.
- ✅ Clean task model: one type-safe job per coroutine.
- ✅ Fully typed `execute<T>` method — no `Any`, no boxing.
- ✅ Strong observability: OpenTelemetry + Prometheus support.
- ✅ Designed for concurrency and correctness.

---

## 🏗️ Architecture Overview

```text
┌────────────┐         ┌─────────────────────┐         ┌────────────────────────────┐
│ may::go!   │─────▶──▶  DbPoolManager       │─────▶──▶   SeaORM / raw SQL queries  │
└────────────┘         └─────────────────────┘         └────────────────────────────┘
```

- **DbPoolManager**: distributes tasks to coroutine workers via a bounded channel.
- **Worker Runtimes**: each runs a local tokio runtime and a shared SeaORM `DatabaseConnection`.

```mermaid
sequenceDiagram
    participant App
    participant Config
    participant PoolManager
    participant Workers
    participant Tokio
    participant SeaORM
    participant DB

    App->>Config: load() from config.toml and ENV
    Config-->>App: DatabaseConfig

    App->>PoolManager: new_with_params(url, pool_size)
    loop for each worker
        PoolManager->>Workers: spawn may::go! coroutine
        Workers->>Tokio: build current-thread Runtime
        Tokio->>SeaORM: Database::connect(url)
        SeaORM-->>Workers: DatabaseConnection
        Workers->>run_worker_loop: loop(rx, db)
    end

    App->>PoolManager: execute(|conn| async { ... })
    PoolManager->>Workers: send DbRequest::Run(Box<...>)
    Workers->>SeaORM: run closure: db.query_*/execute
    SeaORM->>DB: actual SQL
    DB-->>SeaORM: result
    SeaORM-->>Workers: query result
    Workers-->>App: Result<T, DbErr>

    App->>App: continue with result

```


```mermaid
graph TD
    App[Application]
    Config[Config Loader]
    Metrics[Metrics Exporter]
    Pool[DbPoolManager]
    Worker[Coroutine Workers]
    Tokio[Tokio Runtime]
    SeaORM[SeaORM ORM]
    PG[(PostgreSQL)]

    App --> Config
    App --> Pool
    App --> Metrics
    Pool -->|spawn| Worker
    Worker --> Tokio
    Worker --> SeaORM
    SeaORM --> PG
    Metrics -->|records| Pool
    Metrics -->|records| Worker
```

```mermaid
stateDiagram-v2
    [*] --> Spawned
    Spawned --> Initializing_Runtime : build_runtime
    Initializing_Runtime --> Connecting : db_connect
    Connecting --> Running : start_loop

    state Running {
        [*] --> Idle
        Idle --> Receiving : recv_job
        Receiving --> Executing : run_closure
        Executing --> Idle
        Receiving --> [*] : shutdown
    }
```

```mermaid
stateDiagram-v2
    [*] --> Spawned
    Spawned --> InitRuntime : ok
    InitRuntime --> Connecting : db_ok
    Connecting --> Running

    state Running {
        [*] --> Idle
        Idle --> Receiving : job_received
        Receiving --> Executing : run_ok
        Executing --> Idle

        Executing --> JobError : panic / query_fail
        JobError --> Idle : recovered

        Receiving --> [*] : rx_closed
    }

    InitRuntime --> FatalError : runtime_fail
    Connecting --> FatalError : db_connect_fail
```

```mermaid
graph TD
    App[Application]
    Pool[DbPoolManager]
    Channel[bounded_100_channel]
    Worker1[Worker 1]
    Worker2[Worker 2]
    Blocked[Blocked Caller]

    App -->|submit job| Pool
    Pool -->|send| Channel
    Channel --> Worker1
    Channel --> Worker2
    Channel -->|queue full| Blocked
```

```mermaid
graph TD
    macro["lifeguard_execute!"]
    pool["DbPoolManager"]
    closure["FnOnce wrapper"]
    channel["DbRequest::Run"]
    worker["Worker Thread"]
    dbcall["SeaORM Operation"]

    macro --> pool
    pool --> closure
    closure --> channel
    channel --> worker
    worker --> dbcall
```





---

## 💻 Getting Started

```toml
[dependencies]
lifeguard = { git = "https://github.com/microscaler/lifeguard", branch = "overhaul" }
```

```rust
let config = DatabaseConfig::load()?;
let pool = DbPoolManager::new_with_params(&config.url, config.max_connections)?;

may::go!(move || {
    let users = pool.execute(|db| async move {
        MyEntity::find().all(&db).await
    });
});
```

---

## 📊 Observability

Powered by `opentelemetry` + `opentelemetry-prometheus`.

- `lifeguard_queries_total`
- `lifeguard_query_duration_seconds`
- `lifeguard_coroutine_wait_seconds`
- `lifeguard_pool_queue_depth`

Enable scraping via:
```bash
just metrics-server
```

---

## 🔧 Testing

Lifeguard includes a `test_pool!()` macro that creates a mock connection pool for unit testing with SeaORM’s `MockDatabase`.

```rust
let pool = test_pool!();
```

---

## 🚀 Roadmap to Alpha Release

### ✅ Implemented
- [x] Coroutine-backed worker pool
- [x] `execute<T>` dispatch method
- [x] Fully typed `DbRequest` with closure-based jobs
- [x] OpenTelemetry metrics instrumentation
- [x] Configuration from TOML + env
- [x] Entity CRUD tests
- [x] Transaction rollback validation
- [x] Concurrent task handling
- [x] Retry loop simulation

### 🧩 To Implement
- [ ] Tracing integration via `tracing::instrument`
- [ ] Expose `/metrics` via HTTP
- [ ] Retry policies with exponential backoff
- [ ] Graceful shutdown signal to worker loop
- [ ] Macros for `lifeguard_execute!`, `lifeguard_txn!`
- [ ] CLI: metrics reporter or local benchmark runner
- [ ] Dashboards for Grafana

---

## 📜 License

Licensed under Apache-2.0.
