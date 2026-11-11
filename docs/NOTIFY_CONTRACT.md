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

## Statement granularity: `ids`

```rust
#[notify(channel = "order_changes", on = "insert,update,delete", granularity = "statement")]
```

`granularity = "statement"` generates `FOR EACH STATEMENT` triggers with transition tables (one
per operation) instead of a row trigger. Each statement publishes the distinct keys it touched in
an array:

```json
{"table": "orders", "op": "UPDATE", "ids": [41, 42, 97], "v": 23861592, "ts": 1793119890340}
```

| Field | Meaning |
|---|---|
| `ids` | the distinct primary keys the statement touched: new rows for `INSERT`, old rows for `DELETE`, old **and** new rows for `UPDATE` (so a key change invalidates both) |
| `v`, `ts` | as above, taken once per statement |

- A statement touching many rows is split into several notifications, each with part of the keys,
  so every payload stays under 8000 bytes. The split is by key text length (7000 bytes of keys per
  payload: about 190 UUIDs or 500 to 1000 integer keys).
- A statement that touched no rows sends nothing.
- A payload carries either `id` or `ids`, never both. A consumer handles `ids` as one change per
  element, with the same `table`, `op`, `v` and `ts`.
- Use it for tables written in bulk (multi-row `INSERT`, `UPDATE .. FROM`, batch upserts): a
  500-row statement sends one to three notifications instead of 500. For single-row writes the
  two granularities cost the same.

## Timing

A notification is queued by the trigger and delivered when the writing transaction commits, so
invalidation follows the **commit**, not the statement:

- Rows written one per transaction are invalidated one by one, each at its own commit.
- Rows written in one batched statement or one transaction are invalidated together, at that
  commit. Batching never delays a key past the commit that made it visible; it can delay the
  first key of a batch by the time it took to build the batch.
- Within one transaction PostgreSQL drops a notification identical to one already queued (same
  channel and payload), so repeated writes to one row in one transaction collapse.


## Delivery

PostgreSQL delivers a notification after the writing transaction commits, to sessions that were
listening at that moment, in commit order. Nothing is delivered for rolled-back writes. A consumer
that was not connected misses notifications: it must catch up from another source (for example a
change-timestamp column) after any gap.

## Compatibility rules

- Version 1 consumers must ignore fields they do not know.
- New fields may be added without changing the version. Removing or changing the meaning of a
  field changes the version.
- `ids` (statement granularity) is an addition under the same version: a consumer that reads
  only `id` must be upgraded before a table switches to statement granularity.
- `v` and `ts` were added in version 1; triggers generated before them send only `table`, `op`
  and `id`, which consumers should accept (losing stale-event detection and lag measurement).
