# Decision: table-qualified columns in `build_where_condition`

**Status:** Accepted (Phase A — A4)  
**Date:** 2026-03-28  
**Code:** `lifeguard::relation::def::condition::build_where_condition`

## Context

`build_where_condition` emits equality predicates for `find_related` / `LazyLoader` using `sea_query::Expr::cust` with a string of the form `{table}.{column} = {value}`.

## Decision

**`build_where_condition` (find_related / LazyLoader)** uses `sea_query::Expr::col` with:

- `ColumnName(Some(table_name.clone()), column)` when `to_tbl` is `TableRef::Table(_, None)` (same `TableName` as in `FROM`, including optional schema).
- `(alias, column)` when `to_tbl` carries a table alias.

Non-`Table` `TableRef` variants still fall back to `Expr::cust` + `extract_table_name` (rare).

**Join helpers** (`join_tbl_on_condition`, `join_tbl_on_expr`) still use `Expr::cust` with string-concatenated identifiers; revisit separately if joins need the same guarantees.

## Alternatives considered

- **Full migration of join paths:** Same `ColumnName` approach for `JOIN ON`; deferred to keep this change scoped to the shared related-row filter.

## Follow-up

- Migrate `join_tbl_on_condition` / `join_tbl_on_expr` to structured column refs when join SQL needs schema/quoting parity.
- Revisit batched `.with()` loaders (same predicate builder stays centralized in `build_where_condition`).
