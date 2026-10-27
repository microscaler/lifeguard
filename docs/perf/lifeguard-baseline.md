# Lifeguard performance baseline and first-pass findings

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
`PERF_REFLECT_*` (reflector prototype). The suite refuses to run without `PERF_RESET=1`, and it
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
