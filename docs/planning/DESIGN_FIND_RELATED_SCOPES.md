# Design note: `find_related` and named scopes

**Status:** Default behavior **documented in crate rustdoc** (`query::scope`, `FindRelated`). **Related-side opt-in:** [`FindRelated::find_related_scoped`](../../src/relation/traits.rs) applies a scope on the related `SelectQuery` in one call (same as `find_related()?.scope(…)`). **Caller-side / parent-table opt-in:** [`FindRelated::find_related_parent_scoped`](../../src/relation/traits.rs) adds an **`INNER JOIN`** on `RelationDef::from_tbl` and ANDs predicates on **`Self::Entity`**’s table (same expressions as `Self::Entity::find().scope(…)`). **`has_many_through`** is rejected until supported. **Implicit** merge of arbitrary parent `SelectQuery` state into loaders / multi-hop joins remains future work.  
**PRD:** [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md §7](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md) (scopes, SC-1–SC-4); follow-on queue **expanded** in [PRD_FOLLOWON_NEXT_THREE.md](./PRD_FOLLOWON_NEXT_THREE.md) (G6 hygiene, `find_related` + scope **example surface**, inherited parent + loader merge).

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
- **~~Add examples~~** — **Done:** `tests/db_integration/related_trait.rs` — `test_find_related_chains_scope_on_related_query` …; **also** root **`examples/find_related_scope_example.rs`** (compile-only demo: `cargo check --example find_related_scope_example`).
- **Related-side one-call API** — **Done:** [`find_related_scoped`](../../src/relation/traits.rs); test `test_find_related_scoped_matches_chained_scope`.
- **~~Dedicated opt-in for parent-table predicates~~** — **done:** `find_related_parent_scoped` (see above). **Implicit** inheritance of a full parent `SelectQuery` / loader merge remains out of scope.
- Cross-link: [`SEAORM_LIFEGUARD_MAPPING.md`](./lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md) scopes row points here for `find_related` + scopes semantics.

---

## Appendix: Deferred behavior and how it would be used

This section records **roadmap** items that are **not** fully implemented today, with **usage** guidance so application authors know what to do now versus what a future API might enable.

### A. `compare-schema` — operator classes / full `CREATE INDEX` parity vs `pg_indexes`

**Today:** [`lifeguard-migrate` `compare-schema`](../../lifeguard-migrate/README.md) (and [`schema_migration_compare`](../../lifeguard-migrate/src/schema_migration_compare.rs)) reconcile **names** of columns appearing in a **best-effort** parse of [`pg_indexes.indexdef`](https://www.postgresql.org/docs/current/view-pg-indexes.html): btree **key** columns and optional **`INCLUDE`** lists where the parser recognizes them. Expression or functional keys are skipped when parsing fails.

**Not compared:** Access method / **operator class** (e.g. `USING gist`, `jsonb_path_ops`, `text_pattern_ops`), per-column **collation**, **NULLS FIRST/LAST**, or arbitrary **expression** keys as first-class equality. Two indexes can differ only in those dimensions while sharing the same column names — **`compare-schema` may not report drift** for that gap.

**How full parity would be used:** Run in **CI or release gates** so a DBA change in production (e.g. switching to `GIN` + `jsonb_path_ops`) fails the check unless merged migrations (or a future canonical index IR) encode the same intent. Teams that need strict **index equivalence** today should add **manual review**, **`pg_dump` diff**, or **Postgres event triggers** outside Lifeguard until this roadmap item lands.

**Where to read more:** [`lifeguard-migrate/README.md` § `compare-schema` — Limits and roadmap](../../lifeguard-migrate/README.md#compare-schema-limits-and-roadmap-index-comparison).

---

### B. `find_related_parent_scoped` and `has_many_through`

**Today:** [`FindRelated::find_related_parent_scoped`](../../src/relation/traits.rs) supports direct edges (`through_tbl` is `None`). If [`RelationDef::through_tbl`](../../src/relation/def/struct_def.rs) is set (**many-to-many** / join table), the method returns an error: **`has_many_through` is not supported yet**.

**How support would be used:** Same **opt-in** meaning as today — “related rows for this model instance, **and** an extra predicate on the **caller** (or join) side” — but for paths like **User → `user_roles` → Role**. Example intent: *tags for this user only if the membership row satisfies X*. Implementation would build the correct **chain of `JOIN`s** (or `EXISTS`) over `through_tbl` / `through_*_col` and then AND the caller’s `IntoCondition`, instead of a single `INNER JOIN` on `from_tbl` only.

**What to do until then:** Use **`find_related`** / **`find_related_scoped`** on the entity that is actually in `FROM`, or compose a **`SelectQuery`** with explicit **`inner_join` / `filter`** for the join table; see integration tests and [`build_where_condition`](../../src/relation/def/condition.rs) orientation docs.

---

### C. Implicit parent-`SelectQuery` / loader merge

**Today:** Filters on **`User::find().scope(…)`** apply only to that **`SelectQuery<User>`**. They are **not** automatically merged into **`user.find_related::<Post>()`**, and they are **not** implicitly merged into **eager loaders**. That avoids surprising SQL when a scoped parent list and a relation load are composed without an explicit API.

**How implicit merge would be used (if ever added):** A hypothetical API might mean: “load posts for **exactly** the users matched by this scoped parent query,” or “when batch-loading related rows, **inherit** the parent query’s `WHERE` without repeating `.scope` on every child query.” That reduces boilerplate but increases **integration risk** (duplicate predicates, join order, performance).

**What to do today:** Use **`find_related_parent_scoped`** for **explicit** caller-side predicates on **`Self::Entity`**’s table; use **`find_related_scoped`** for **related-table** predicates; chain **`.scope` / `.filter`** on the `SelectQuery` returned by **`find_related`**; or build **manual joins** for multi-hop cases. See the [Recommended default](#recommended-default-adopted-for-v0-documentation) section above.
