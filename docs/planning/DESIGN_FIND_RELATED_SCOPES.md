# Design note: `find_related` and named scopes

**Status:** Documentation only (PRD Phase C follow-on).  
**PRD:** [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md §7](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md) (scopes, SC-1–SC-4).

## Current behavior (v0)

- [`SelectQuery::scope`](../../src/query/scope.rs) and [`SelectQuery::filter`](../../src/query/select.rs) apply predicates on the **root** entity’s `SELECT`.
- [`SelectQuery::scope_or`](../../src/query/scope.rs) / [`scope_any`](../../src/query/scope.rs) compose **OR** branches for that same root query (PRD SC-2).
- [`FindRelated`](../../src/relation/mod.rs) and loaders build **join** / **subselect** paths; their `WHERE` clauses target related tables via relation metadata, not via the parent entity’s scope helpers.

## Product question

When loading related rows (e.g. `post.find_related(Comment)`), should **parent scopes** (e.g. `Post::published()`) automatically constrain which parents participate, while **child scopes** (e.g. `Comment::visible()`) apply only to the related table?

## Recommended default (when implemented)

1. **Root `SelectQuery` scopes** apply to the root entity only (current behavior).
2. **`find_related`**: document explicitly whether filters on the parent query are **re-used** for the relation SQL. Default safe choice: **only primary-key / join keys** flow into the relation query unless an API adds `related_scope(...)` / `with_scope_on_related(...)`.
3. **Eager loaders** (`Loader*`): same rule—avoid silently ANDing unrelated table scopes onto join SQL without an explicit API, to prevent surprising cartesian restrictions.

## Next implementation steps (optional)

- Add examples under `examples/` showing `find_related` + manual `filter` on the returned query type if the API allows chaining.
- If product wants **inherited scope**, add a dedicated method (name TBD) on the relation builder so call sites opt in.
- Cross-link this doc from [`SEAORM_LIFEGUARD_MAPPING.md`](./lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md) when a behavior ships.
