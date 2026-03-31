# Design note: `find_related` and named scopes

**Status:** Default behavior **documented in crate rustdoc** (`query::scope`, `FindRelated`). **Related-side opt-in:** [`FindRelated::find_related_scoped`](../../src/relation/traits.rs) applies a scope on the related `SelectQuery` in one call (same as `find_related()?.scope(…)`). **Inherited parent scopes** (merge parent `Post::find().scope(…)` predicates into `find_related` SQL) remain future work.  
**PRD:** [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md §7](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md) (scopes, SC-1–SC-4).

## Current behavior (v0)

- [`SelectQuery::scope`](../../src/query/scope.rs) and [`SelectQuery::filter`](../../src/query/select.rs) apply predicates on the **root** entity’s `SELECT`.
- [`SelectQuery::scope_or`](../../src/query/scope.rs) / [`scope_any`](../../src/query/scope.rs) compose **OR** branches for that same root query (PRD SC-2).
- [`FindRelated`](../../src/relation/mod.rs) and loaders build **join** / **subselect** paths; their `WHERE` clauses target related tables via relation metadata, not via the parent entity’s scope helpers.

## Product question

When loading related rows (e.g. `post.find_related(Comment)`), should **parent scopes** (e.g. `Post::published()`) automatically constrain which parents participate, while **child scopes** (e.g. `Comment::visible()`) apply only to the related table?

## Recommended default (**adopted for v0 documentation**)

1. **Root `SelectQuery` scopes** apply to the root entity only (current behavior).
2. **`find_related`**: filters on the parent entity’s **separate** `SelectQuery` (e.g. `Post::find().scope(...)`) are **not** merged into `find_related` SQL—only join/PK-driven `WHERE` from [`build_where_condition`](../../src/relation/def/condition.rs). Callers chain `.scope` / `.filter` on the `SelectQuery` returned by `find_related`, or use [`find_related_scoped`](../../src/relation/traits.rs) to apply a **related-side** scope in one step (still no parent-scope inheritance). **Inherited parent scopes** remain a separate, future opt-in if product needs them.
3. **Eager loaders** (`Loader*`): same rule—avoid silently ANDing unrelated table scopes onto join SQL without an explicit API, to prevent surprising cartesian restrictions.

## Next implementation steps (optional)

- After a write on the primary, **read-your-writes** on pooled executors: use `PooledLifeExecutor::with_read_preference(ReadPreference::Primary)` (see `src/pool/pooled.rs`) so `SELECT` paths do not hit a possibly stale replica (same applies to pooled reads right after `INSERT`/`UPDATE`).
- **~~Add examples~~** — **Done:** `tests/db_integration/related_trait.rs` — `test_find_related_chains_scope_on_related_query` chains [`.scope`](../../src/query/scope.rs) on the [`SelectQuery`](../../src/query/select.rs) returned from [`find_related`](../../src/relation/traits.rs) (parent scopes are still not merged; this constrains the **related** table only). Optional `examples/` binary can mirror the same pattern later.
- **Related-side one-call API** — **Done:** [`find_related_scoped`](../../src/relation/traits.rs); test `test_find_related_scoped_matches_chained_scope`.
- If product wants **inherited parent scope** (merge parent table predicates into `find_related` SQL), add a dedicated method or relation metadata so call sites opt in (highest integration risk).
- Cross-link: [`SEAORM_LIFEGUARD_MAPPING.md`](./lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md) scopes row points here for `find_related` + scopes semantics.
