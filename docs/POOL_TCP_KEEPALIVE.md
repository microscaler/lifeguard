# PostgreSQL TCP options: NODELAY and keepalive

Pool connections are long-lived. Two TCP settings matter for them (PRD R4.1):

- **TCP_NODELAY.** Lifeguard pipelines statements: a unit writes `BEGIN`, its statements and `COMMIT` without waiting between them, and a read-only commit isn't awaited. With Nagle on, the next small write waits for the server's delayed ACK, which adds roughly one round trip to back-to-back units.
- **Keepalive.** Cloud NAT and conntrack drop idle TCP flows without notice, after about 10 minutes on GCP. Without keepalive, the pool only finds out on the next query, which then fails.

## Defaults

The `may_postgres` fork that lifeguard pins enables both:

| Setting | Default |
|---|---|
| `tcp_nodelay` | on |
| `keepalives` | on |
| `keepalives_idle` | 60 s |
| `keepalives_interval` | 10 s |
| `keepalives_retries` | 5 |

With these defaults a dead peer is detected in about 110 s, and probes keep idle flows alive well inside a 10-minute reap window.

## Overriding

Overrides apply in this order, and the later one wins:

1. The driver defaults above.
2. Connection-string keys on the URL, for example `postgres://user:pass@host:5432/db?keepalives_idle=30&tcp_nodelay=0`. In key-value form, write `keepalives_idle=30`.
3. Pool settings (`LifeguardPoolSettings::tcp`, a `TcpOptions` value), loaded from `[database]` in `config/config.toml` or from the environment:

| `[database]` key | Env var |
|---|---|
| `tcp_nodelay` | `LIFEGUARD__DATABASE__TCP_NODELAY` |
| `tcp_keepalives` | `LIFEGUARD__DATABASE__TCP_KEEPALIVES` |
| `tcp_keepalives_idle_seconds` | `LIFEGUARD__DATABASE__TCP_KEEPALIVES_IDLE_SECONDS` |
| `tcp_keepalives_interval_seconds` | `LIFEGUARD__DATABASE__TCP_KEEPALIVES_INTERVAL_SECONDS` |
| `tcp_keepalives_retries` | `LIFEGUARD__DATABASE__TCP_KEEPALIVES_RETRIES` |

If a key is unset, the connection string or the driver default applies.

The driver uses `keepalives_retries`, which is the tokio-postgres name. The libpq name `keepalives_count` is **not** accepted.

## Relationship to pool idle liveness

**TCP keepalive** detects a dead transport in the kernel and costs nothing on the wire beyond the probes. The pool's **`idle_liveness_interval_ms`** (see [`DatabaseConfig`](../src/pool/config.rs)) runs an application-level `SELECT 1` on idle workers (PRD R4.2). With keepalive on, idle probes are usually unnecessary. Keep them only if you need to detect a session that is up at the TCP level but unusable at the server.


## Pre-send retry

A pinned unit (an RLS statement, a `with_session_batch`, a session transaction's first write, a batch write's `BEGIN`)
is retried **once** on a fresh connection when its connection is found dead **before anything
was sent**:

| Stage | When |
|---|---|
| `checkout` | The connection is already known dead when the unit takes the slot (marked broken, or its I/O coroutine exited because the server or the network closed it while idle). The slot is healed before the unit starts. |
| `send` | Preparing a statement (no effect on the server) or handing the unit's first message to the connection fails with a connectivity error. |

Once the unit's first message has been accepted for sending, a failure is **returned, never
retried**: the server may have applied a write. A unit cut short by the reply deadline is never
retried, and no unit is retried more than once. The fresh connection runs the whole unit again
(`BEGIN`, `rls_set_session`, statements), so the RLS context is always set on the connection that
runs the work.

Retries are counted in `lifeguard_pool_presend_retry_total{unit, stage}` (`unit`: `rls_unit`,
`batch` or `session_transaction`) and logged at `warn`. With keepalive on, most connections that
die while idle are already marked closed at checkout; these retries turn that into a successful unit
instead of an error to the caller.

### Standalone statements (shared slots)

A statement run without a session context (on a shared slot) follows the same rule. A
connectivity error always heals the slot, and the statement runs once more on the fresh
connection only when that cannot apply it twice:

| Stage | When |
|---|---|
| `checkout` | The slot's connection was already dead before the statement started. |
| `send` | Preparing it or handing over its message failed: nothing reached the server. |
| `postsend_readonly` | It was sent, but it is a plain `SELECT`: starts with `SELECT`, no row locks (`FOR UPDATE` / `FOR SHARE` …), no `SELECT … INTO`, no known side-effecting built-in (sequences, `pg_notify`, advisory locks, `set_config`, large objects). Anything else, including a leading comment, is not retried. A `SELECT` that calls a user function with side effects cannot be told apart from the text; avoid relying on this retry for such statements. |

Any other failure after the statement was sent is returned (a write may have been applied). These
retries count under `unit="shared_statement"`. Batch writes in `Bisect` mode no longer re-run a
chunk after a dropped connection or reply timeout, for the same reason.
