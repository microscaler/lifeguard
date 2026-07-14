# ADR 0002: RLS session injection is an executor protocol

- Status: accepted
- Date: 2026-07-14
- Decision owners: Microscaler RLS delivery

## Context

PostgreSQL RLS requires request identity to be installed on the exact connection and transaction
that executes protected application queries. Lifeguard supports direct clients, pooled worker
threads, explicit transactions, and ORM operations over `LifeExecutor`. A consumer-side wrapper
cannot safely guarantee that all pooled statements use one physical connection.

The operation uses fixed SQL for PostgreSQL transaction control and one fixed, schema-qualified
function call. It does not select application rows, construct identifiers, deserialize untyped
results, or replace `SelectQuery`/model APIs.

## Alternatives considered

### Model or `SelectQuery`

Rejected. `BEGIN`, `COMMIT`, `ROLLBACK`, transaction-local GUC state, and function invocation
before arbitrary ORM statements are connection lifecycle operations, not table queries. They
cannot be represented as a `LifeModel`, relation, scope, validator, or select expression.

### Extend the query builder with RLS expressions

Rejected. A query expression cannot pin a pool worker or ensure context is injected before every
statement. Adding identity clauses to generated SQL would also be weaker than database RLS and
would duplicate policy logic.

### Add a Sesame-specific executor wrapper

Rejected. It duplicates the executor hierarchy, couples Lifeguard to an identity product, and
still cannot control the underlying pool slot without a base pool capability.

## Decision

RLS context injection is a first-class optional protocol on Lifeguard's base executors:

- `Option<SessionContext>` enables contextual one-shot execution; `None` retains autocommit.
- `LifeguardPool::with_session_transaction` pins the existing primary executor for
  multi-statement work.
- Lifeguard calls only the constant SQL entry point
  `public.rls_set_session($1::text, $2::uuid, ..., $8::text)` with bound values.
- The application owns the database function and its `sesame.*` GUC mapping.
- Context injection and application work share a transaction. Injection failure, returned error,
  or unwinding rolls back before the connection is released.

## Risk controls

- The SQL string and schema/function identifiers are compile-time constants; callers cannot
  interpolate identifiers or values.
- Every value is a typed bind parameter (`uuid`, `text`, or `jsonb`).
- No JWT, credential, or bearer payload reaches PostgreSQL.
- `SessionContext` requires the hard tenant, subject, active organization, and session boundary.
- Debug output redacts identifiers and session values.
- Live integration tests use a non-owner role with RLS enabled and prove fail-closed behavior,
  pool-slot reuse, commit, rollback, application error, panic, and missing-helper failure.

## Consequences

This narrow fixed SQL is allowed at the executor layer. It does not authorize application code to
add raw SQL data-access paths; those remain subject to the raw-SQL policy and separate approval.
