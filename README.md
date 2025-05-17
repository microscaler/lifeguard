<p align="center">
  <img src="/docs/images/Lifeguard_cropped.png" alt="Lifeguard logo" />
</p>

# 🛟 Lifeguard

**Lifeguard** is a coroutine-friendly, high-performance PostgreSQL connection pool designed for Rust applications using [SeaORM](https://www.sea-ql.org/SeaORM/) and the [`may`](https://github.com/Xudong-Huang/may) coroutine runtime.

It aims to provide efficient database interaction with:
- **Minimal thread usage**
- **Fully coroutine-to-async bridging**
- **Real-time observability**
- **High insert throughput with batching**

> Designed for use in microservice stacks, data ingest pipelines, and performance-sensitive Rust backends.
> 
> Implemented through the use of `may::go!` with `pool.execute(...)` to safely run queries inside green threads.


---

## 🚀 Goals

- ⚡ **Fast**: Handle up to millions of queries per second using low-overhead coroutines
- 🧠 **Simple**: Just plug into your SeaORM + may app and go
- 📊 **Observable**: Integrated metrics via OpenTelemetry, Prometheus, and Grafana
- 🔁 **Flexible**: Supports single queries or high-volume batch inserts
- 📦 **Portable**: Fully testable with `docker-compose.test.yml` including Postgres, Loki, Grafana, and Prometheus

---

## 🧱 Why Use Lifeguard?
You should consider Lifeguard if you:

- ✅ Are building microservices in Rust with high query throughput
- ✅ Want to limit thread usage with efficient coroutine handling
- ✅ Need transparent Prometheus metrics + dashboards
- ✅ Want to scale up ingest/batching with predictable latency

---

## 🏗️ Architecture

```
┌────────────┐         ┌────────────────┐         ┌────────────────────────┐
│ may::go!   │─────▶──▶│  DbPoolManager │─────▶──▶   tokio + SeaORM client │
└────────────┘         └────────────────┘         └─────────────▲──────────┘
                                                            async await
```


### Components

| Component                 | Description                                                                     |
|---------------------------|---------------------------------------------------------------------------------|
| `Lifeguard library`       | Main API — sends coroutine-safe query jobs to SeaORM                            |
| `Postgress`               | Postgres database (Implementer may chose a flavour of their choice)             |
| `Postgress Plugins`       | Compatible with Postgres plugins such as pgJWT                                  |
| `Redis`                   | Redis caching layer - (Optional and entirely transparent to the developer)      |
| `metrics`                 | Lifeguard exposes real-time metrics via OpenTelemetry                           |
| `Logging`                 | Loki for logging                                                                |
| `Grafana`                 | Grafana for real-time dashboards and alerts                                     |
| `docker-compose`          | Local dev/test stack for Postgres, Grafana, Loki, Prometheus, OTel              |



---

## 🔧 Features

### ✅ Coroutine-safe execution

`DbPoolManager` uses `may::go!` to run queries in green threads with Macro wrappers around the SeaORM API to allow for 
rapid assimilation by developers adopting the library.

High-volume seed or ingest operations are supported through efficient batched inserts (handling 500+ rows per query).

The following table shows the mapping of Lifeguard macros to SeaORM API calls:

| Macro                                     | Maps to SeaORM API                   |
|-------------------------------------------|--------------------------------------|
| `lifeguard_connect!`                      | `sea_orm::Database::connect!`        |
| `lifeguard_execute`                       | `execute`                            |
| `lifeguard_query`                         | `query`                              |
| `lifeguard_insert`                        | `insert`                             |
| `lifeguard_update`                        | `update`                             |
| `lifeguard_delete`                        | `delete`                             |
| `lifeguard_find`                          | `find`                               |
| `lifeguard_find_one`                      | `find_one`                           |
| `lifeguard_find_many`                     | `find_many`                          |
| `lifeguard_find_by`                       | `find_by`                            |
| `lifeguard_find_by_one`                   | `find_by_one`                        |
| `lifeguard_find_by_many`                  | `find_by_many`                       |
| `lifeguard_find_by_count`                 | `find_by_count`                      |
| `lifeguard_find_by_exists`                | `find_by_exists`                     |
| `lifeguard_find_by_exists_one`            | `find_by_exists_one`                 |
| `lifeguard_find_by_exists_many`           | `find_by_exists_many`                |
| `lifeguard_find_by_exists_count`          | `find_by_exists_count`               |


---

### ✅ Redis cached operations

Lifeguard supports Redis caching for query results, allowing you to cache the results of expensive queries and reduce 
the load on your database as well as the latency of your application.

Caching is transparent to the user and can be enabled by setting the `LIFEGUARD_CACHE` environment variable to `true`.

The caching is implemented internally within the macros and toggled by the `LIFEGUARD_CACHE` environment variable.

Caching operations havebsensible defaults and can be configured globally via the `LIFEGUARD_CACHE_*` environment 
variables as well as overidden on a per-query basis on each macro invocation.


---


### ✅ Built-in Prometheus metrics

| Metric                             | Description                         |
|------------------------------------|-------------------------------------|
| `lifeguard_queries_total`          | Total queries executed              |
| `lifeguard_query_duration_seconds` | Histogram for DB execution time     |
| `lifeguard_coroutine_wait_seconds` | Time a coroutine waits for result   |
| `lifeguard_pool_queue_depth`       | Number of coroutines waiting        |


### ✅ Dashboards + Alerts

Grafana dashboards for:
- Real-time query throughput
- Latency p95/p99
- Pool queue depth
- Alerting to Slack / Webhook / Email

---

## 💻 Usage

### 1. Add to your project

```toml
[dependencies]
lifeguard = { path = "./lifeguard" }
```

### 2. Create a connection pool
```rust
use lifeguard::DbPoolManager;
use may::go;

let pool = DbPoolManager::new("postgres://...", 10)?;

go!(move || {
    let result = pool.execute(|db| {
        Box::pin(async move {
            // Use SeaORM
            let rows = MyEntity::find().all(db).await?;
            Ok::<_, DbErr>(rows)
        })
    });
});

```

---

### 🧪 Development & Testing

```bash
just setup             # Start database + apply migrations
just metrics-server    # Expose Prometheus metrics on :9898
just seed-db-heavy n=100000 -- --batch-size=500
just test              # Run integration + unit tests
```

---


#### 🧪 Coverage Summary

| Test Case                                 | Covered |
|--------------------------------------------|---------|
| Config defaults & fallback logic           | ✅       |
| Env overrides                             | ✅       |
| DB PoolManager initialization              | ✅       |
| Execute with failing query                 | ✅       |
| Multiple concurrent queries (non-blocking) | ✅       |


---

### 📊 Observability Stack

```bash
docker compose -f docker-compose.test.yml up -d
```

Then visit:

- [Grafana](http://localhost:3000/) — user: admin / admin
- [Prometheus](http://localhost:9090/)
- [Metrics endpoint](http://localhost:9898/)



#### 🔔 Alerts
Alerts for:

- Queue depth > 50
- p99 latency > 100ms
- Delivered via Slack / Email / Webhooks

See grafana/alerts/lifeguard-alerts.yml



# 🙌 Acknowledgements
- may — green-thread coroutine runtime
- SeaORM — async database ORM
- OpenTelemetry — metrics framework
- Grafana — metrics visualization
- Prometheus — metrics collection
- Loki — logging
- Redis — caching
- Postgres — database
- Tokio — async runtime
- Fang — Out of process task runner

# 📜 License

Apache-2.0 — use freely, contribute openly.

