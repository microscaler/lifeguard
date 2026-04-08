# UUID and PostgreSQL scalar types (LifeModel / LifeRecord)

This document is part of the **Lifeguard** public documentation. It exists so applications do not repeat a common **runtime** failure mode: declaring a Rust type that does not match what the PostgreSQL driver decodes for a given SQL column.

## Problem

PostgreSQL stores UUIDs with the `uuid` type. The stack Lifeguard uses for I/O (`**may_postgres`**, `**postgres`**, `**postgres-types**`) deserializes that OID as `**uuid::Uuid**`, not as `**String**`.

If a `LifeModel` field is declared as:

```rust
#[column_type = "UUID"]
pub id: String,
```

then `SELECT` / `FromRow` paths can fail at **runtime** with an error equivalent to:

```text
cannot convert between the Rust type `alloc::string::String` and the Postgres type `uuid`
```

This may surface as empty API responses if application code maps `LifeError` to “no rows” instead of surfacing the error.

`**#[column_type = "UUID"]` describes the SQL column; it does not coerce the Rust field to accept `uuid` as text.** The Rust type must match the driver.

## Required mapping

For a non-null UUID column:

```rust
use uuid::Uuid;

#[derive(Clone, Debug, LifeModel, LifeRecord)]
#[table_name = "example"]
pub struct Example {
    #[primary_key]
    #[column_type = "UUID"]
    pub id: Uuid,
    // ...
}
```

For a nullable UUID column:

```rust
#[column_type = "UUID"]
pub optional_ref: Option<Uuid>,
```

Enable the `**uuid**` crate in your application (with `serde` if you expose IDs in JSON):

```toml
uuid = { version = "1", features = ["v4", "serde"] }
```

`lifeguard` / workspace crates already align `uuid` versions where needed; match your workspace.

## Foreign keys referencing `UUID`

If column `campaign_id` is `UUID` referencing another table, use `**Uuid**`, not `String`, on the model field.

## API boundaries

HTTP handlers and OpenAPI often use `**String**` for path/query parameters. Parse at the boundary:

```rust
let id = Uuid::parse_str(id_str.trim())?;
```

Serialize to strings for JSON with `**.to_string()**` or serde’s `Uuid` support where applicable.

## Derive / codegen notes

`lifeguard-derive` generates `FromRow` and parameter binding that align with the **Rust field type**. The derive recognizes `**uuid::Uuid`** for UUID columns and emits the correct reads/writes (see generated code paths for UUID in `lifeguard-derive`).

Do not rely on “stringly-typed” UUIDs for persisted columns.

## Related reading

- Hauliage postmortem (real incident): [hauliage — postmortem consignments list_jobs](https://github.com/microscaler/hauliage/blob/main/docs/postmortem-consignments-list-jobs-empty-2026-04.md) (path may differ if the repo is vendored; clone: `hauliage/docs/postmortem-consignments-list-jobs-empty-2026-04.md`).
- Planning note (type inference): [ISSUE_UUID_NAIVEDATETIME_TYPE_INFERENCE.md](../planning/lifeguard-derive/ISSUE_UUID_NAIVEDATETIME_TYPE_INFERENCE.md)

## Other scalars

- `**TIMESTAMP WITH TIME ZONE`**: Prefer types that match your driver and Lifeguard bindings (often `chrono::DateTime<Utc>` or documented `NaiveDateTime` patterns). Mismatches also fail at **deserialize** time.
- When in doubt, check `**postgres-types`** / `**tokio-postgres`** compatibility for the Rust type you choose.

