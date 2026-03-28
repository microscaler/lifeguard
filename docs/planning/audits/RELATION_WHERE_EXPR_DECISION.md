# Decision: table-qualified columns in `build_where_condition`

**Status:** Accepted (Phase A — A4)  
**Date:** 2026-03-28  
**Code:** `lifeguard::relation::def::condition::build_where_condition`

## Context

`build_where_condition` emits equality predicates for `find_related` / `LazyLoader` using `sea_query::Expr::cust` with a string of the form `{table}.{column} = {value}`.

## Decision

**Keep `Expr::cust` for now**, with these rules:

1. Table names come from `extract_table_name` in `src/relation/def/condition.rs` on `RelationDef::to_tbl` (not `Debug` formatting).
2. Column names come from `Identity` / `DynIden` `to_string()` as used elsewhere in join building.
3. **Reserved identifiers, mixed-case identifiers, and schema-qualified table names** are not guaranteed unless callers use names that Postgres accepts unquoted; this matches typical snake_case table/column usage in Lifeguard today.

## Alternatives considered

- **`Expr::col((table, column))` with typed refs:** Preferable long-term for quoting and schema safety; requires a stable mapping from `TableRef` + `Identity` into `sea_query` column refs across all backends. Deferred until a concrete need (e.g. `"order"` as table name) or multi-schema support.

## Follow-up

- If a production entity uses a reserved word, add a focused integration test and either quote in `cust` or migrate that predicate to `Expr::col`.
- Revisit when implementing batched `.with()` loaders (same predicate builder should stay centralized).
