# LifeReflector: where it stands and the design choices to revisit

Status: review, 27 Oct 2026. Companion to [`lifeguard-baseline.md`](lifeguard-baseline.md), whose
`reflector_*` rows are the evidence quoted here. Everything below is generic: it describes the
component, not any one application's tables.

> **Update (Phase 2, same day).** The approved design is now built in the `lifeguard-reflector`
> crate: LISTEN/NOTIFY kept, payload `{"table","op","id","v","ts"}` (the `v`/`ts` stamps are in the
> generated trigger), invalidate rather than rewrite, batched `UNLINK`, one active instance by a
> session advisory lock with listening standbys (gapless take-over when the standby's buffer
> covers the old leader's watermark), catch-up after gaps (ranged by a change column, or a
> per-table flush), refusal of a `noeviction` Redis, and `lifereflector_*` metrics. The `may` /
> `may_redis` double scheduler is resolved by patching `may` to the fork workspace-wide (the fork is
> crates.io 0.3.51 plus one additive API). Still open: a notification wake-up in `may_postgres`
> (the reflector polls its queue every 500 µs), multi-region fan-out (§5.4), and deployment. The
> crate's README is the current description; sections 1–2 below describe the state before it.

## 1. What the design says

The design is in `lifeguard-reflector/README.md`, `VISION.md`, `ARCHITECTURE.md`,
`docs/background/05_redis_cache.md`, `docs/operation_sledge_hammer/09_caching_and_latency_reduction.md`
and the LLM wiki page `docs/llmwiki/topics/reflector-cache-and-coherence.md`. In short:

1. An application writes through Lifeguard to the Postgres primary.
2. A row trigger runs `pg_notify(channel, '{"table","op","id"}')`. The payload carries the key, never
   the row.
3. One reflector per cluster `LISTEN`s. It is leader-elected: Raft in the README, a Redis lock in
   `src/cache/reflector.rs`.
4. For each notification it checks whether the key is already in Redis, the "TTL-based active set".
   If the key is there, it re-reads the row from Postgres and `SETEX`es it. If not, it ignores the
   notification.
5. Services read from Redis. On a miss they read Postgres and populate the cache (cache-aside).
6. Later there is one Redis per region or continent, each fed by the reflector, as an open-source,
   Coherence-style near cache.

## 2. What exists today

| Piece | State |
|---|---|
| Trigger generation | **Built.** `#[notify(channel = "...", on = "insert,update,delete")]` on an entity. `lifeguard-migrate` generates the trigger function and trigger, with payload `{"table": TG_TABLE_NAME, "op": TG_OP, "id": <pk>}`. Single-column PKs only. Marked SPIKE. |
| `LISTEN` delivery in the driver | **Built.** `may_postgres` decodes `NotificationResponse` into `Client::notifications()`, a lock-free queue. There is **no wake-up**: a consumer has to poll the queue. |
| Coroutine Redis client | **Built.** `may_redis` 0.1.0 has RESP2, a pipeline, cluster, TLS and pub/sub. It depends on a `may` fork (`microscaler/may`, branch `may-sleep`), so a binary that also links crates.io `may` must `[patch]` them together. Without the patch it runs two coroutine schedulers. |
| Reflector service | **Not built.** `lifeguard-reflector/src/main.rs` prints "In Development". The standalone `lifeguard-reflector` repository is the same stub. `src/cache/reflector.rs` is a scaffold that hard-codes `is_leader = true` and sleeps in a loop. |
| Read-side cache in the ORM | **Not built.** `CacheProvider` / `RedisCacheProvider` exist, but the only caller is a write-through in generated `update()`/`delete()`, and only for integer PKs. No executor returns a provider (`cache_provider()` is `None` everywhere). `RedisCacheProvider` uses the blocking `redis` crate and opens a new TCP connection for every `get`/`set`. On a `may` worker thread that is an OS-blocking call. |
| Metrics | Pool and query metrics exist. Per-entity ORM metrics were added in this pass ([observability](#6-observability-contract)). No reflector metrics exist, because no reflector exists. |

## 3. Measured: the prototype in the perf suite

`examples/perf-suite` has a minimal reflector built from the shipped pieces: a `may_postgres`
`LISTEN` connection, the generated payload shape, a refresh `SELECT` and `may_redis`. It was measured
on a 24-core dev box against local Postgres 16 and Redis 7 (see the baseline doc for the numbers):

* **Paced writes** (500/s, one writer): commit → Redis write p50 ≈ 0.8 ms, p99 ≈ 2.9 ms in both modes.
  About 200 µs of that is the queue poll interval (`PERF_REFLECT_POLL_US`), which only exists because
  the driver has no wake-up.
* **Burst** (16 writers, ≈ 78k inserts/s):
  * **per-key** (the README's design: one `SELECT` and one `SET` per notification) reflected only
    ≈ 7k/s. Lag grew without bound: p50 12 s, p99 37 s, still draining after 30 s.
  * **batched** (drain the queue, one `WHERE id = ANY($1)`, one pipelined batch of `SET`s) kept up
    at ≈ 80k/s with p50 0.56 ms and p99 2.8 ms.
* **Payload limit**: `pg_notify` accepts 7999 bytes and rejects 8000 ("payload string too long").
  A full-row trigger (`row_to_json(NEW)`) on a 9 KB row makes **the application's INSERT fail**.
  This is a hard reason to keep payloads key-only.

## 4. Gaps between the design and a reliable service

1. **Reliability of `LISTEN/NOTIFY`.** Notifications go only to connected listeners. Anything
   committed while the reflector is disconnected (restart, failover, network blip) is lost, and
   nothing records a gap. The design has no backfill.
2. **Ordering and duplicates.** NOTIFY is delivered in commit order on one connection, which is
   good. But the payload has no commit LSN or xid, so a consumer cannot tell "already applied" from
   "new", or detect a gap. Postgres also folds identical `(channel, payload)` pairs within one
   transaction, which is harmless for key-only payloads.
3. **The refresh read races the write.** "Re-read the row, then `SET`" is correct only if refreshes
   for a key are applied in order. With more than one worker, or with retries, an older read can
   overwrite a newer one. A version column (or the commit LSN), plus a compare-and-set in Redis
   (Lua `SET` if newer), closes this.
4. **"Only refresh keys that exist"** is a check-then-act race: the `EXISTS` and the `SET` are
   separate. A key can be evicted between them and resurrected with a TTL. It also needs one extra
   Redis round trip per notification. The alternative is `SET ... XX`, which updates only if the key
   exists, atomically and in one command. Or invalidate with `DEL` and let readers repopulate.
5. **Deletes.** The trigger fires on DELETE, but the design describes only refresh. A delete must
   become a `DEL` (or a tombstone with a short TTL), or a stale row lives until its TTL.
6. **Transaction pooling.** `LISTEN` does not work through PgBouncer in transaction mode. The
   listener needs a direct (session) connection to the primary. That must be a deployment rule.
7. **The notify queue on the server.** Postgres buffers undelivered notifications in a shared queue
   (8 GB by default). A slow listener holds back queue cleanup for the whole cluster.
   `pg_notification_queue_usage()` is the metric to alert on.
8. **Leader election.** Raft (README) and a Redis lock (scaffold) disagree. For a single-writer
   consumer, a Postgres advisory lock held on the LISTEN connection is the simplest correct choice:
   the lock dies with the session, so a fenced-off old leader cannot keep writing once its
   connection is gone.

## 5. Options to decide

### 5.1 Change source: LISTEN/NOTIFY or logical replication

| | LISTEN/NOTIFY + triggers (today) | Logical replication (`pgoutput`, or `wal2json`) |
|---|---|---|
| Missed changes while down | Lost. Needs a separate backfill. | None: the slot holds WAL until confirmed. |
| Ordering / dedupe | Commit order, but no LSN in the payload | Commit LSN on every change. Exactly-once is possible with LSN fencing. |
| Payload limit | 8000 bytes (key-only is fine) | None |
| Write-path cost | A trigger per row, plus the notify queue | No triggers. WAL is already written. |
| Operational risk | Low | A stuck slot retains WAL and can fill the disk. Needs `max_slot_wal_keep_size` and alerting. |
| Managed Postgres | Works everywhere | Needs `wal_level=logical` and a replication role. The CNPG/Zalando operators support it. |
| Driver work | Done in `may_postgres` (needs a wake-up) | `may_postgres` would need `START_REPLICATION` / CopyBoth support. Not built. |
| Multi-region | Each region's reflector LISTENs to the primary | One slot per region, or one consumer fanning out |

**Recommendation:** keep NOTIFY for v1, because it is what ships, and make it safe:

* key-only payloads with the commit **LSN** added (`pg_current_wal_insert_lsn()` in the trigger is an
  upper bound and good enough as a monotonic version);
* a **backfill** on every (re)connect: `updated_at >= last_seen - safety` per reflected table, or a
  small `lifeguard_changes(table, id, lsn)` log table pruned after N minutes;
* **versioned writes** to Redis.

Plan logical replication as v2, once `may_postgres` grows CopyBoth. It removes backfill and triggers
entirely.

### 5.2 What goes in Redis, and coherence

* **Refresh-on-write (the README) or invalidate-on-write.** Refresh keeps hit rates high on hot keys
  but costs a Postgres read per change. Invalidate (`DEL`) is cheap and always correct, at the cost
  of a miss on the next read. A sensible default: invalidate, plus refresh for a configured set of
  hot entities.
* **Versioned values**, e.g. `{"v": <lsn>, ...}`, written with a Lua compare-and-set. This makes
  retries, replays and multiple writers safe, and lets readers report staleness.
* **TTL** stays as a safety net (hours), not as the coherence mechanism.
* **Serialisation:** the prototype formats JSON by hand. A real reflector should serialise from the
  row with a stable schema (`serde_json`), or store the row as a Redis hash (`HSET`) so a reader can
  fetch fields.

### 5.3 Batching and Redis round trips

The burst numbers settle this. Notifications must be drained in batches, with one `= ANY($1)` read
and one `may_redis` pipeline per batch (bounded, e.g. 256 keys or 2 ms). Per-key processing caps out
an order of magnitude below the write rate. `may_redis::Pipeline` allocates two channels per command
(`spsc::channel` per response). That is fine at these rates, but it is the first thing to optimise in
`may_redis` if reflector CPU matters.

### 5.4 Multi-region fan-out

* **Option A, one reflector per region**, each `LISTEN`ing to the primary across regions, with a
  regional Redis. Simple. Every region pays a cross-region `SELECT` per batch. Cross-region LISTEN
  needs the direct-connection rule above.
* **Option B, one reflector at the primary** that writes to every regional Redis. One read, N
  pipelined writes. A regional outage then delays only that region's writes; give each region its
  own queue.
* **Option C, a log in between** (a stream such as Fluvio/Kafka, or Redis Streams). The reflector
  publishes changes once; regional appliers consume them, each with its own offset. This is the
  Coherence-like design: replay, independent lag per region, backpressure.

Recommendation: B to start (it reuses the batched loop), and C if the number of regions or the lag
budget per region grows.

### 5.5 Failure and backfill

* On (re)connect: take the advisory lock, `LISTEN`, *then* backfill from `last_applied_lsn` /
  `updated_at`. Listening first means nothing falls into the gap between the backfill query and
  `LISTEN`.
* Persist `last_applied_lsn` per channel in Redis (or in a Postgres table) after each batch.
* Track queue depth inside the reflector and drop to "invalidate only" under overload, rather than
  falling behind indefinitely.

### 5.6 Placement: hot and durable Redis

Reflected data is cache: it belongs on an **eviction-enabled "hot" Redis** (`allkeys-lru` or
`volatile-lru`, with TTLs), never on a `noeviction` Redis that holds durable state such as sessions.
A full `noeviction` instance rejects writes, and a reflector flood would turn into session-write
failures. The reflector configuration should refuse to start against an instance whose
`maxmemory-policy` is `noeviction`.

## 6. Observability contract

Shipped in this pass (Lifeguard core; zero-allocation handles, see `src/metrics.rs`):

| Metric | Labels | Backs |
|---|---|---|
| `lifeguard_entity_op_duration_seconds` (histogram) | `entity`, `op` = select/insert/update/delete/upsert | per-entity read and write rates (`_count`), query p50/p95, slowest operations |
| `lifeguard_entity_rows_total` | `entity`, `op` | rows read and written |
| `lifeguard_entity_errors_total` | `entity`, `op` | errors per entity |
| `lifeguard_pool_checkout_seconds` (histogram) | `pool_tier` | checkout wait p95 |
| `lifeguard_pool_in_flight`, `lifeguard_pool_waiters` (gauges) | `pool_tier` | pool saturation (with `lifeguard_pool_workers`) |
| `lifeguard_statement_cache_total` | `pool_tier`, `result` | prepared-statement reuse |

`entity` is the table name the derive already knows, so a new entity appears with no dashboard
change. `LIFEGUARD_METRICS_ENTITY_LABEL=off` collapses it to `_all`.

To define when the reflector is built (names reserved here so dashboards can be written against
them):

| Metric | Labels |
|---|---|
| `lifereflector_notifications_total` | `channel`, `entity` |
| `lifereflector_notification_payload_bytes` (histogram) | `channel` |
| `lifereflector_reflect_lag_seconds` (histogram; commit → Redis write, from the LSN or a commit timestamp in the payload) | `entity` |
| `lifereflector_cache_writes_total` | `entity`, `result` = set/del/skipped_stale |
| `lifereflector_batch_size` (histogram) | — |
| `lifereflector_reconnects_total`, `lifereflector_backfills_total`, `lifereflector_backfill_rows_total` | `channel` |
| `lifereflector_dropped_total` | `channel`, `reason` |
| `lifereflector_listening_channels` (gauge), `lifereflector_leader` (gauge) | — |
| `lifeguard_cache_reads_total` | `entity`, `result` = hit/miss |

`lifeguard_cache_reads_total` needs a read-side cache API in the ORM (`find_by_id` served from Redis
first), which does not exist yet.
