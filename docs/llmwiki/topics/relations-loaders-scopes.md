# Relations, loaders, `find_related`, scopes

- **Status**: `verified`
- **Source docs**: [`docs/planning/DESIGN_FIND_RELATED_SCOPES.md`](../../planning/DESIGN_FIND_RELATED_SCOPES.md), [`docs/planning/DESIGN_INHERITED_PARENT_SCOPES_SPIKE.md`](../../planning/DESIGN_INHERITED_PARENT_SCOPES_SPIKE.md)
- **Code anchors**: `lifeguard/src/relation/`, `lifeguard/src/query/loader.rs`, `lifeguard/src/query/scope.rs`
- **Last updated**: 2026-04-17

## What it is

Lifeguard implements **SeaORM-like** relation traits (`Related`, `FindRelated`, `FindLinked`, loaders) with **scoped** parent/child queries. Composite keys, schema-qualified table names, and **empty `Linked::via()`** are edge cases documented in design docs and rustdoc.

## Gotchas

- **WHERE expr** building for joins: see `RELATION_WHERE_EXPR_DECISION` and audits under [`docs/planning/audits/`](../../planning/audits/).
- **Inherited parent scopes** — spike completed; behavior summarized in DESIGN_INHERITED_PARENT_SCOPES_SPIKE.

## Cross-references

- [`query-select-and-active-model.md`](./query-select-and-active-model.md)
- [`session-identity-map.md`](./session-identity-map.md)
