# Workspace crates and `lifeguard` module map

- **Status**: `verified`
- **Source docs**: root [`README.md`](../../../README.md), [`Cargo.toml`](../../../Cargo.toml)
- **Code anchors**: [`src/lib.rs`](../../../src/lib.rs)
- **Last updated**: 2026-04-17

## Workspace members (typical)

| Crate | Role |
|-------|------|
| `lifeguard` | Core ORM: `LifeModel`/`LifeRecord`, `SelectQuery`, `LifeExecutor`, `LifeguardPool`, relations, session, migrations API. |
| `lifeguard-derive` | Procedural macros: `#[derive(LifeModel, LifeRecord)]`, `#[scope]`, attributes on fields/tables. |
| `lifeguard-migrate` | CLI: schema inference, SQL generation, `compare-schema`, dependency ordering (`sql_dependency_order`). |
| `lifeguard-codegen` | Supporting codegen utilities used by tooling. |
| `lifeguard-reflector` | Optional NOTIFY-driven cache refresh adjunct (see reflector topic). |

Integration tests and examples live under `tests-integration/`, `examples/`, `tests/`.

## `lifeguard` crate — top-level modules

Declared in [`src/lib.rs`](../../../src/lib.rs) (non-exhaustive; see file for re-exports):

| Module | Concern |
|--------|---------|
| `config`, `connection` | Connection strings and health checks. |
| `executor` | `LifeExecutor`, `MayPostgresExecutor`, `LifeError`. |
| `raw_sql` | Helpers over `LifeExecutor` (`execute_statement`, `find_by_statement`, …). **Product policy:** prefer `query` / `SelectQuery`; raw SQL only per [`topics/raw-sql-vs-selectquery-policy.md`](../topics/raw-sql-vs-selectquery-policy.md). |
| `pool` | `LifeguardPool`, `PooledLifeExecutor`, `ReadPreference`, WAL lag routing. |
| `query` | `SelectQuery`, `ColumnTrait`, `LifeModelTrait`, indexes, scopes. |
| `active_model` | `ActiveModelTrait`, validators, `ValidateOp`. |
| `model` | `ModelTrait`, `TryIntoModel`. |
| `session` | Identity map, `Session`, dirty notifications. |
| `relation` | `FindRelated`, `RelationDef`, loader helpers. |
| `partial_model` | Partial selects. |
| `transaction` | `Transaction`, isolation. |
| `migration` | Runtime migrator types (`Migrator`, locks) when embedded in apps. |
| `metrics`, `logging` | Optional Prometheus / channel logging. |
| `cache` | Cache provider traits for coherence patterns. |
| `test_helpers` | Integration test DB helpers. |

## Cross-references

- [`entities/life-model-and-life-record.md`](../entities/life-model-and-life-record.md)
- [`entities/life-executor-pool-and-routing.md`](../entities/life-executor-pool-and-routing.md)
- [`../../../../BRRTRouter/llmwiki/reference/codebase-entry-points.md`](../../../../BRRTRouter/llmwiki/reference/codebase-entry-points.md) — HTTP stack in sibling router.
