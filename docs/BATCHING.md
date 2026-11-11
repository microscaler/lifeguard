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

## Which tool

Every statement costs at least one network round trip; with the database a millisecond or two
away, round trips dominate most request paths.

| You have | Use | Round trips |
|---|---|---|
| several independent statements in one request (reads or writes) | [`StatementBatch`] with `LifeguardPool::with_session_batch` (RLS) / `run_batch` / `PooledLifeExecutor::run_batch` | 1 for the whole unit |
| statements that depend on each other, in one transaction | `LifeguardPool::with_session_transaction` (pipelined `BEGIN`) or `with_session_read_transaction` | one per statement |
| many rows of one entity to insert, upsert, update or delete | `insert_many`, `upsert_many`, `update_many`, `delete_many`, `BatchWriter` | one per chunk (default 1000 rows) |

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
| `failure` | `None` | failure policy for this call; `None` uses the entity's `#[batch_failure]`, else Strict (see [Partial success](#row-batches-and-partial-success)) |
| `transient_retries` | 1 | under Bisect, re-runs of a chunk after a transient error |
| `ordered` | false | chunks must stay whole and in order: Bisect falls back to Strict |

`VALUES` uses one parameter per value, so a 10-column entity fits 1000 rows in 10,000 parameters.
`unnest` uses one array parameter per column whatever the row count, so its SQL is the same for
every chunk size and is prepared once per connection. Full chunks of the `VALUES` form also repeat
the same SQL, so either way the statement cache serves every chunk after the first.

### Errors and atomicity

Chunks run in order, each as its own statement. By default (Strict) a failure returns
`BatchError`; [Bisect](#row-batches-and-partial-success) is the opt-in alternative:

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
- **Failure policy:** Strict by default, like the calls above; set
  `BatchWriterConfig::options.failure` (or `#[batch_failure = "bisect"]` on the entity) to hand only
  the rejected rows to the error handler, one row per call.
- **Delivery:** a failed write is retried (up to `retries` times, exponential backoff) only when
  that cannot write a row twice. Each `BatchError` carries a typed `kind` (`FailureKind`):
  - retried: `NotSent` (the connection failed before the statement reached the server),
    `Transient` (`40001`, `40P01`, pool acquire timeout);
  - after send (`ConnectionLost`, `ReplyTimeout`): never for `Insert`; an `Upsert` only when its
    conflict clause is `DO NOTHING` or only plain `col = EXCLUDED.col` assignments
    (`update_columns`), so a replay converges. `qty = t.qty + EXCLUDED.qty` is not retried;
  - `Data` (`22xxx`, `23xxx`) is never retried: it goes through the failure policy;
  - anything else is not retried.

  A retry resumes at the failing part: committed chunks are not written again or handed over.
  What is not retried goes to the error handler with the error, so the caller can tell a
  `ConnectionLost` insert (which may or may not have been applied) from a rejected row.
- **Shutdown:** `close()` flushes everything and returns the final counters. Dropping an open
  writer closes it with a bounded wait; call `close()` explicitly so failures are seen.

## RLS

The batch calls use the executor they are given. With a `PooledLifeExecutor` carrying a session
context, each chunk runs inside `BEGIN; rls_set_session(..); <statement>; COMMIT` like any other
statement, so policies (`USING` and `WITH CHECK`) apply to every row.

## Statement batches

```rust
let mut b = StatementBatch::new();
let open = b.query_opt("SELECT count(*) FROM orders WHERE status = $1", &Values(vec![Value::from("open")]))?;
let limits = b.query_all("SELECT key, value FROM limits", &Values(vec![]))?;
let mut out = pool.with_session_batch(&ctx, b)?;   // BEGIN · rls_set_session · 2 statements · COMMIT
let open = out.take(open);
let limits = out.take(limits);
```

- **One round trip.** Every message is written before any reply is read.
- **Atomic.** All statements share one transaction; the first real error is returned and nothing
  commits. A batch never splits or retries: it is one unit of work.
- **Independent statements only.** A statement cannot use an earlier statement's result. Use a
  CTE, or two units.
- **Isolation level.** `with_session_batch_isolated(ctx, IsolationLevel::RepeatableRead, b)` puts
  the level on the `BEGIN` (PostgreSQL rejects `SET TRANSACTION` once the RLS context is set).
- **Reply deadline.** The batch is one dispatch, bounded as a whole by the pool's reply timeout.

## Session transactions

`with_session_transaction` sends `BEGIN` and `rls_set_session` in the same write as the first
statement, saving one round trip. `with_session_read_transaction` (`BEGIN READ ONLY`) also sends
its `COMMIT` without waiting for the reply, which the connection's next request reads and skips.
PostgreSQL runs the `COMMIT` before anything sent after it on that connection, and the
transaction-local RLS settings end with it.

`ExclusivePrimaryLifeExecutor::run_batch` pipelines several statements inside a session
transaction, so `BEGIN · rls_set_session · w₁ … wₙ` is one round trip and `COMMIT` one more.

`LIFEGUARD_RLS_PIPELINE=off` turns all pipelining off (one statement per round trip, same results
and atomicity), to rule it out while diagnosing a problem without a rebuild.

## Row batches and partial success

`insert_many` and friends write a chunk of rows per statement. By default (**Strict**) a chunk
that fails writes nothing and the call returns `BatchError` naming the chunk.

**Bisect** is opt-in. When a chunk fails on bad data (SQLSTATE class 22 or 23), Lifeguard splits it
in half and retries each half, down to single rows. The bad rows come back in
`BatchOutcome::failed` with their input index and error, and every other row is written. That
costs about `k · log2(n)` extra statements for `k` bad rows in a chunk of `n` (a 1000-row chunk
with 3 bad rows takes about 53 statements), against `n` for row by row. Transient errors
(serialization failure, deadlock, pool acquire timeout) re-run the chunk instead, up to
`transient_retries` times. A dropped connection or reply timeout is returned without a re-run: the
chunk may already have been applied (the pool itself retries once when the connection failed
before the chunk was sent).

### When Bisect is safe

Only when rows are **independent**: each row can be written without the others, and writing some
but not all of them leaves the data correct. Idempotent ingest of separate facts (prices,
documents, events keyed by their own id) is the typical case. Rows that only make sense together,
rows whose order matters, or a batch that must commit whole are not.

Lifeguard cannot know which case it is in, so there is **no global switch**. You opt in:

```rust
// per call
insert_many(&exec, &rows, &BatchOptions { failure: Some(BatchFailure::bisect()), ..Default::default() })?;
// per entity: declares this entity's rows independent
#[derive(LifeModel, LifeRecord)]
#[table_name = "dividends"]
#[batch_failure = "bisect"]
pub struct Dividend { /* .. */ }
```

A per-call `failure` always wins over the entity's declaration. Configuration sets only sizes and
limits: `LIFEGUARD_BATCH_CHUNK_SIZE` (1000), `LIFEGUARD_BATCH_MAX_DEPTH` (20),
`LIFEGUARD_BATCH_TRANSIENT_RETRIES` (1).

Even when requested, Bisect falls back to Strict, and says why in
`BatchOutcome::fallback_reason`, when the batch shows a split would be unsafe:

- duplicate keys in the batch (splitting could change which row wins);
- a row references another row of the same batch through a self-referencing foreign key;
- `BatchOptions::ordered` is set.

Run Bisect on a pool executor (each chunk its own transaction). Inside a transaction the first
error aborts the transaction and every retry fails with `25P02`.

### What partial success looks like

```rust
let out = insert_many(&exec, &rows, &BatchOptions::bisect())?;
if !out.is_complete() {
    for f in &out.failed {
        log::warn!("row {} rejected ({:?}): {}", f.index, f.sqlstate, f.error);
    }
}
```

`out.policy_applied` says which policy ran. The metric
`lifeguard_entity_batch_total{entity, policy, kind}` counts `committed_rows`, `failed_rows`,
`bisect_round_trips` and `fallbacks`.

## Change notifications in bulk

`#[notify(channel = "..", on = "insert,update,delete", granularity = "statement")]` generates
statement-level triggers that publish the keys a statement touched as `"ids": [..]` (split under
PostgreSQL's 8000-byte limit), instead of one notification per row. See `NOTIFY_CONTRACT.md` for
the payload and for invalidation timing.

## When to use `COPY` instead

`COPY FROM STDIN` is the fastest way to load rows, but it bypasses the ORM entirely (no hooks,
no validators, no `ON CONFLICT`, no `RETURNING`) and needs a dedicated connection. For bulk loads
into staging tables it remains the right tool; for application writes, `insert_many` /
`upsert_many` get most of the way there (see the benchmark table) without giving those up.
