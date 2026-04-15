# Chrono and PostgreSQL date/time types (LifeModel / LifeRecord)

This document complements [`UUID_AND_POSTGRES_TYPES.md`](./UUID_AND_POSTGRES_TYPES.md) for **time** columns. It summarizes the **canonical** mapping used by Lifeguard derive, `sea_query::Value`, and [`src/query/converted_params.rs`](../src/query/converted_params.rs) (parameter binding).

## Canonical mapping

| PostgreSQL | Rust type (typical) | `sea_query::Value` variant |
| ---------- | -------------------- | --------------------------- |
| `timestamp without time zone` | `chrono::NaiveDateTime` | `ChronoDateTime` |
| `timestamp with time zone` / `timestamptz` | `chrono::DateTime<chrono::Utc>` | `ChronoDateTimeUtc` |
| `date` | `chrono::NaiveDate` | `ChronoDate` |
| (optional) local wall clock | `chrono::DateTime<chrono::Local>` | `ChronoDateTimeLocal` |

**Nullable columns:** use `Option<T>`; SQL NULL is represented as typed null variants, e.g. `ChronoDateTimeUtc(None)`, not `String(None)` for a `timestamptz` column.

## Runtime binding (`converted_params`)

[`with_converted_value_slice`](../src/query/converted_params.rs) converts a slice of `Value` into `&[&dyn ToSql]` in **two passes** while preserving order. Typed NULLs for UUID and chrono **must** use dedicated `Option<T>` buckets so PostgreSQL receives the correct OID / `ToSql` implementation (see module docs on that file).

## Derive and migrations

- Implementation tracker: [`COMPLETE_CHRONO_TASKS.md`](./COMPLETE_CHRONO_TASKS.md), PRD [`COMPLETE_CHRONO_IMPLEMENTATION.md`](./COMPLETE_CHRONO_IMPLEMENTATION.md).
- `lifeguard-migrate` infers `timestamptz` → `DateTime<Utc>`; keep derive and migrate versions aligned when upgrading.

## Related reading

- [`UUID_AND_POSTGRES_TYPES.md`](./UUID_AND_POSTGRES_TYPES.md)
