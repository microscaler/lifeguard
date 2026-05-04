# Query builder, `SelectQuery`, and `ActiveModel`

- **Status**: `verified`
- **Source docs**: [`docs/planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md`](../../planning/lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md)
- **Code anchors**: `lifeguard/src/query/`, `lifeguard/src/active_model/`
- **Last updated**: 2026-04-17

## What it is

**`SelectQuery`** (and related APIs under `lifeguard::query`) builds **SeaQuery**-backed SQL for PostgreSQL. **`ActiveModelTrait`** implements insert/update with optional **validators** (`ValidateOp`, aggregate vs fail-fast strategies).

## Key ideas

- **Prefer this over raw SQL:** default to `SelectQuery` and typed models. Raw SQL helpers exist but are **last resort** — see [`raw-sql-vs-selectquery-policy.md`](./raw-sql-vs-selectquery-policy.md).
- **Scopes**: named filters (`#[scope]`, `scope_bundle`) — see [`relations-loaders-scopes.md`](./relations-loaders-scopes.md).
- **Streaming / pagination**: cursor and stream helpers live under `query` — verify signatures in rustdoc when changing behavior.
- **Validators**: PRD [`PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md`](../../planning/PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md).

## Cross-references

- [`entities/life-model-and-life-record.md`](../entities/life-model-and-life-record.md)
- [`session-identity-map.md`](./session-identity-map.md)
