# LifeReflector

Keeps a Redis cache of Lifeguard entities coherent with Postgres by **invalidating** cached rows
when they change. Licence: see [`LICENSE`](LICENSE).

```
writer ──INSERT/UPDATE/DELETE──▶ Postgres ──trigger: pg_notify(channel, {"table","op","id","v","ts"})──▶
  LifeReflector (one active, others listening standbys) ──batched UNLINK {prefix}{table}:{id}──▶ Redis
readers: GET key → miss → read Postgres → SET key (with a TTL)
```

## Design

- **Invalidate, never rewrite.** The reflector deletes keys; readers refill on miss. A duplicate,
  late or replayed invalidation costs one extra read and never installs a value itself, which is
  what makes at-least-once delivery, overlapping leaders during failover and catch-up all safe.
- **Payload.** Lifeguard's `#[notify(channel = "...")]` generates the trigger (lifeguard-migrate).
  The payload carries the key, never the row, so it stays far below Postgres's 8000-byte NOTIFY
  limit (a full-row payload makes the writing INSERT fail on wide rows):
  `{"table": "items", "op": "UPDATE", "id": 42, "v": <WAL position>, "ts": <ms since epoch>}`.
  `v` is monotonic per key (row locks order writes to one row); `ts` drives the lag metric and the
  catch-up watermark.
- **One active instance.** Each instance connects directly to Postgres (not through PgBouncer in
  transaction mode: LISTEN and session advisory locks need a dedicated session), issues `LISTEN`,
  then contends for `pg_try_advisory_lock(LIFEREFLECTOR_LOCK_KEY)`. The holder leads; the others
  stay connected and keep the keys they hear (bounded by `LIFEREFLECTOR_MAX_PENDING`). A dead
  leader's session ends, its lock is released, and a standby takes over within one heartbeat.
- **Batching.** The leader drains the notification queue, coalesces repeated keys and sends one
  `UNLINK` per batch (up to `LIFEREFLECTOR_BATCH_MAX` keys; several 1024-key commands are
  pipelined). Under load batches grow by themselves; at low rates each change goes out alone.
- **Gaps and catch-up.** After a successful flush the newest `ts` is the watermark, written to
  Redis (`LIFEREFLECTOR_WATERMARK_KEY`) at most once a second and on shutdown. Whenever the
  reflector may have missed changes (first start, reconnect, a take-over the standby's buffer does
  not cover, or a Redis outage long enough to overflow the pending set) it catches up:
  - tables listed as `table=column` (a `timestamptz` updated on every write): invalidate the rows
    whose column moved since `watermark - LIFEREFLECTOR_CATCHUP_SLACK_MS`. Blind to hard deletes,
    so only for tables that never hard-delete;
  - other listed tables: `SCAN` + `UNLINK` of `{prefix}{table}:*`;
  - no tables listed: everything under the prefix.
  A standby whose buffer reaches back past the old leader's watermark takes over without a
  catch-up and invalidates what it buffered instead.
- **Refuses `noeviction`.** The cache must be able to evict: with `noeviction`, a cache at
  `maxmemory` rejects the readers' refills. The reflector reads `maxmemory-policy` with `CONFIG GET`
  and refuses to start on `noeviction`; where `CONFIG` is disabled (managed Redis) the operator
  states the policy in `LIFEREFLECTOR_REDIS_EVICTION_POLICY`.
- **Shutdown.** Killing the process is safe: nothing is lost that the successor's catch-up does not
  cover. A clean stop flushes what is pending and writes the watermark.

## Known limit: the refill race

Invalidation-based caching has one window it cannot close by itself: a reader misses, reads row
version 5 from Postgres, a writer commits version 6, the reflector deletes the key, and only then
does the reader's `SET` of version 5 land. The cache holds version 5 until the next change or
the key's TTL. Readers should therefore always set a TTL (it bounds this and anything else
outside the design, such as a write that bypasses triggers). Where a few seconds of a stale row
matter, keep the TTL short for that entity or read it from Postgres.

## Configuration

| Variable | Default | |
|---|---|---|
| `LIFEREFLECTOR_PG_URL` | required | direct Postgres DSN |
| `LIFEREFLECTOR_REDIS_URL` | required | `redis://[:password@]host:port[/db]` |
| `LIFEREFLECTOR_CHANNELS` | `lifeguard_changes` | comma list of NOTIFY channels |
| `LIFEREFLECTOR_KEY_PREFIX` | `lifeguard:model:` | keys are `{prefix}{table}:{id}` |
| `LIFEREFLECTOR_TABLES` | none | catch-up list: `items, ledger=changed_at` |
| `LIFEREFLECTOR_WATERMARK_KEY` | `lifereflector:watermark` | |
| `LIFEREFLECTOR_CATCHUP_SLACK_MS` | 5000 | margin for clock skew and long transactions |
| `LIFEREFLECTOR_LOCK_KEY` | fixed | advisory lock key; one active reflector per key |
| `LIFEREFLECTOR_BATCH_MAX` | 512 | keys per batch |
| `LIFEREFLECTOR_BATCH_DELAY_MS` | 0 | extra wait to grow a batch (0: flush when the queue drains) |
| `LIFEREFLECTOR_POLL_US` | 500 | idle poll of the notification queue |
| `LIFEREFLECTOR_HEARTBEAT_MS` | 2000 | liveness check and standby lock retry |
| `LIFEREFLECTOR_MAX_PENDING` | 200000 | keys held while Redis fails before falling back to catch-up |
| `LIFEREFLECTOR_REDIS_EVICTION_POLICY` | none | operator-stated policy when `CONFIG GET` is refused |
| `LIFEREFLECTOR_METRICS_ADDR` | `0.0.0.0:9187` | `/metrics`, `/healthz`; `off` disables |

## Metrics

`lifereflector_notifications_total{table}`, `lifereflector_invalid_payloads_total`,
`lifereflector_coalesced_total`, `lifereflector_keys_invalidated_total`,
`lifereflector_keys_present_total`, `lifereflector_flushes_total{result}`,
`lifereflector_batch_keys`, `lifereflector_flush_seconds`, `lifereflector_lag_seconds`
(trigger time to invalidation done), `lifereflector_reconnects_total{target}`,
`lifereflector_catch_ups_total{mode}`, `lifereflector_catch_up_keys_total`,
`lifereflector_overflows_total`, `lifereflector_leader`, `lifereflector_pending_keys`,
`lifereflector_watermark_ms`.

## Runtime

`may` coroutines throughout (`may_postgres`, `may_redis`). The workspace patches `may` to the
microscaler fork that `may_redis` uses, so the process has one scheduler; see the root
`Cargo.toml`.

## Tests

Unit tests run with `cargo test -p lifeguard-reflector`. The end-to-end tests need a disposable
Postgres and Redis:

```sh
LIFEREFLECTOR_TEST_PG_URL=postgresql://postgres:postgres@127.0.0.1:5432/postgres \
LIFEREFLECTOR_TEST_REDIS_URL=redis://127.0.0.1:6379 \
cargo test -p lifeguard-reflector --test live
```

They cover batched invalidation, a single active leader with gapless failover to a standby,
ranged and flush catch-up after downtime, and the `noeviction` refusal.
