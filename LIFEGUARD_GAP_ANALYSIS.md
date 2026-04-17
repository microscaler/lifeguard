# Lifeguard ORM Gap Analysis Audit
**Target Analysis**: Feature Parity Assessment vs. `SeaORM 2.0`
**Ecosystem**: Rust Coroutine Architecture (`may` + `may_postgres`)

## Executive Summary
Lifeguard was engineered out of pure architectural necessity: the standard Rust backend framework ecosystem strictly couples ORMs to `Tokio`/`async-std`, which inherently clash with symmetrical `may` coroutines. By constructing Lifeguard from scratch and wrapping `sea-query`, the system successfully bridges asynchronous database connections on the `may` runtime.

However, when pitched against the highly mature [SeaORM 2.0](https://www.sea-ql.org/SeaORM), several quality-of-life, infrastructure, and advanced relational abstractions are missing. To propel Lifeguard to a world-leading standard for the Rust coroutine ecosystem, the following feature gaps must be bridged.

---

## 1. Ergonomic Nested Object Graph Persistence
**The Gap:** 
SeaORM 2.0 introduced fluent *Nested ActiveModels*, allowing developers to transactionally persist deeply nested data hierarchies (parent and children) in a single operation:
```rust
// SeaORM
user::ActiveModel::builder()
    .set_name("Bob")
    .set_profile(profile::ActiveModel::builder().set_picture("image.jpg"))
    .add_post(post::ActiveModel::builder().set_title("Nice weather"))
    .save(db).await?;
```
**Lifeguard State:** 
`LifeRecord` (the `ActiveModel` equivalent) is strictly **one-dimensional**. While the system intercepts lifecycles cleanly (via `before_insert`/`after_update` hooks), persisting a user with children requires manual tracking of IDs across multiple sequential database `.save()` calls manually wrapped inside a transaction.

**Action Required:** Implement recursive graph validation and transactional resolution inside `LifeRecord`.
* **Proposed Implementation:** 
  1. Extend the `lifeguard-derive` macro for `LifeRecord` to parse relationship attributes (`#[has_many]`, `#[belongs_to]`).
  2. Generate fluent setter methods (`add_related`, `set_related`) that store child `LifeRecord` instances in an internal `Vec` mapped to relation variants.
  3. Introduce a `save_graph(&self, db)` function that automatically spawns a `db.transaction(|tx| ...)`. It will topologically sort constraints: inserting `belongs_to` parents first, then the primary record, then propagating the generated `primary_key` down to `has_many` children before inserting them sequentially within the `may_postgres` transaction block.
* **Hauliage Platform Impact:** Critical for the **`consignments`** and **`bidding`** microservices. Complex creations (like persisting a `Consignment` alongside its nested `Location`, `Company`, and `Item` hierarchies) currently require risky, manually orchestrated sequential saves.
* **Roll-out Story:** A backend engineer handles the `/consignments` POST route. They initialize `ConsignmentRecord::from_model(...).add_item(ItemRecord...).set_receiver(LocationRecord...)` and conclude with `.save_graph(db)`. They delete 60 lines of brittle transactional mapping code, and the integration checks pass instantly.

---

## 2. Advanced `DataLoader` Abstractions (N+1 Avoidance)
**The Gap:**
SeaORM elegantly prevents the N+1 phenomenon by natively wiring a chunked `DataLoader` under the hood. Complex hierarchical chains (e.g., loading Users ➔ their Posts ➔ those Posts' Tags) can natively traverse object boundaries effortlessly with `.with()`.
**Lifeguard State:**
Lifeguard currently relies on manual explicit eager fetchers (`lifeguard::relation::eager::load_related`) utilizing `selectinload` grouping heuristics mapped manually to hash maps. It does not automatically hydrate deep `.find_also_related()` hierarchies returned as contiguous struct models transparently.

**Action Required:** Upgrade `SelectQuery` to seamlessly chain relation loads directly via `.with()`.
* **Proposed Implementation:**
  1. Introduce a `Loader` trait and registry capable of batching primary keys from the parent result set. 
  2. Modify `SelectQuery` builder to accept `.with(Relation::Author)` chains, storing target relations in the execution pipeline. 
  3. Post-fetch, invoke `lifeguard::relation::eager::load_related` invisibly over the entire batch.
  4. Hydrate the struct directly by expanding `ModelTrait` to contain dynamically sized relationship stubs (`Option<Vec<ChildModel>>`), mapping the raw HashMaps automatically back into the nested fields before yielding `Vec<LoadedModel>`.
* **Hauliage Platform Impact:** Essential for the **`bff`** (Backend for Frontend) aggregation layer. Automatically resolving N+1 queries when fetching lists of Bids and their associated Carrier details simultaneously will drastically reduce load times on the shipper dashboard.
* **Roll-out Story:** The BFF team updates `get_organization_dashboard`. By simply appending `.with(Relation::Carrier)` to their base `Bid::find()` builder, the dashboard load time drops from 800ms to under 45ms. Zero manual `HashMap` grouping is required.

---

## 3. Reverse Engineering Tooling (`sea-orm-cli`)
**The Gap:**
SeaORM's `sea-orm-cli` drastically reduces boilerplate by connecting to a live database and generating the canonical Entity traits automatically (`--entity-format dense`), cutting hours off onboarding times.
**Lifeguard State:**
Lifeguard lacks any scaffolding utility. `LifeModel` macros drastically reduce the boilerplate needed to compose raw mapping properties, but structural changes to a database demand tedious manual translations into corresponding struct `Option<T>` fields.

**Action Required:** Introduce `lifeguard-cli generate` to scaffold entities automatically.
* **Proposed Implementation:**
  1. Create a distinct `lifeguard-cli` crate that runs standalone.
  2. Connect to the specified Postgres URI and query `information_schema.columns` and `pg_constraint`.
  3. Map Postgres `oid` data types (e.g., `VARCHAR`, `UUID`, `JSONB`) dynamically to Rust scalar equivalents using a registry.
  4. Write out `.rs` files containing perfectly formatted `#[derive(LifeModel, LifeRecord)]` structs, fully annotating primary keys and auto-increment defaults.
* **Hauliage Platform Impact:** Accelerates **New Microservice Onboarding**. As the Hauliage infrastructure scales (e.g., separating `identity` and `company` out from monolithic schemas), reverse-engineering the production Postgres schemas into Lifeguard entity structs instantly removes manual boilerplate translation.
* **Roll-out Story:** An engineer is assigned to decouple the `billing` microservice. They launch `lifeguard-cli generate --uri postgres://... --schema billing`. Instantly, 15 complete `#[derive(LifeModel)]` structs are generated in `src/models/`, allowing them to start wiring BRRT routes immediately.

---

## 4. Entity First Workflows & Schema DDL Extractor
**The Gap:**
SeaORM 2.0 utilizes `sea-schema` to diff Rust Entity definitions against live databases to output exact `CREATE TABLE` and `ALTER TABLE` instructions programmatically.
**Lifeguard State:**
Lifeguard implements an excellent state engine for migration status tracking (`MigrationStatus`/`MigrationLockGuard`); however, structural SQL (adding columns, dropping indices) is hand-written sequentially in raw SQL rather than abstract representations.

**Action Required:** Implement structural `sea-schema` DDL diffing directly against `LifeModelTrait`.
* **Proposed Implementation:**
  1. Expand the `lifeguard-derive` macro to emit a programmatic `sea_query::Table::create()` definition dynamically from struct and attribute parsing (`#[column(type = "VARCHAR")]`). 
  2. Create a generic `SchemaManager` service that calculates the diff between the `sea_query::Table` declarations in Code, against the live metadata present in the connected Postgres database using `sea-schema`.
  3. Output deterministic DDL (`CREATE TABLE`, `ALTER TABLE ADD COLUMN`) and execute it programmatically on application boot, effectively removing the need for hand-written `up()` and `down()` SQL strings in the `migrations/` API.
* **Hauliage Platform Impact:** High value for **rapid-iteration services** like **`inbox`** and **`notifications`**. Eliminating manual SQL migration scripts removes human error and guarantees the database exactly matches the rust codebase throughout testing, staging, and production deployments.
* **Roll-out Story:** The `inbox` team needs a new `read_receipt` column. The developer merely adds `pub read_receipt: Option<bool>` to their `MessageModel` struct. On Tilt deployment, the DB synchronizer issues `ALTER TABLE messages ADD COLUMN read_receipt BOOLEAN` automatically. No SQL scripts are written.

---

## 4.1 Row-Level Security Entity Enforcement
**The Gap:**
PostgreSQL supports powerful row-level security policies (`ENABLE ROW LEVEL SECURITY`) to enforce multi-tenant isolation at the database core (preventing queries from extracting data belonging to other organizations or drivers). Currently, traditional ORMs treat this as out-of-scope, requiring messy, raw SQL overrides.
**Lifeguard State:**
Because Lifeguard sits directly on top of Hauliage's microservice infrastructure, it can explicitly encode and generate Policy logic.

**Action Required:** Parse declarative `RLS` logic from `Lifemodel` annotations seamlessly executing them inside Lifeguard migrations.
* **Proposed Implementation:**
  1. Add `#[enable_rls]` and `#[rls_policy(name = "x", command = "SELECT", using = "org_id = current_setting('app.tenant_id')::uuid")]` decorators inside the `lifeguard-derive` macro map.
  2. Emit native `sea_query` or raw PostgreSQL execution blocks during the `SchemaManager` structural diffing (detailed in Gap 4). If a Model declares an RLS policy, Lifeguard natively executes the `CREATE POLICY` DDL exactly like it creates standard tables/indexes.
  3. Standardize a session injection step inside `MayPostgresExecutor` (e.g. `SET LOCAL app.tenant_id = '...'`) automatically pulled from contextual BFF request signatures, preventing cross-tenant data leaks natively mapping security layers down to the core engine.

---

## 5. Cursor Pagination & Channels Streaming
**The Gap:**
Modern high-performance applications process data streams dynamically. SeaORM supports index-driven Cursor Pagination alongside Offset limitations.
**Lifeguard State:**
Lifeguard supports `.paginate()` and built-in optimized `.paginate_and_count()` using traditional `OFFSET` methodology (which degrades at massive scale) and operates strictly on arrays (`Vec<E>`). Given that Lifeguard operates on `may`, providing synchronous streaming bridges (yielding values directly through Coroutine `std::sync::mpsc::Receiver` endpoints) could prove substantially more performant.

**Action Required:** Implement Native Cursor Pagination and native `may` channel streaming.
* **Proposed Implementation:**
  1. **Cursors**: Add an `.order_by_cursor(column)` modifier to `SelectQuery` that generates composite `WHERE (col, id) > (last_col, last_id)` rules dynamically based on the last yielded row, ensuring `O(1)` index lookups regardless of row depth.
  2. **Streaming**: Expose a `.stream_all(db)` method that opens a `may_postgres::Transaction` with a server-side Postgres `DECLARE CURSOR`.
  3. Connect the Postgres cursor directly into a `may::sync::mpsc::channel`, looping over `FETCH FORWARD 500` and immediately feeding instances to the worker coroutine receiver, achieving minimal-memory constant-flow streaming.
* **Hauliage Platform Impact:** Crucial for the **`analytics`** microservice. Exporting huge datasets (like historical Freight Spend Reports) via `.stream_all(db)` avoids overwhelming Kubernetes pod memory limits (preventing OOMKills) compared to `OFFSET`/`Vec<T>` buffering.
* **Roll-out Story:** Operations requests a massive multi-million row CSV export from `analytics`. Using `.stream_all(db)` chained to an HTTP chunked response body, the application streams 4GB of data perfectly to the client while the pod memory ceiling never breaches ~40MB.

---

## 6. Supported Ecosystem Bridges

**Platform note (supersedes the “universal GraphQL for BFF” narrative below):** Hauliage BFF and dashboard composition are **OpenAPI-first, BRRTRouter-generated**, with **REST-shaped view endpoints** — not a GraphQL runtime. A GraphQL stack does not align with that model. The optional **`graphql`** / **`async-graphql`** integration in this repo is **legacy / frozen** from a product perspective; do not treat it as the direction of travel for the platform. See **`docs/llmwiki/topics/graphql-optional-feature.md`**.

**The Gap (vs SeaORM OSS):**
SeaORM extends beyond standard data fetching with `Seaography` for instant GraphQL schema translation, and `SeaORM Pro` for dashboard templating.
**Lifeguard State:**
Lifeguard meshes tightly with internal architectures (like BRRT routing validation), but it currently operates asynchronously to the external OSS landscape (like `juniper` or GraphQL APIs). An optional **`graphql`** feature flag exists: when enabled, `LifeModel` can emit **`async-graphql::SimpleObject`** on generated entities (`lifeguard-derive`), primarily to support existing tests and narrow integrations — **not** a mandate to expose GraphQL from Hauliage services.

**Historical “Action Required” (not pursued for Hauliage BFF):** ~~Provide macro extensions for universal GraphQL integration.~~ **Current stance:** no expansion of GraphQL as the BFF or multi-service dashboard API; BFF composition is handled via composed REST/OpenAPI views instead.

* **What was proposed (archived):**
  1. Selective feature flags (e.g. `#[cfg(feature = "graphql")]`) within `lifeguard-derive` — **done** for `SimpleObject` emission.
  2. Automatic `async-graphql::SimpleObject` on generated entities — **partially present**; not a platform rollout target for dashboards.
* **Hauliage Platform Impact (superseded):** ~~GraphQL for the `bff`…~~ **Replaced by:** typed downstream calls and BFF view endpoints per PRD (REST-shaped, spec-driven).
* **Roll-out Story (superseded):** Client-specific field selection via GraphQL on the BFF — **not** the chosen model; mobile/web clients consume OpenAPI-described view routes instead.

## Foundation work aligned with this audit (not gap closure)

The following changes **do not** close the SeaORM parity gaps above; they keep the **current** ORM honest and testable so future work on those gaps builds on correct primitives:

| Area in this document | How recent work aligns |
|-------------------------|-------------------------|
| **§2 DataLoader / relations** | `find_related` and `LazyLoader` now build `WHERE` clauses on the **related** table (`to_tbl` / `to_col`) with values from the source row (`from_col`, via `get_by_column_name` with a primary-key name fallback). That matches how batched / eager loaders must filter child rows and avoids invalid SQL when the parent row is not in the query’s `FROM` list. |
| **§5 Cursor / streaming** | Integration coverage for `stream_and_cursor` lives in the same single binary as other DB tests, sharing one Postgres (and Redis URL in context) per process—appropriate for exercising streaming without spinning containers per test module. |
| **§1 Nested graph (indirect)** | `active_model_graph` tests share the same harness; graph persistence remains a **future** item—this only ensures the graph-oriented tests run under the same lifecycle as the rest of the suite. |
| **Test infrastructure** | One integration binary (`db_integration_suite`) + `tests/context.rs`: one pair of Postgres/Redis (or env URLs) per **process**, `ctor::dtor` cleanup for testcontainers—reduces container churn and matches how CI/local runs should stress the library, without implementing §3 CLI or §4 schema diffing. |

When adding `.with()`-style loaders (§2) or nested `save_graph` (§1), relation definitions and `build_where_condition` semantics above should stay the single source of truth for “filter related rows by this parent instance.”

**Continuation & robustness (tabulated):** see [docs/planning/audits/LIFEGUARD_FOUNDATION_CONTINUATION.md](docs/planning/audits/LIFEGUARD_FOUNDATION_CONTINUATION.md) for next implementation steps, risk hardening, and CI verification.

## Conclusion
Lifeguard commands a distinct niche inside the `may` ecosystem with a heavily optimized, memory-safe footprint utilizing distributed query caching/Redlock architectures. By tackling the **Nested Object Persistence** and **Entity First Workflows**, it can fully cross the precipice into a functionally identical, syntactically superior alternative to Tokio-bound equivalents.
