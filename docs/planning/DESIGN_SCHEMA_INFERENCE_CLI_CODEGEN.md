# Design: Schema inference — CLI and codegen boundary

**Status:** Companion to [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md §5](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md). **v0** is implemented; this doc fixes the **contract** between introspection, the CLI, and how generated Rust is consumed.

## Goals

- Make explicit what **`infer-schema`** guarantees vs what remains **human review**.
- Separate **`lifeguard-migrate`** (operational tool + introspection) from **`lifeguard-derive`** (compile-time codegen on structs you already wrote).
- Avoid implying that inference **replaces** entity authoring or migration generation without an explicit product decision.

## Current architecture (v0)

| Layer | Crate / binary | Responsibility |
|--------|----------------|----------------|
| Introspection | `lifeguard_migrate::schema_infer` | Query PostgreSQL `information_schema` (and related catalogs), map types conservatively, emit Rust source **as text** via `infer_schema_rust` → `emit_inferred_rust`. **Golden tests** lock emitter output under `lifeguard-migrate/tests/golden/` (no live DB required). |
| CLI | `lifeguard-migrate` subcommand **`infer-schema`** | Parse `--database-url` / env (`DATABASE_URL`, `LIFEGUARD_DATABASE_URL`), `--schema`, repeatable `--table`; connect via `may_postgres`, call `infer_schema_rust`, print or write output. |
| Consumption | Application / examples | Teams **copy, review, and commit** emitted `LifeModel` / `LifeRecord` modules into their crate (e.g. `examples/entities`). No automatic merge into `lifeguard-codegen` today. |

**Codegen boundary:** Inference outputs **Rust source strings** that are **compatible** with `#[derive(LifeModel, LifeRecord)]` and existing column attributes. It does **not** invoke `lifeguard-derive` or `lifeguard-codegen` at runtime. The derive macros run later, when the pasted source is compiled.

## What is intentionally out of scope for v0

- **Bidirectional sync** (DB change → Rust → DB) as a single command.
- **Watch mode** / CI diff gates (PRD stretch; may build on stable sort + golden files).
- **Emitting migrations** from inferred models — migration SQL continues to flow from entity definitions + `lifeguard-migrate` generators, not from `infer-schema` alone.

## Type mapping policy

- **Conservative:** unknown PostgreSQL types → omit column with `// OMITTED:` (see `schema_infer.rs` and PRD SI-2).
- **Composite primary keys:** emitted with `TODO` comments; single-column PKs get `#[primary_key]`.
- **Versioning:** mapping tables live in code; when extending types, update tests and this doc’s PRD cross-reference.

## Safety and configuration

- Follow the same **no credential logging** rules as other `lifeguard-migrate` paths (PRD SI-5).
- Connection strings come from flags or env — document in `lifeguard-migrate` README / `--help`, not in generated Rust.

## Future integration options (not committed)

1. **`infer-schema --out-dir`** writing one file per table under a configured module tree (still review-first).
2. **Golden tests** in CI: fixed SQL fixture schema → snapshot of emitted Rust (SI-1 acceptance).
3. **Optional pipeline** to `lifeguard-codegen` if we introduce an intermediate IR (JSON/schema DDL) — would be a separate design.

## References

- [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md §5](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md)
- Implementation: `lifeguard-migrate/src/schema_infer.rs`, `lifeguard-migrate/src/main.rs` (`infer-schema`).
