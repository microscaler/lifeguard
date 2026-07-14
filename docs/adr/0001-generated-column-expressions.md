# ADR 0001: Compile-time expressions for PostgreSQL generated columns

- Status: Accepted
- Date: 2026-07-14
- Scope: `#[generated_always_as = "<expression>"]`

## Context

PostgreSQL requires the defining expression for a generated column in its DDL:
`GENERATED ALWAYS AS (<expression>) STORED`. SeaQuery accepts that expression as
a custom expression, while a Rust derive attribute cannot carry a SeaQuery
builder value. Lifeguard already has the same compile-time boundary for schema
metadata such as `default_expr`, checks, and expression indexes.

Without generated-column metadata, an application must maintain hand-written
DDL that can drift from its `LifeModel`, or mark the field readonly while leaving
entity-driven schema generation unable to reproduce the table.

## Decision

Lifeguard accepts a SQL expression string only as compile-time model metadata:

```rust
#[generated_always_as = "lower(email)"]
pub normalized_email: String,
```

The derive validates that the expression is non-empty, caps it at 64 KiB, makes
the field implicitly readonly, and rejects a simultaneous default. Runtime
schema creation and `lifeguard-migrate` emit the same PostgreSQL generated-column
clause.

This is not a runtime query API. Request data, tenant data, and other dynamic
values must never be interpolated into the attribute. The application developer
and normal migration review remain responsible for the expression's SQL safety,
immutability, and compatibility with the target PostgreSQL version.

## Alternatives considered

- Hand-written migrations: rejected because they split the schema source of
  truth and prevent deterministic entity-driven generation.
- A fixed expression enum: rejected because PostgreSQL generated expressions
  legitimately use application-specific columns and immutable functions.
- A procedural Rust builder in the attribute: not representable as stable
  compile-time metadata consumable by both derive output and migration tooling.

## Consequences

- Generated columns round-trip through entity metadata and both migration paths.
- The expression is deliberately a trusted developer boundary, not a bindable
  value or an end-user input surface.
- Compile-time validation catches empty, oversized, and default-conflicting
  declarations; PostgreSQL remains the authority for expression validity.
