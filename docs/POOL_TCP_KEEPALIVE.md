# PostgreSQL TCP keepalive (Lifeguard / libpq URLs)

Lifeguard uses **`may_postgres`** (libpq-compatible connection strings). For long-lived pool
connections, operators often want **TCP keepalives** so firewalls and NATs do not silently drop
idle sessions (PRD R4.1).

## URI parameters

Append these as query parameters on `postgres://` / `postgresql://` URLs (values are illustrative):

```
postgres://user:pass@host:5432/db?keepalives=1&keepalives_idle=60&keepalives_interval=10&keepalives_count=5
```

| Parameter | Meaning |
|-----------|---------|
| `keepalives` | `1` to enable TCP keepalive on the socket |
| `keepalives_idle` | Seconds of idle time before the first keepalive probe |
| `keepalives_interval` | Seconds between keepalive probes |
| `keepalives_count` | Failed probes before the connection is considered dead |

Exact behavior follows **libpq** and the OS TCP stack. Tune with your network team and Postgres
`idle_session_timeout` / firewall idle limits.

## Relationship to pool idle liveness

**TCP keepalive** detects dead transports. The pool’s **`idle_liveness_interval_ms`** (see
[`DatabaseConfig`](../src/pool/config.rs)) runs an application-level **`SELECT 1`** on idle workers
to validate the session before the next query (PRD R4.2). Use both when you need transport and
session-level checks.

## Environment

With file + env config, set **`LIFEGUARD__DATABASE__URL`** (or `url` in `config/config.toml`) to a
URL that already includes the query string above.
