# `#[notify]` payload contract

Version **1**. This is the open contract between Lifeguard's generated change triggers and any
consumer that `LISTEN`s to them (a cache-invalidation service, a queue worker, an audit tap).
[LifeReflector](https://github.com/microscaler/lifeguard-reflector), the Redis cache-coherence
service, is one such consumer; it depends on this document, not on Lifeguard's internals.

## Producing it

```rust
#[derive(LifeModel, LifeRecord)]
#[table_name = "orders"]
#[notify(channel = "order_changes", on = "insert,update,delete")]
pub struct Order {
    #[primary_key]
    pub id: i64,
    // ...
}
```

`lifeguard-migrate` generates the trigger function and an `AFTER ... FOR EACH ROW` trigger that
calls `pg_notify(channel, payload)`. Single-column primary keys only.

## The payload

A JSON object, sent as the NOTIFY payload text:

```json
{"table": "orders", "op": "UPDATE", "id": 42, "v": 23861592, "ts": 1793119890340}
```

| Field | Type | Meaning |
|---|---|---|
| `table` | string | `TG_TABLE_NAME`: the table, without schema |
| `op` | string | `INSERT`, `UPDATE` or `DELETE` (`TG_OP`) |
| `id` | number or string | the row's primary key, as JSON renders its type (numbers for integer keys, strings for UUID/text keys); for `DELETE` the old row's key |
| `v` | integer | the WAL insert position when the trigger ran (`pg_current_wal_insert_lsn() - '0/0'`). Monotonic **per key**: writes to one row are serialised by its row lock, so a later write's `v` is larger. Not a commit order across keys. |
| `ts` | integer | the trigger's wall-clock time, milliseconds since the Unix epoch (`clock_timestamp()`), for lag measurement and catch-up watermarks |

The payload carries the key, never the row, so it stays far below PostgreSQL's 8000-byte NOTIFY
limit whatever the row's width (a full-row payload makes the writing statement fail on wide
rows).

## Delivery

PostgreSQL delivers a notification after the writing transaction commits, to sessions that were
listening at that moment, in commit order. Nothing is delivered for rolled-back writes. A consumer
that was not connected misses notifications: it must catch up from another source (for example a
change-timestamp column) after any gap.

## Compatibility rules

- Version 1 consumers must ignore fields they do not know.
- New fields may be added without changing the version. Removing or changing the meaning of a
  field changes the version.
- `v` and `ts` were added in version 1; triggers generated before them send only `table`, `op`
  and `id`, which consumers should accept (losing stale-event detection and lag measurement).
