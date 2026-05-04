# Raw SQL vs `SelectQuery` — entrypoints and policy

- **Status**: `verified` (policy); `partially-verified` (API surface — confirm in rustdoc)
- **Source docs**: [`src/raw_sql.rs`](../../../src/raw_sql.rs), [`src/lib.rs`](../../../src/lib.rs) re-exports, [`reference/workspace-and-module-map.md`](../reference/workspace-and-module-map.md)
- **Code anchors**: [`src/connection/`](../../../src/connection/), [`src/executor/`](../../../src/executor/), [`src/raw_sql.rs`](../../../src/raw_sql.rs), [`src/query/`](../../../src/query/)
- **Last updated**: 2026-04-17

## Default path: idiomatic ORM

Use **`SelectQuery`**, **`LifeModel` / `LifeRecord`**, relations, scopes, validators, and other **`lifeguard::query`** APIs for application data access. That is the **normal** and **expected** path for product code.

See [`query-select-and-active-model.md`](./query-select-and-active-model.md) and [`entities/life-model-and-life-record.md`](../entities/life-model-and-life-record.md).

## Raw SQL and low-level helpers — last resort

The crate exposes **raw SQL helpers** (for example `execute_statement`, `execute_unprepared`, `find_by_statement`, `find_all_by_statement`, `query_value`) from [`src/raw_sql.rs`](../../../src/raw_sql.rs), re-exported at the crate root in [`src/lib.rs`](../../../src/lib.rs). **`connection`** and **`executor`** are the lower layers those helpers sit on.

**Policy:**

1. **Raw SQL is a last resort.** Do not introduce it for convenience, micro-optimization, or to skip modeling work.
2. **Human approval is required** before any new raw-SQL path lands in a codebase that consumes Lifeguard (no agent-only or drive-by adoption).
3. **A comprehensive ADR is required** that demonstrates, with specifics, that the use case **cannot** be met by **extending Lifeguard** with **new idiomatic ORM functionality** (query builder, derive attributes, migration/compare-schema, or other first-class APIs). The ADR must justify why raw SQL is the remaining option and how risks (injection, drift from schema, untyped rows) are mitigated.

Until that bar is met, **extend the ORM or open a design discussion** instead of embedding raw strings.

## Security and maintenance note

Raw SQL concentrates **injection** and **schema drift** risk on the caller. [`SECURITY_PROMPT.md`](../../../SECURITY_PROMPT.md) treats query construction as an audit theme; prefer typed, generated, or builder-backed SQL.

## Cross-references

- [`reference/workspace-and-module-map.md`](../reference/workspace-and-module-map.md)
- [`query-select-and-active-model.md`](./query-select-and-active-model.md)
- [`entities/transaction-boundaries.md`](../entities/transaction-boundaries.md) — transactions wrap executor usage
