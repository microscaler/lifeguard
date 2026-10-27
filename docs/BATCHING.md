# Batching

Single-row ORM calls cost one statement, and so one round trip, per row. `lifeguard::batch`
writes and reads many rows per statement and keeps the ORM's behaviour: hooks and validators run
per record, `save_as` expressions apply, RLS session context and tier routing come from the
executor, and rows map through the entity's `FromRow`.

| Call | SQL |
|---|---|
| `insert_many(&exec, &records, &opts)` | `INSERT .. VALUES (..), (..)`, or `INSERT .. SELECT * FROM unnest($1::int8[], ..)` for large chunks |
| `upsert_many(&exec, &records, on_conflict, &opts)` | the same plus `ON CONFLICT ..` |
| `update_many(&exec, &records, &[cols], &opts)` | `UPDATE t SET c = v.c FROM unnest(..) AS v(..) WHERE t.pk = v.pk` |
| `delete_many::<E>(&exec, &ids, &opts)` | `DELETE FROM t WHERE pk = ANY($1)` |
| `find_by_ids::<E>(&exec, &ids, &opts)` | `SELECT .. FROM t WHERE pk = ANY($1)` |
| `BatchWriter` | a buffer that coroutines push into; one flusher writes with the calls above |

Measured numbers (single-row loops vs these calls vs raw `COPY`) are in
[`docs/perf/lifeguard-baseline.md` §6.3](perf/lifeguard-baseline.md#63-batching).

## Writing many rows

```rust
use lifeguard::{insert_many, upsert_many, BatchOptions, OnConflict};

let records: Vec<ReadingRecord> = samples.iter().map(ReadingRecord::from_sample).collect();

// Plain insert, no rows back.
let out = insert_many(&executor, &records, &BatchOptions::default())?;
assert_eq!(out.rows_affected, records.len() as u64);

// Idempotent ingest: last write wins on the natural key, models returned.
let out = upsert_many(
    &executor,
    &records,
    OnConflict::columns([reading::Column::SensorId, reading::Column::TakenAt])
        .update_columns([reading::Column::Value])
        .to_owned(),
    &BatchOptions::returning(),
)?;
for model in out.models { /* .. */ }
```

All records in one call must set the same columns (one statement has one column list). Within
one `upsert_many` call a key must not repeat: PostgreSQL refuses to update the same row twice in
one statement. De-duplicate first, or use `BatchWriter`, which does.

### Options

| Field | Default | Meaning |
|---|---|---|
| `max_rows` | 1000 | rows per statement |
| `max_params` | 32,000 | bind parameters per statement (PostgreSQL's hard limit is 65,535) |
| `unnest_min_rows` | 64 | chunks at least this large use `unnest` arrays |
| `returning` | false | add `RETURNING *` and map the rows into `out.models` |

`VALUES` uses one parameter per value, so a 10-column entity fits 1000 rows in 10,000 parameters.
`unnest` uses one array parameter per column whatever the row count, so its SQL is the same for
every chunk size and is prepared once per connection. Full chunks of the `VALUES` form also repeat
the same SQL, so either way the statement cache serves every chunk after the first.

### Errors and atomicity

Chunks run in order, each as its own statement. A failure returns `BatchError`:

```rust
match insert_many(&executor, &records, &opts) {
    Ok(out) => { /* out.rows_affected, out.statements */ }
    Err(e) => {
        // e.chunk: index of the failing chunk; e.rows: its range in `records`;
        // e.rows_written_before: rows committed by earlier chunks; e.source: the cause.
        retry_later(&records[e.rows.start..]);
    }
}
```

To make a whole batch atomic, run it in a transaction:

```rust
pool.with_session_transaction(&session, |tx| insert_many(tx, &records, &opts).map_err(Into::into))?;
```

### Updates and deletes

```rust
// Every record carries its primary key and the columns being changed.
let changes: Vec<_> = ids.iter().map(|id| {
    let mut r = AccountRecord::new();
    r.set_id(*id).set_status("closed".into());
    r
}).collect();
update_many(&executor, &changes, &[account::Column::Status], &BatchOptions::default())?;

delete_many::<account::Entity>(&executor, &stale_ids, &BatchOptions::default())?;
```

`update_many` runs each record's `before_update` hook and validators. `delete_many` deletes by
key directly, so per-record `before_delete` hooks do not run.

## Reading by key: removing N+1 queries

```rust
// Before: one query per order line.
for line in &lines {
    let product = product::Entity::find().filter(product::Column::Id.eq(line.product_id)).find_one(&exec)?;
}

// After: one query per 1000 keys.
let ids: Vec<Value> = lines.iter().map(|l| l.product_id.into()).collect();
let products: HashMap<i64, ProductModel> = find_by_ids::<product::Entity>(&exec, &ids, &BatchOptions::default())?
    .into_iter()
    .map(|p| (p.id, p))
    .collect();
```

Rows come back in database order; missing keys are absent. The entity's projection and
soft-delete filter apply as for `find()`.

## `BatchWriter`: ingest loops

Code that receives rows one at a time (a feed, a queue consumer, a scraper) can push them into a
`BatchWriter` instead of writing each:

```rust
use lifeguard::{BatchWriter, BatchWriterConfig, WriteMode, OnConflict};

let writer: BatchWriter<QuoteRecord> = BatchWriter::new(
    PooledLifeExecutor::new(pool.clone()),
    WriteMode::Upsert {
        on_conflict: OnConflict::columns([quote::Column::Symbol, quote::Column::Ts])
            .update_columns([quote::Column::Price])
            .to_owned(),
        key: vec![quote::Column::Symbol, quote::Column::Ts],
    },
    BatchWriterConfig::default(), // 500 rows or 50 ms, 10,000 rows of buffer
    |err, rows| log::error!("dead letter: {err} ({} rows)", rows.len()),
);

for msg in feed {
    writer.push(QuoteRecord::from(msg)).ok();   // parks when the buffer is full
}
let stats = writer.close();                      // flushes, then stops
```

Semantics:

- **Flush triggers:** `max_rows` rows buffered, or `max_delay` since the oldest buffered row,
  whichever comes first. `flush()` forces one and waits for it.
- **Backpressure:** at most `capacity` rows wait; `push` parks its caller until there is room.
- **Ordering:** one flusher writes batches in push order, so for a given key a later push is
  never overwritten by an earlier one. In `Upsert` mode a batch keeps only the last row per key
  (`stats.superseded` counts the dropped ones).
- **Delivery:** at-least-once. A failed batch is retried `retries` times with exponential
  backoff, then handed to the error handler. A retried batch may already be partly written (an
  earlier chunk succeeded), so for exactly-once results use `Upsert` on a natural key: replays
  converge to the same rows.
- **Shutdown:** `close()` flushes everything and returns the final counters. Dropping an open
  writer closes it with a bounded wait; call `close()` explicitly so failures are seen.

## RLS

The batch calls use the executor they are given. With a `PooledLifeExecutor` carrying a session
context, each chunk runs inside `BEGIN; rls_set_session(..); <statement>; COMMIT` like any other
statement, so policies (`USING` and `WITH CHECK`) apply to every row.

## When to use `COPY` instead

`COPY FROM STDIN` is the fastest way to load rows, but it bypasses the ORM entirely (no hooks,
no validators, no `ON CONFLICT`, no `RETURNING`) and needs a dedicated connection. For bulk loads
into staging tables it remains the right tool; for application writes, `insert_many` /
`upsert_many` get most of the way there (see the benchmark table) without giving those up.
