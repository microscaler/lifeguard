Below is the **new, corrected, complete `README.md`** for the Lifeguard project.
This README is aligned with the final PRD and suitable for immediate inclusion in the GitHub repo as the new canonical documentation.

---

# **Lifeguard**

### *Coroutine-native Postgres Data Platform for Rust*

Lifeguard is a **high-performance, coroutine-native database layer** designed for Rust services built on the [`may`](https://github.com/Xudong-Huang/may) runtime.

It provides:

* A **deterministic, persistent Postgres connection pool**
* A lightweight ORM: **MayModel** and **MayRecord**
* SQL building powered by **SeaQuery**
* A robust **migration system**
* Deep **observability** (Prometheus + OpenTelemetry)
* Support for core → advanced PostgreSQL features

Lifeguard is designed to power **high-throughput microservices**, **API routers**, and **database-heavy systems** with predictable latency and zero async runtime overhead.

---

# **Why Lifeguard?**

Traditional Rust database libraries (sqlx, SeaORM, Diesel):

* rely heavily on **async**, **Tokio**, and **Send/Sync** constraints
* assume a **future-polling execution model**
* are not compatible with **coroutine runtimes** like `may`
* create complexity and performance overhead in low-latency systems

**Lifeguard takes a different approach.**

It is:

### ✔ Coroutine-first

### ✔ Postgres-focused

### ✔ Built for extreme throughput

### ✔ Designed for predictable performance

### ✔ Fully synchronous internally

If you are building microservices or routers using `may`, Axum-with-may, or BRRTRouter—
Lifeguard is the database foundation you actually need.

---

# **Features**

### **✓ Persistent Postgres Connection Pool**

* Pre-allocates `N` long-lived connections
* Reuses them aggressively
* Avoids DB churn & connection storms
* Ensures bounded concurrency via semaphore
* Auto-recovers from bad connections

### **✓ ORM-lite Layer**

* `#[derive(MayModel)]` for reading data
* `#[derive(MayRecord)]` for inserts/updates
* CRUD helpers
* SeaQuery-backed SQL generation

### **✓ Safe, Simple Migrations**

* Lifeguard-native migration trait
* Safe up/down versioning
* Supports full Postgres schema operations:

    * tables, indexes, FKs, views, sequences, JSONB, etc.

### **✓ Metrics & Instrumentation**

* Connection pool gauges
* Query latency histograms
* Error counters
* Optional OTel spans and metrics

### **✓ PostgreSQL Power Features**

Supported now or coming soon:

| Feature               | Status |
| --------------------- | ------ |
| Foreign Keys          | ✓ v1   |
| Views                 | ✓ v1   |
| JSONB                 | ✓ v1   |
| Partial Indexes       | ✓ v1   |
| Materialized Views    | v2     |
| Generated Columns     | v2     |
| Exclusion Constraints | v2     |
| Full-text Search      | v2     |
| Window Functions      | v2     |
| PostGIS               | v3     |
| Partitioning          | v3     |
| Triggers & Procedures | v3     |

---

# **Quick Start**

## **1. Add Lifeguard**

```toml
[dependencies]
lifeguard = { git = "https://github.com/microscaler/lifeguard" }
```

## **2. Configure your database**

```toml
# config.toml
[database]
url = "postgres://app:pass@localhost/mydb"
max_connections = 32
min_connections = 8
```

Load config:

```rust
let config = LifeguardConfig::load()?;
let pool = LifeguardPool::new(config)?;
```

---

# **3. Define a Model**

```rust
#[derive(MayModel)]
#[table = "users"]
struct User {
    #[primary_key]
    id: i64,
    email: String,
    is_active: bool,
}
```

Query:

```rust
let user = User::find_by_id(&pool, 1)?;
let users = User::find().filter(User::Email.eq("test")).all(&pool)?;
```

---

# **4. Insert or Update Records**

```rust
#[derive(MayRecord)]
#[table = "users"]
struct NewUser {
    email: String,
}

let created = NewUser { email: "first@demo.com".into() }.insert(&pool)?;
```

Update existing rows:

```rust
let updated = created
    .to_record()
    .set_email("new@demo.com")
    .update(&pool)?;
```

---

# **5. Migrations**

Create a migration:

```rust
pub struct CreateUsers;

impl Migration for CreateUsers {
    fn up(&self, exec: &impl Executor) -> Result<()> {
        let stmt = Table::create()
            .table(Alias::new("users"))
            .if_not_exists()
            .col(ColumnDef::new(Alias::new("id")).big_integer().not_null().primary_key())
            .col(ColumnDef::new(Alias::new("email")).string().not_null())
            .to_owned();

        exec.execute(&stmt.to_string(PostgresQueryBuilder), &[])?;
        Ok(())
    }

    fn down(&self, exec: &impl Executor) -> Result<()> {
        exec.execute("DROP TABLE IF EXISTS users", &[])?;
        Ok(())
    }

    fn name(&self) -> &'static str {
        "20250101_create_users"
    }
}
```

Run migration:

```
lifeguard migrate up
```

---

# **6. Architecture Summary**

```
LifeguardPool  → Persistent connection slots
MayExecutor    → Direct may_postgres I/O
MayModel       → Select/query
MayRecord      → Insert/update/delete
SeaQuery       → SQL builder
Migrations     → Schema evolution
Metrics        → Prometheus + OTel
Postgres       → The engine powering it all
```

Everything above `may_postgres` is fully synchronous and coroutine-native.

---

# **Roadmap**

### **v1.0 (Current Work)**

* Persistent pool + executor
* MayModel / MayRecord
* CRUD operations
* Migration system
* Metrics + Testkit
* Support: FKs, views, indexes, JSONB

### **v2.0**

* Relation loading
* Materialized views
* Sequences API
* Generated columns
* Full-text search
* Window functions

### **v3.0**

* PostGIS
* Partitioning
* Triggers & stored procedures
* Schema inspector
* Codegen tools

---

# **Philosophy**

Lifeguard is not trying to be SeaORM or Diesel.
It is built for:

* **Deterministic performance**
* **High throughput**
* **Coroutine style concurrency**
* **Deep Postgres feature support**

If you want async, choose sqlx.
If you want “ActiveRecord”, choose SeaORM.
If you want **raw power with coroutine speed**, choose **Lifeguard**.

---

# **Contributing**

PRs are welcome once the v1 API is stabilized.

Key areas for contribution:

* ORM macro improvements
* Migration DSL extensions
* Postgres feature support
* Metrics exporters
* Testkit enhancements

---

# **License**

MIT.

---

# **Next Steps**

If you'd like, I can now generate:

### ✓ `CONTRIBUTING.md`

### ✓ `/src/` scaffolding code

### ✓ Architecture diagrams (SVG + ASCII)

### ✓ Examples directory

### ✓ Template migrations

### ✓ Issue templates + GitHub Actions CI config

Just tell me what you want next.
