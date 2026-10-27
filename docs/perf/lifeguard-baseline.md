# Lifeguard performance baseline and findings

Phase 1 (sections 1–5) measured the ORM and fixed the small things. Phase 2 (section 6) rebuilt
the pool and added batching and the reflector; its gate against Phase 1 is in §6.1.

27 Oct 2026. The method follows BRRTRouter's `docs/perf/copy-and-borrow-audit.md`: measure, then
find where the time goes against a raw-driver floor, then fix what is small and safe, with numbers
before and after, and list the rest for a decision. The ORM API is unchanged. Every change here is
internal or an additive opt-in.

The harness is in [`examples/perf-suite`](../../examples/perf-suite). Design notes on the cache
component are in [`lifereflector-design-review.md`](lifereflector-design-review.md).

## 1. How to run

```bash
cd examples/perf-suite
./run.sh <label>                     # docker compose: Postgres 16 :55440, Redis 7 :56380
# results: target/perf-results/<label>.md and .json
PERF_SCENARIOS=pk_select,contention ./run.sh quick
LIFEGUARD_STATEMENT_CACHE_CAPACITY=0 ./run.sh nocache        # statement cache off
PERF_NO_DEFAULT_FEATURES=1 ./run.sh noobs                    # lifeguard without metrics/tracing
```

Knobs: `PERF_ITERATIONS` (3000), `PERF_WARMUP` (300), `PERF_POOL_SIZE` (8),
`PERF_CONCURRENCY` (`1,16,128,1024`), `PERF_CONTENTION_MS` (3000), `PERF_MAY_WORKERS` (8),
`PERF_REFLECT_*` (reflector prototype), `PERF_BATCH_SIZES` (`100,1000,10000`), `PERF_BATCH_REPS`
(5), `PERF_REPLY_TIMEOUT_OFF=1` (pool without a reply deadline), `PERF_QUEUE_DEPTH` (statements in
flight per slot). The suite refuses to run without `PERF_RESET=1`, and it
reads only `PERF_DATABASE_URL`, never `DATABASE_URL`.

Scenarios. All use synthetic tables: `perf_items` (7 columns, 101k rows), `perf_wide` (40
columns), `perf_ledger` (RLS) and `perf_reflect`.

| Scenario | ORM variant | Floors it is compared with |
|---|---|---|
| `pk_select` | `Entity::find().filter(Id.eq(k)).find_one` through `LifeguardPool` | `MayPostgresExecutor` (no pool); `may_postgres` with `&str` (prepares every call) and with a cached `Statement`; `tokio-postgres`, the same two ways |
| `list_{10,100,1000}` | `.filter(AccountId.eq(a)).all` | `may_postgres` and `tokio-postgres`, prepared, positional mapping |
| `insert` / `update` / `upsert` | `LifeRecord::insert`, `update` (returns the model), `insert_on_conflict ... DO UPDATE` | the same SQL, prepared |
| `rls_pk_select` | `with_session_context` (one statement) and `with_session_transaction`, as a non-superuser against a `FORCE ROW LEVEL SECURITY` table | `BEGIN` + prepared `rls_set_session` + prepared `SELECT` + `COMMIT` |
| `wide_map_cpu` | derived `FromRow` over 2000 preloaded 40-column rows | hand-written positional mapping |
| `wide_list_1000` | `.all` of 1000 × 40 columns | `may_postgres` prepared |
| `pk_contention` | K coroutines doing `pk_select` for 3 s on an 8-slot pool | K coroutines over 8 shared `may_postgres` connections (clones, pipelined), and `tokio-postgres` tasks over 8 clients |
| `obs_overhead_cpu` | the per-operation instrumentation alone, in a tight loop | the same with `--no-default-features` |
| `redis`, `cache_read` | `may_redis` `SET EX` (single and 64-deep pipeline) and `GET` | ORM pk select |
| `notify_payload_limit`, `reflector_*` | NOTIFY trigger → LISTEN → refresh `SELECT` → Redis, per-key and batched | — |
| `batch_{100,1000,10000}` | `insert_many` (VALUES, unnest, default), `upsert_many`, `update_many`, `find_by_ids`, `delete_many`, `BatchWriter` with 16 producers | the same rows one ORM call at a time; `COPY FROM STDIN` |
| `reflector_crate_e2e` | the `lifeguard-reflector` service: trigger → NOTIFY → batched `UNLINK` | — |

Sqlx and Diesel were left out: tokio-postgres already gives the driver floor. Flamegraphs are made
with `perf record --call-graph dwarf` + `inferno` (`docs/perf/flamegraphs/`, see §5).

Machine: dev box, 24 cores, shared with CI runners, so expect some noise. Postgres 16 and Redis 7 in
Docker on loopback. Toolchain 1.99.0. `may` with 8 workers. Absolute numbers are loopback numbers;
over a real network every extra round trip costs the RTT (0.2–1 ms in-region), so the round-trip
findings below matter **more** in production than they do here.

## 2. Results: before vs after this pass

`before` is `origin/main` at `e77cb1e`. `after` is this commit. Sequential latencies on the shared
box vary run to run by up to 2×, so both `after` runs are shown (A = the full run, B = the
observability-on rerun). Contention throughput is the more stable signal.

### Sequential, one caller (p50 µs)

| Scenario | ORM before | ORM after (A / B) | raw `may_postgres` prepared | raw `tokio-postgres` prepared |
|---|---:|---:|---:|---:|
| pk_select | 280.8 | **93.3 / 44.6** | 36–39 | 33–43 |
| list_10 | 339.5 | **96.9 / 44.0** | 43–52 | 51–58 |
| list_100 | 399.3 | **152.0 / 149.0** | 76–79 | 73–87 |
| list_1000 | 920.6 | **575.2 / 638.1** | 391–394 | 373–413 |
| insert | 214.9 | **68.5 / 116.8** | 29–38 | 39–45 |
| update (model returned) | 589.6 | **137.8 / 216.4** | 40–45 | 44–52 |
| upsert | 257.4 | **77.7 / 242.4** | 42–50 | 36–55 |
| rls_pk_select (session ctx) | 705.0 | **271.1 / 158.3** | 109–145 | — |
| wide_list_1000 (40 cols) | 4085 | **3131** | 2248–2697 | — |
| wide_map_cpu, ns per 40-col row | 1745 | **623** | 559–570 (positional) | — |

Mean latencies move the same way: pk_select mean went from 565 µs to 116 µs, and
update from 1027 µs to 223 µs.

### Pool under contention (8 slots; ops/s and p99)

| Callers | ORM before | ORM after (A / B) | ORM after, statement cache off | raw `may_postgres`, 8 conns | raw `tokio-postgres`, 8 clients |
|---:|---:|---:|---:|---:|---:|
| 1 | 3.9k | 5.8k / 9.9k | 8.2k | 21–33k | 17–22k |
| 16 | 32.8k | **78.4k / 76.6k** | 36.3k | 161–197k | 192–265k |
| 128 | 29.8k (p99 35 ms) | **104.0k / 93.5k** (p99 7–8 ms) | 41.9k | 473–628k | 368–485k |
| 1024 | 31.7k (p99 141 ms) | **102.1k / 97.7k** (p99 32–65 ms) | 40.1k | 555–663k (p99 2.8–3.7 ms) | 667–718k |

### Observability overhead

The per-operation instrumentation a pooled select pays (entity timer, query and checkout
histograms, statement-cache counter, in-flight and waiter gauges) costs **264 ns** with metrics on
and **15.5 ns** with metrics off (`obs_overhead_cpu`). Against a ≥ 40 µs pooled select that is
≤ 0.7 %. End to end, contention throughput with metrics on vs off was 76.6k vs 78.3k (k=16) and
93.5k vs 92.2k (k=128): inside run-to-run noise, and inside the 1–2 % target. Most of the 264 ns
is the two existing OpenTelemetry histograms (attribute lookup inside the SDK). The new
per-entity series are prometheus-native handles resolved once per entity: atomics only.

## 3. Quick wins in this commit (internal, API unchanged)

1. **Per-connection prepared-statement cache in the pool** (`src/pool/stmt_cache.rs`). Every
   `&str` query used to cost Parse+Describe (one round trip), then Bind+Execute (a second), then a
   Close. Now each worker keeps its connection's statements (`LIFEGUARD_STATEMENT_CACHE_CAPACITY`,
   default 256, `0` turns it off), and evicts and retries once on a stale-statement error
   (`26000`, or `0A000` "cached plan must not change result type"). Through PgBouncer this needs
   `max_prepared_statements > 0`, which the uncached path also needed. Effect: the largest single
   win, 2.5× pool throughput under contention (41.9k → 104k at k=128) and about half the latency
   per call.
2. **Simple-protocol `BEGIN`/`COMMIT`/`ROLLBACK`** in the contextual (RLS) paths of the pool and of
   `MayPostgresExecutor`. They were prepared every time: 2 round trips each. Now 1. The
   `rls_set_session` call uses the statement cache. A contextual statement went from 8 round trips
   to 4: rls_pk_select 705 → 158–271 µs.
3. **Positional `FromRow`.** The derives now try the field's position in entity order (the order of
   `SELECT *`) before the by-name lookup (`__lg_try_get`). By-name lookup is a linear scan per
   field, then a second case-insensitive scan when nothing matched, so it was O(columns²) per row.
   When the result is not in entity order, the fallback gives the same result and the same
   errors. 40-column rows: 1745 → 623 ns per row (the hand-written floor is 570).
4. `std::env::var("LIFEGUARD_DEBUG_SQL")` was read on **every generated insert**: a process-wide
   env lock plus an allocation. It is now read once (`lifeguard::debug_sql_enabled()`).
5. The metrics `pool_tier` attribute allocated a `String` per record. It is now static for
   `primary`/`replica`. `SelectQuery::all` pre-sizes its result `Vec`.
6. **Observability** (see the design review §6 for the full list): per-entity
   `lifeguard_entity_op_duration_seconds{entity,op}`, `..._rows_total` and `..._errors_total`, from
   the table name the derive knows; pool checkout histogram; in-flight and waiter gauges; statement
   cache hit/miss. The `lifeguard.execute_query` span now carries `db.operation` (the statement
   kind) instead of the full SQL text. `LIFEGUARD_METRICS_ENTITY_LABEL=off` folds entities into
   `_all`.

## 4. Ranked findings (what is left)

Expected gains are measured against the `after` numbers above.

| # | Finding | Evidence | Proposal (keeps the API) | Expected gain | Size |
|---|---|---|---|---|---|
| 1 | **The pool serialises each connection and hops threads four times per query.** One OS thread per slot. A per-slot `may::Mutex` is held for the whole job. Each call goes caller coroutine → crossbeam queue → worker thread → `may_postgres` request queue → `pg-io` coroutine and back, through a `may` mpsc. Round-robin picks a slot even when it is busy, so callers queue behind it while other slots sit idle. | Pooled pk_select 45–93 µs against 39 µs raw on the same connection. In the flamegraphs, 71 % of may-side CPU is futex park/unpark. At k=128 the ORM does 104k ops/s against **628k** for 8 raw connections; at k=1024 its p99 is 32–65 ms against 2.8 ms. | A **coroutine-native pool**: the caller's coroutine uses a `Client` clone of a slot directly. `may_postgres` already pipelines concurrent requests over one connection. Pick the least-loaded slot. Pin a slot exclusively only for transactions and RLS units. Keep `LifeExecutor`, `ReadPreference`, heal, rotation and timeouts. The worker threads go away, and so does the class of "starved worker thread" wedge they caused. | 3–6× throughput under concurrency; −30 to −50 µs per call | Large: needs a decision |
| 2 | **An RLS-scoped statement costs 4 round trips** (`BEGIN`, `rls_set_session`, the statement, `COMMIT`), paid per statement. | rls_pk_select 158–271 µs against ~40 µs for a plain pk select | Send all four as one pipelined batch on the pinned connection (one round trip), or `BEGIN; SELECT set_config(...)` as a single simple query with bound literals. Push callers towards `with_session_transaction` for several statements per request. | ≈ 3× on contextual statements; more over real RTTs | Medium |
| 3 | **`update()` runs `UPDATE` then a separate `SELECT`** to rebuild the model: two pool jobs. The `SELECT` may also route to a replica, so it can **read its own write stale**. | update 138–216 µs against 40 µs for `UPDATE ... RETURNING` | `RETURNING *` + `FromRow` when the entity has no `select_as` columns and no soft-delete; keep the current path otherwise. Fixes the replica read-after-write hazard as well. | ≈ 2× on update; a correctness fix with replicas | Small–medium |
| 4 | **No statement cache on `MayPostgresExecutor`** (the direct, unpooled executor). | pk_select through it is 54–108 µs against 39 µs prepared | The same cache, held in the executor value (keyed to its client). Additive and internal. | ≈ 2× on direct-executor paths | Small |
| 5 | **Per-call allocations on the pool path:** `query.to_string()`, `Values → Vec<OwnedParam>`, a full `SessionContext` clone (role and permission `Vec<String>`s) per contextual job, and the role/permission JSON re-serialised per statement. | Visible in the flamegraphs under `dispatch`/`exec_worker_job`; small next to the round trips | `Arc<str>` SQL, `Arc<SessionContext>` with its JSON precomputed once | A few µs per call | Small |
| 6 | **SeaQuery builds the SQL string on every call**, and the statement cache then hashes it. | Part of the remaining ORM-vs-raw gap on cheap queries | Cache the built SQL per query shape for the common `find_by_id` / `filter(eq)` forms | 1–3 µs per call | Medium |
| 7 | **Notifications have no wake-up.** `Client::notifications()` is a queue the caller must poll. | The reflector prototype pays its 200 µs poll interval; a 50 ms poll in an application adds 0–50 ms | A `may` channel or event signalled by the `pg-io` coroutine on `NotificationResponse` (in `may_postgres`) | Removes the poll latency; enables a real reflector | Small (other repo) |
| 8 | **Blocking calls reachable from coroutines:** `RedisCacheProvider` uses the blocking `redis` crate and opens a TCP connection per `get`/`set`. `may_redis` is built on a `may` fork, so without a `[patch]` a binary runs two schedulers. | Code reading; the perf suite needs the patch | Move `RedisCacheProvider` onto `may_redis` with one shared connection. Upstream the fork's change to `may`, or pin the patch in a workspace template. | Avoids worker-thread stalls once any cache is enabled | Small–medium |
| 9 | **Batch writes have no ORM path.** Callers that write many rows loop `insert()`, one statement each. | One round trip per row, 69–117 µs each | Additive `insert_many` / `upsert_many` (multi-row `VALUES`, chunked under the 65,535-parameter limit) | 10–100× on bulk writes | Medium |
| 10 | **Test and dev crates are normal dependencies** (`may_minihttp`, `csv`, `fake`, `rand`, `redis`), so every consumer compiles them. | `Cargo.toml` | Move them to dev-dependencies or behind features (`redis` behind a `redis-cache` feature) | Build time and binary size for consumers | Small |
| 11 | LifeReflector: the per-key refresh loop cannot keep up with bursts; batching can. Full design review in the companion doc. | per-key 7–14k/s with lag growing to 15–37 s; batched 74–80k/s, p99 2.7–2.8 ms | Batched loop, version-checked writes, backfill on reconnect | — | Large: needs decisions |

## 5. Flamegraphs

`docs/perf/flamegraphs/{before,after}-may-side.svg` were recorded with `perf record -F 999
--call-graph dwarf` over `pk_select` + `pk_contention` (k=128), and filtered to the `may`/pool
threads. The tokio baseline's threads are excluded.

DWARF unwinding cannot walk `may`'s generator stacks, so most samples are attributed to leaf
frames, not to Lifeguard functions. What the profile still shows clearly:

| Share of may-side CPU samples | before | after |
|---|---:|---:|
| kernel `futex` wait/wake paths (thread park/unpark, scheduler) | 60.0 % | 71.0 % |
| `malloc` / `free` | 9.5 % | 9.5 % |
| `may_postgres::prepare::prepare` (Parse/Describe) | 1.4 % | 0.5 % |
| `crossbeam` channel | 0.2 % | 0.4 % |

So once the statement cache removes the second round trip, the CPU left on the ORM path is mostly
**thread hand-off**: futex wake-ups between the caller's `may` worker, the pool's OS thread and the
connection's I/O coroutine. That is finding 1. Allocation (9.5 %) is the next target (finding 5).
Coroutine-aware call stacks need frame pointers (`RUSTFLAGS="-C force-frame-pointers=yes"` plus
`perf --call-graph fp`) or an in-process sampler. That is a follow-up for the harness.

## 6. Phase 2: coroutine-native pool, one-round-trip RLS, `RETURNING`, executor cache, batching

27 Oct 2026, same machine and harness. Findings 1–5 and 9 of §4 are done; 7 and 8 are partly
done (see §6.5). The public API is unchanged; batching is additive.

### 6.1 The gate: every suite row against `cb953eb`

`base` is `cb953eb` (the Phase 1 commit) run with its own harness; `new` is this commit. They
were run interleaved (base, new, base, new, base, new) on the shared box, and each cell is the
median of the three runs. The machine's load average was 15–50 during these runs (other
workloads), so absolute numbers are noisier and mostly higher than in §2; compare the columns
with each other, not with §2. **Every row is better on p50, p99 and ops/s.**

Sequential, one caller (µs):

| Scenario (k=1) | p50 base → new | p99 base → new | ops/s base → new |
|---|---:|---:|---:|
| pk_select `orm_pool` | 78 → **36** | 831 → **98** | 7.6k → **25.7k** |
| pk_select `orm_direct_client` | 66 → **33** | 334 → **75** | 12.8k → **28.7k** |
| list_10 `orm_pool` | 81 → **39** | 1135 → **72** | 6.7k → **24.7k** |
| list_100 `orm_pool` | 148 → **84** | 1184 → **361** | 4.1k → **10.6k** |
| list_1000 `orm_pool` | 620 → **530** | 2144 → **864** | 1.4k → **1.9k** |
| insert `orm_pool` | 84 → **39** | 1194 → **115** | 6.3k → **23.4k** |
| update `orm_pool` (model returned) | 158 → **48** | 1716 → **272** | 3.3k → **16.8k** |
| upsert `orm_pool` | 91 → **45** | 1205 → **122** | 5.8k → **21.5k** |
| rls_pk_select `orm_pool_session_ctx` | 233 → **69** | 2662 → **172** | 2.3k → **13.3k** |
| rls_pk_select `orm_pool_session_tx` | 308 → **121** | 2833 → **422** | 2.1k → **7.5k** |
| wide_list_1000 `orm_pool` | 2987 → **2919** | 5168 → **5123** | 0.30k → **0.33k** |

Pool under contention (8 slots, `pk_contention`):

| Callers | ORM ops/s base → new | p50 µs base → new | p99 µs base → new | raw `may_postgres`, 8 conns (ops/s, p99 µs) | new, reply deadline off (ops/s) |
|---:|---:|---:|---:|---:|---:|
| 1 | 6.9k → **20.3k** | 78 → **35** | 1204 → **281** | 23k, 242 | 23k |
| 16 | 76.8k → **165.1k** | 91 → **88** | 1740 → **247** | 142k, 267 | 175k |
| 128 | 89.7k → **310.3k** | 546 → **360** | 10474 → **1548** | 433k, 1491 | 388k |
| 1024 | 86.3k → **324.9k** | 11984 → **2873** | 83118 → **5831** | 573k, 3480 | 389k |

Under contention the old pool's p50 was low because it was unfair: at k=128 its mean latency was
1.4 ms while its p50 was 0.14–0.55 ms, i.e. most callers were fast and a tail waited behind busy
slots for tens of milliseconds. The new pool serves callers in order, so p50 sits near the mean
and p99 drops by 5–14×.

Reliability checks on the new pool, all green: the full DB suite (152 tests), RLS context isolation
under concurrency (`test_p1`–`test_p7`) 10 runs, `worker_starvation_repro`, `pool_worker_death`
(including a new coroutine-caller reply-timeout test), and `conn_leak_probe` 10 runs in a row.
`conn_leak_probe` counts backends database-wide, so it can fail when another test binary connects
to the same database at the same moment (it did once in a whole-workspace run, as in Phase 1);
alone it passes every time.

### 6.2 What changed, item by item

1. **Coroutine-native pool** (finding 1). A slot is one connection that many callers use at once:
   each caller takes a `Client` handle of the slot's connection and `may_postgres` pipelines the
   requests. The least-loaded open slot is chosen; at most `job_queue_capacity_per_worker`
   statements are in flight per slot (a semaphore, with the acquire timeout). Transactions and
   RLS units pin a slot exclusively; concurrent pins claim distinct slots with a compare-and-swap,
   so two transactions never queue on one slot while another is idle (that deadlock was caught by
   `test_p6` while building this). The slot gate is a poison-free atomic: a panic inside
   `with_session_transaction` no longer leaves a poisoned lock behind. The worker threads, their
   queues and the thread hand-offs are gone.

   The reply deadline is kept, but no longer costs a coroutine, a channel and a timer per
   statement. A coroutine caller runs its statement inline and registers it with the slot's
   watchdog (the pool's maintenance thread); past the deadline the watchdog shuts the
   connection's socket down, `may_postgres` fails every request waiting on it, and the caller gets
   `PoolReplyTimeout` (the slot is then replaced in the background). An OS-thread caller still
   waits on a helper coroutine with a timeout, which holds even when every `may` worker is
   blocked (the starvation reproducer). Measured: with the helper coroutine on every statement
   the pool was 30–50 % slower sequentially and had 2–4× worse p99; with the watchdog it is
   within noise of running with no deadline at all (last column above).

2. **RLS units in one round trip** (finding 2). `BEGIN`, `rls_set_session(..)`, the statement and
   `COMMIT` are written together with cached statements and read back in order: 233 → 69 µs p50.
   Statements that return only a row count are pipelined the same way, reading the count from
   `RowStream::rows_affected` (fork `may_postgres` 9c55df0; before it they took three round trips).

3. **`update()` is one `UPDATE … RETURNING *`** (finding 3): 158 → 48 µs p50, and the
   read-your-own-write hazard with replicas is gone. Entities with `select_as` columns or soft
   delete keep the two-statement path.

4. **Statement cache in `MayPostgresExecutor`** (finding 4): pk_select through the direct
   executor 66 → 33 µs p50.

5. **Allocations** (finding 5): `select_as` detection per query allocated a `ColumnDefinition` per
   column; it is now computed once per entity. RLS session arguments are built once per executor,
   not per statement. SQL travels as `Arc<str>`.

### 6.3 Batching

`lifeguard::batch` (guide: [`docs/BATCHING.md`](../BATCHING.md)). Rows per second, pool of 8,
one caller (the `BatchWriter` row has 16 producers), median of 5:

| Operation | 100 rows | 1,000 rows | 10,000 rows |
|---|---:|---:|---:|
| `insert` one row per call | 16.5k | 17.2k | — |
| `insert_many`, multi-row `VALUES` | 187k | 201k | 244k |
| `insert_many`, `unnest` arrays | 177k | 302k | 320k |
| `insert_many`, default options | **231k** | **317k** | **292k** |
| raw `COPY FROM STDIN` (context) | 221k | 294k | 391k |
| `upsert` one row per call | 15.4k | 15.8k | — |
| `upsert_many` (existing keys, `DO UPDATE`) | **137k** | **160k** | **153k** |
| `update` one row per call | 15.1k | 13.3k | — |
| `update_many` | **167k** | **211k** | **209k** |
| `find_one` by key, one per row (the N+1) | 19.9k | 19.1k | — |
| `find_by_ids` | **581k** | **1,059k** | **913k** |
| `delete_many` | 338k | 1,008k | 1,112k |
| `BatchWriter`, 16 producers, upsert, until `close()` | 131k | 151k | 157k |

So batched writes are 10–16× single-row calls on loopback, and `insert_many` is within 0–25 % of
`COPY` while keeping hooks, validators, `ON CONFLICT` and `RETURNING`. `find_by_ids` replaces an
N+1 loop at 30–55× the rate. Over a real network each single-row call also pays the RTT, so the
gap grows with distance to the database.

### 6.4 LifeReflector

The `lifeguard-reflector` crate (README in the crate) measured end to end: a writer commits a row
with the generated trigger, the reflector `UNLINK`s its key.

| Load | Keys/s | e2e p50 | e2e p99 | Notes |
|---|---:|---:|---:|---|
| 1 writer, 500 rows/s | 422 | 0.78 ms | 2.6 ms | 1,994 flushes for 2,000 changes: each change goes out on its own |
| 16 writers, as fast as they go | 75.9k | 0.32 ms | 2.1 ms | 34k flushes (about 7 keys each): batches grow with load |
| same, 1M unrelated keys in Redis, writes start during the start-up catch-up | 77.1k (burst), 419 (paced) | 0.32 ms / 0.84 ms | 2.1 ms / 2.8 ms | the catch-up `SCAN`s a page per loop pass between live flushes, so it does not delay them |

For comparison, the Phase 1 prototype that re-read each row and `SET` it fell behind under the
same burst (8.4k keys/s, lag growing to 26 s); its batched variant kept up (82.6k/s, p99 2.2 ms),
which is the shape the crate now has, minus the re-read. Writers are unaffected by the trigger's
extra `v` and `ts` fields (the INSERT's p50 stays at ~0.18–0.21 ms in both scenarios).

Two earlier paced runs showed an e2e p50 of 2.5 s. In the first, the start-up catch-up ran to
completion before live changes were served (it is now incremental). The second used the old
harness, which started writing on a fixed 1.5 s timer; it was not reproduced once the harness
waited for leadership, so its cause is not fully pinned down. The third row starts writing during
a long catch-up on purpose and shows no delay; rerun it after any change to the loop.

### 6.5 Still open

| # | Finding | State |
|---|---|---|
| 6 | SeaQuery builds the SQL string per call | open |
| 7 | No notification wake-up in `may_postgres` | done: `Client::next_notification` (fork `may_postgres` 9c55df0); the reflector and the perf listener block on it instead of polling |
| 8 | `RedisCacheProvider` on the blocking `redis` crate; two `may` schedulers with `may_redis` | the scheduler half is fixed (workspace `[patch]` to the fork `may_redis` uses); the provider is unchanged |
| 10 | Test crates as normal dependencies | `rand` removed (unused); `fake`, `csv`, `may_minihttp` and `redis` remain |
| — | Execute-kind RLS statements take 3 round trips | done: one round trip via `RowStream::rows_affected` (fork `may_postgres` 9c55df0) |
