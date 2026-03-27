What about things like foreign keys, views, and some of the more advanced functionality that Postgres’s provides? We will need this, maybe not in the first iteration but we need to build these into the PRD

---

Absolutely — **foreign keys, views, constraints, indexes, sequences, triggers, materialized views, generated columns, partial indexes, computed expressions, and Postgres-specific features** must be included in the *full system PRD*, even if phased into later implementations.

Below is the **expanded PRD section** covering all advanced PostgreSQL functionality Lifeguard must eventually support — divided into:

1. **Core v1 must-have features**
2. **Postgres Advanced Features (v2/v3 scope)**
3. **ORM surface requirements**
4. **Migration system requirements**
5. **Query builder enhancements**
6. **Introspection & tooling**
7. **Future expansion**

This can be inserted directly into the earlier PRD.

---

# 🔥 **Lifeguard PRD — Advanced PostgreSQL Feature Support**

The following advanced Postgres functionality must be supported in Lifeguard's ORM layer (`MayModel`, `MayRecord`), migration system, and query builder paths.

This defines the **feature-complete end state**, not just v1.

---

# 1. Base PostgreSQL Capabilities (v1 / v1.5)

These must be present early because they’re structural:

## **1.1 Foreign Keys**

* Add FK constraints via migrations.
* Provide ORM annotations:

```rust
#[belongs_to(User, foreign_key = "user_id")]
```

* Provide query helper:

```rust
Comment::find().with(User).all(&pool)?;
```

* FK enforcement is handled by Postgres; ORM only helps with struct-level mapping & relation loading.

---

## **1.2 Indexes**

Support in migrations:

* `CREATE INDEX`
* `UNIQUE INDEX`
* Multi-column indexes
* Partial indexes

Examples:

```rust
Index::create()
    .name("idx_users_email")
    .table(User::TABLE)
    .col(User::Email)
    .unique()
    .to_owned();
```

Partial index:

```rust
Index::create()
    .table(Order::TABLE)
    .col(Order::Archived)
    .condition(Expr::col(Order::Archived).eq(false))
```

---

## **1.3 Composite Primary Keys**

ORM must be capable of deriving:

```rust
#[derive(MayModel)]
#[primary_key(order_id, line_id)]
```

v1 may simply support read-only composite PKs; updates come in v2.

---

## 1.4 Unique Constraints

In migration:

```rust
ColumnDef::new(Order::Reference)
    .string()
    .unique()
```

ORM-level validation remains optional, but DB-level enforcement is mandatory.

---

# 2. Intermediate PostgreSQL Features (v2)

These are essential in production systems.

## **2.1 Views**

### Requirements:

* Migrations must support `CREATE VIEW` and `CREATE OR REPLACE VIEW`.
* Views must be queryable via `MayModel`:

```rust
#[derive(MayModel)]
#[view = "active_users_view"]
struct ActiveUserView { ... }
```

No `MayRecord` on views (read-only).

---

## **2.2 Materialized Views**

### Requirements:

* `CREATE MATERIALIZED VIEW` support in migrations.
* Trigger refresh:

```rust
MaterializedView::refresh("daily_sales_summary", &pool)?;
```

Optional: auto-refresh scheduling via external cron or Postgres triggers.

---

## **2.3 Generated Columns**

Postgres 12+ supports:

```sql
generated always as (expr) stored
```

Migration DSL:

```rust
ColumnDef::new(User::SearchKey)
    .string()
    .generated("lower(email || ' ' || name)")
    .stored()
```

MayModel should treat these as read-only fields.

---

## **2.4 Check Constraints**

Migration support:

```rust
Table::create()
    .col(ColumnDef::new(User::Age).integer())
    .check(Expr::col(User::Age).gte(0))
```

No ORM-level enforcement necessary.

---

## **2.5 Exclusion Constraints**

For scheduling / geospatial systems:

```rust
Constraint::exclude()
    .using("gist")
    .col((Booking::RoomId, "="))
    .col((Booking::TimeRange, "&&"))
```

---

## **2.6 Sequences**

Migration support:

```rust
Sequence::create().name("user_seq").starts_with(1000)
```

ORM support:

```rust
let id = Sequence::nextval("user_seq", &pool)?;
```

---

# 3. Advanced PostgreSQL Features (v3+)

These are powerful, less commonly used, but must be in the architecture.

---

## **3.1 Triggers**

Migration DSL:

```rust
Trigger::create()
    .name("set_updated_at")
    .table(User::TABLE)
    .when(TriggerEvent::BeforeUpdate)
    .function("update_timestamp()")
```

ORM design:

* No built-in trigger generation beyond migration DSL.
* Triggers operate entirely at the DB level.

---

## **3.2 Stored Procedures & Functions**

Migration support:

```rust
Function::create()
    .name("calculate_discount")
    .language("plpgsql")
    .body("BEGIN ... END;")
```

ORM support for calling functions:

```rust
let result = db.call_function("calculate_discount", &[&id, &qty]);
```

---

## **3.3 Full-Text Search**

Migration DSL:

```rust
Index::create()
    .using("gin")
    .col(Expr::cust("to_tsvector('english', body)"))
```

ORM query helper:

```rust
Post::search("rust programming", &pool)?;
```

Under the hood:

```sql
WHERE to_tsvector('english', body) @@ plainto_tsquery($1)
```

---

## **3.4 JSONB Querying**

ORM helper for:

```rust
User::find().filter(User::data().contains(json!({"active": true}))).all(&pool)
```

Migration must support JSONB columns:

```rust
ColumnDef::new(User::Preferences).json_binary()
```

---

## **3.5 Geospatial (PostGIS) support**

v3+ optional.

* Migration support for `geometry` and `geography` types.
* ORM support for:

```rust
Location::find().within_radius(lat, long, 1000)?;
```

---

## **3.6 Partitioning**

Migration DSL:

```
Table::create().partition_by("RANGE (created_at)")
```

ORM: transparent (models do not need special logic).

---

## **3.7 Logical Replication / CDC Hooks**

Out of scope for ORM, but may be required for ETL integrations in the future.

---

# 4. ORM Requirements for Advanced Features

To support advanced PostgreSQL features, MayModel must be extended:

1. **Column attribute metadata**

    * Types
    * Nullable flags
    * Constraints
    * Default values
    * Generated columns

2. **Relations**

    * `has_one`
    * `has_many`
    * `belongs_to`

3. **Relation loading**

    * `.with(User)`
    * `.with_many(Orders)`
    * `.preload(...)`

4. **Computed columns**

    * Should support read-only fields.

5. **View models**

    * Must be queryable like tables.

6. **Materialized views**

    * Must be queryable + refreshable.

---

# 5. Migration Requirements

Migrations must support:

### Table operations:

* Create/alter/drop tables
* Column add/remove/modify
* Composite primary keys
* Constraints (check, unique, exclusion, FK)
* Index creation (btree, gin, gist, hash)
* Partial indexes
* Adding/removing default expressions

### View operations:

* Create view
* Replace view
* Drop view

### Materialized view operations:

* Create
* Refresh
* Drop

### Trigger operations:

* Create trigger
* Drop trigger

### Function/procedure operations:

* Create
* Replace
* Drop

### Foreign keys & relationships:

* Add FK
* Drop FK

### Sequences:

* Create sequence
* Alter sequence
* Drop sequence

### Partitions (v3):

* Create partitions
* Attach/detach partitions

---

# 6. Query Builder Requirements

Enhancements to SeaQuery wrappers:

### Must support:

* joins (inner / left / right / cross)
* group-by / having
* aggregates (sum, avg, count)
* ordering
* windows:

```rust
OVER (PARTITION BY ... ORDER BY ...)
```

* subqueries
* raw SQL fragments when needed

---

# 7. Introspection & Tooling

Future optional tool:

### `lifeguard inspect`

* Inspect Postgres schema
* Generate draft models from tables
* Validate migrations vs actual schema
* Diff detection

This is not v1, but must be in PRD.

---

# 8. Future Expansion (v3+)

* Connection multiplexing with pgbouncer compatibility
* Async mode (if needed) via feature flag
* Sharding support / read replicas
* CDC hooks (Debezium style)
* Integration into BRRTRouter typed handlers

---

# ⚡ Summary: Yes, Lifeguard Must Support All Advanced PG Features

The PRD now includes:

* Foreign keys
* Views & materialized views
* Indexes (incl. partial & gin/gist)
* Generated columns
* Triggers
* Functions / procedures
* JSONB
* Full-text search
* ORM mapping for complex types
* PostGIS (optional)
* Partitioning

All defined in a phased, structured roadmap (v1 → v2 → v3).

---

If you want, I can:

👉 **Insert this into a fully formatted PRD file for canvas**
👉 Or generate **full system architecture diagrams**
👉 Or design the **derive(MayModel) / derive(MayRecord)** macros with examples of generated code.

Just tell me your preferred next step.
