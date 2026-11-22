# Work package: database connections with SPIFFE client certificates (Zero Trust, stream C)

**Status:** ready to start (19 Nov 2026). **Owner:** Charles Sibbald. **Executor:** a Claude agent on a Mac mini.

This is stream C of the Zero Trust work package. The master plan, decisions, order of work and rules are in
BRRTRouter: [`docs/EPICS/ZERO_TRUST/WORK_PACKAGE-SPIFFE-mTLS.md`](https://github.com/microscaler/BRRTRouter/blob/main/docs/EPICS/ZERO_TRUST/WORK_PACKAGE-SPIFFE-mTLS.md).
PriceWhisperer's side (chart, rollout, pwdb) is [`docs/plans/spiffe-mtls-service-identity.md`](https://github.com/microscaler/PriceWhisperer/blob/main/docs/plans/spiffe-mtls-service-identity.md).
Until their pull requests merge, the BRRTRouter and Lifeguard documents are on the branch `docs/zero-trust-work-package`.

Read [`AGENTS.md`](../AGENTS.md) and [`docs/llmwiki/`](llmwiki/) first; Lifeguard's rules apply on top of these.

## Goal

Every Lifeguard connection to Postgres (through PgBouncer or direct) can present the service's X.509 SVID as its client
certificate, verify the server, and keep working while the certificate rotates every day, with no restart and no failed
query. Today pwdb's plan (gcp-infrastructure ADR 0012) is client certificates plus passwords for every connection; this
work lets those certificates be the services' SPIFFE identities (decision D7 in the master plan) and makes the pool safe
for short-lived certificates.

## What exists

- **may_postgres** (Microscaler fork, `integration/microscaler-fork`, pinned by tag `v0.1.1` in `Cargo.toml:45`) already
  does TLS with rustls 0.23: `sslmode` (`disable`, `prefer`, `require`, `verify-ca`, `verify-full`), `sslrootcert`,
  `sslcert`, `sslkey`, from the connection string (`src/config.rs:410-421`) or the `Config` builder
  (`ssl_mode`, `ssl_root_cert`, …). Certificate files are read at each connect (`src/tls.rs` `client_config`), so a
  new connection picks up a rotated file. Tests: `tests/tls.rs`.
- **Lifeguard** parses the connection string into `may_postgres::Config` (`src/connection.rs:188`), so the parameters
  pass through today. The pool (`src/pool/pooled.rs`) notes that a TLS reconnect needs a 256 KiB stack
  (`pooled.rs:1208-1211`). Replica connections in `src/pool/wal.rs:134,196` connect with their own strings.
- Nothing in Lifeguard knows a connection is TLS, which certificate it used, or when that certificate expires.

## Stories

| ID | Story | Size | Acceptance |
|---|---|---|---|
| C1 | **First-class TLS configuration.** A `TlsConfig` on the pool configuration (mode, root bundle, client certificate and key paths, optional expected server SPIFFE ID), applied to the primary, replica (`wal.rs`) and LISTEN connections alike; the connection string still works and the explicit setting wins. Validation at pool start: certificate and key present and matching, mode needs a root bundle where it must | S | Positive and negative tests per field; a bad path fails the pool start with a clear error, never a panic |
| C2 | **Rotation.** The pool tracks the client certificate's identity (file mtime + SHA-256 of the DER) and its `notAfter`. When the file changes, new connections use it and existing ones are recycled when they are next idle (no in-flight query is cut); a connection also retires before its certificate expires. A hard ceiling on connection age (default 12 h) covers missed changes | M | Rotation test with rcgen certificates against a Postgres container with `hostssl … clientcert=verify-full`: a load loop sees no error across two rotations; after rotation no connection still uses the old certificate |
| C3 | **Server SPIFFE check (may_postgres).** An option beside `verify-full`: verify the chain against the bundle and require the server certificate's URI SAN to equal an expected SPIFFE ID (for pwdb's servers, which have DNS names that differ between the pooler and the primary). A single-purpose PR on the may_postgres fork; upstream only if the maintainer wants it | M | Wrong ID, missing URI SAN, two URI SANs: refused before authentication; existing modes unchanged |
| C4 | **Metrics.** Handshake duration histogram, failures by reason (`certificate_expired`, `unknown_ca`, `bad_certificate`, `server_identity`), the client certificate's seconds to expiry as a gauge, connections recycled for rotation; exposed through the existing pool scrape (`prometheus_scrape_text`) without a lock on the query path | S | Gauges present in the scrape; a failed handshake increments its reason |
| C5 | **Stack and coroutine safety.** Every connect path that may perform a TLS handshake runs with the 256 KiB stack (`pooled.rs:1208`); add a test that a TLS reconnect on a default-sized coroutine is not reachable | S | Test; documentation in `docs/POOLING_OPERATIONS.md` |
| C6 | **LifeReflector** (repository `microscaler/lifeguard-reflector`, a consumer of this pool). Its LISTEN connection (direct primary, session mode) uses the same `TlsConfig` and rotation; its reconnect keeps its position in the notification stream | S | Reflector test across a rotation |
| C7 | **Documentation.** `docs/llmwiki` page "TLS and client certificates", `docs/POOLING_OPERATIONS.md` section, CHANGELOG | S | Linked from the wiki index |

## Order

C1 and C2 in wave 1 (they need nothing from BRRTRouter); C3 and C4 in wave 2; C5–C7 before the release. Release by
tagging the CI-tested commit (annotated tag + GitHub Release notes, no version-bump commit), after a may_postgres tag
if C3 changed it. PriceWhisperer consumes the Lifeguard tag in its wave 5 (see its package, D6).

## Notes for pwdb (gcp-infrastructure, context only)

- PgBouncer runs `client_tls_sslmode = verify-full` against the CA: it proves the client holds one of the platform's
  certificates; the password still proves the login (ADR 0012 §3).
- Direct connections use `hostssl … scram-sha-256 clientcert=verify-full`, which matches the certificate's common name
  to the login. Logins rotate between `_a` and `_b` slots (ADR 0010), so the certificate cannot name one slot: pwdb needs
  an ident map from the SVID (its CN or URI SAN) to both slots. That change is stream E in the PriceWhisperer package,
  not here; C2's tests should use a map the same way.

## Rules

As in the master plan §9. In particular: branch → pull request, one purpose per PR; may_postgres changes go on
Microscaler's fork first; private keys only from the test CA in tests, never committed; nothing printed or logged from a
key.
