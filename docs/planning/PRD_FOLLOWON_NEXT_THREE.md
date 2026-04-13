# PRD follow-on: next three items (expanded)

**Purpose:** Expand [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md §0.3](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md) **follow-on priority** rows **1**, **2**, and **4** the same way we expanded deferred semantics in [DESIGN_FIND_RELATED_SCOPES.md](./DESIGN_FIND_RELATED_SCOPES.md) (usage today vs future, risks, how it would be used). **No implementation commitment** — this is planning and teaching material before any build.

**PRD rows covered**

| Order | §0.3 row | This doc |
|------:|----------|----------|
| 1 | Phase E — mapping / docs | [§1](#1-phase-e--g6-comparison--seaorm-mapping-hygiene) |
| 2 | Phase C — examples (`find_related` + `.scope` / `.filter`) | [§2](#2-phase-c--find_related--related-side-scope-example-surface) |
| 4 | Phase C — inherited parent scopes + loaders (future) | [§3](#3-phase-c--inherited-parent-selectquery--loader-merge-future) |

*(Row 3 in §0.3 — `#[scope_bundle]` — is **shipped**; not duplicated here.)*

---

## 1. Phase E / G6 — COMPARISON + SEAORM mapping hygiene

### What the PRD asks

[§0.3](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md) row **1** and **goal G6** ([§3](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md)): keep **[COMPARISON.md](../../COMPARISON.md)** and **[SEAORM_LIFEGUARD_MAPPING.md](./lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md)** **aligned** when public APIs change, and do **small rustdoc fixes in the same PRs** as features.

### What “aligned” means in practice

- **COMPARISON.md:** Repository truth (what ships), competitive matrix rows, and “Partial / Implemented” labels match **current** crates and CLI. A feature merge that changes user-visible behavior should **either** update COMPARISON in that PR **or** file a follow-up with a tracked issue — default is **same PR** for small deltas.
- **SEAORM_LIFEGUARD_MAPPING.md:** The **PRD parity snapshot** table and per-topic rows (scopes, `find_related`, session, validators, schema tools, …) reflect the **same** semantics as rustdoc and integration tests. Cross-links (e.g. scopes → `DESIGN_FIND_RELATED_SCOPES.md`) stay valid after moves/renames.

### How this work is *used*

- **Maintainers:** Before merging a PR that touches `lifeguard`, `lifeguard-derive`, or `lifeguard-migrate` public surfaces, run through a short **G6 checklist** (below).
- **Readers:** COMPARISON + mapping doc stay the **single narrative** for “what Lifeguard does vs SeaORM” without reading the whole PRD.

### G6 checklist (repeatable)

1. Does **COMPARISON.md** need a row or footnote for this change (new API, limitation removed, new CLI flag)?
2. Does **SEAORM_LIFEGUARD_MAPPING.md** parity snapshot or feature bullets need a one-line update?
3. Are **rustdoc** module/README pointers updated if behavior changed (`query::scope`, `FindRelated`, migrate README, etc.)?
4. If the change touches **scopes + relations**, does [DESIGN_FIND_RELATED_SCOPES.md](./DESIGN_FIND_RELATED_SCOPES.md) still describe the default accurately?

### What this is *not*

- Not a **greenfield feature**: it is **process + documentation discipline** batched with shipping work.
- Not a substitute for **versioned CHANGELOG** entries if the repo uses them for releases — G6 is **mapping accuracy**, not release notes (though they often overlap).

### Before we “build” anything here

There is nothing to code for G6 itself. The checklist lives in **[DEVELOPMENT.md](../../DEVELOPMENT.md)** (**G6 — COMPARISON + SeaORM mapping**) and points here for full context.

---

## 2. Phase C — `find_related` + related-side scope (example surface)

### What the PRD asks

[§0.3](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md) row **2**: an **integration test** or **`examples/`** entry showing **`find_related`** then **`.scope` / `.filter`** on the returned **`SelectQuery`**, with the explicit story that **parent scopes are not merged** into `find_related` SQL.

**SC-1** ([§7.5](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md)) acceptance: *Example in `examples/` or integration test compiles and runs.*

### What already exists

- Integration coverage: **`tests/db_integration/related_trait.rs`** includes patterns such as **`test_find_related_chains_scope_on_related_query`** and **`test_find_related_scoped_matches_chained_scope`** — related-side scope on the **child** query only.

### Gap vs PRD wording

- The PRD still suggests a **discoverable `examples/`** path for developers who do not read `tests/`. A **minimal `examples/` crate** (or a single binary under an existing examples tree) that mirrors the integration test would close the **documentation / onboarding** gap, not the **correctness** gap.

### How this would be *used* (once built)

- New users copy the example: “start from a loaded **parent model**, call **`find_related::<Child>()`**, then **`.scope(ChildEntity::scope_…())`** (or **`.filter`**).”
- Comments in the example state the invariant: **`Post::find().scope(published)`** does **not** flow into **`post_model.find_related::<Comment>()`** — only **Comment**-table scopes on the returned query apply unless you use **`find_related_parent_scoped`** for **parent-table** predicates.

### Shipped example (root crate)

- **`examples/find_related_scope_example.rs`** — compiles without a database (`cargo check --example find_related_scope_example`); demonstrates **`find_related` → `.scope`** and **`find_related_scoped`** with the same related-side predicate. Table names are **`example_scope_demo_*`** (documentation-only; no DDL in this binary).

### Risks / constraints

- Examples must stay **in sync** with derive and relation APIs — treat them as **extra coverage** that CI compiles (`cargo check -p …` / workspace `examples` pattern the repo already uses).
- Do not imply **parent** scope inheritance; pair with a one-line pointer to [DESIGN_FIND_RELATED_SCOPES.md](./DESIGN_FIND_RELATED_SCOPES.md).

### Before further work

Integration tests in **`tests/db_integration/related_trait.rs`** remain the **behavioral** source of truth; the **`examples/`** binary is for **discovery** and **compile-time** documentation.

---

## 3. Phase C — inherited parent `SelectQuery` + loader merge (spike completed)

### What the PRD asked

[§0.3](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md) row **4** (tail): **Inherited parent scopes** — merging a parent **`SelectQuery`** into **`find_related`** SQL — plus **loaders** (highest integration risk).

**Spike conclusion:** **[DESIGN_INHERITED_PARENT_SCOPES_SPIKE.md](./DESIGN_INHERITED_PARENT_SCOPES_SPIKE.md)** — recommend **A** (no new implicit-merge API) + document **D** (two-step / explicit PKs); **B/C** deferred. **Implicit** merge remains **not** implemented; see [DESIGN_FIND_RELATED_SCOPES.md](./DESIGN_FIND_RELATED_SCOPES.md) appendix §C.

### Problem statement

Today, **`User::find().scope(active_users)`** and **`user.find_related::<Post>()`** are **independent**. That is **safe** but repetitive if the product intent is: “posts for this user **and** only if the user satisfies the same predicates as `active_users`.” Today you must **`find_related_parent_scoped`** (explicit parent predicate) or **duplicate** the condition.

**Loaders** batch-load related rows for many parents. There is **no** API that says “apply this **parent** `SelectQuery` filter when resolving FK batches,” because that would require either:

- Carrying **parent query state** into the loader, or
- Re-deriving **SQL** that may not be expressible as a simple **`AND`** on the child query.

### How it *might* be used (hypothetical — not shipped)

1. **Batch:** “For parents returned by **`User::find().scope(tenant).all()`**, load **`Post`** rows per user” — inheriting **`tenant`** on **`users`** into the loader’s SQL or prefetch keys.
2. **Single model:** “**`find_related`** should **see** the same **`WHERE`** I would have had on **`User::find()`**” without calling **`find_related_parent_scoped`** with a hand-duplicated condition.

### Why “highest risk”

- **Semantic ambiguity:** Parent scope might mean **restrict parents** (JOIN), **restrict children** (correlated subquery), or **filter the batch** — three different SQL shapes.
- **Loaders** already optimize **IN (parent_ids)** style batches; adding arbitrary parent **`WHERE`** clauses can force **nested loops** or **temp tables** if not carefully constrained.
- **Duplicate predicates:** If both parent and child paths apply similar filters, it is easy to **double-apply** or **contradict** unless the API is explicit.

### What exists today (bridge APIs)

| Intent | API / pattern |
|--------|----------------|
| Related-table predicate only | **`find_related_scoped`** or **`find_related()?.scope(...)`** |
| Parent-table predicate in same SQL as related load (direct edge) | **`find_related_parent_scoped`** |
| Full parent query not expressible as one `IntoCondition` | Manual **`SelectQuery`** / joins |

See also appendix **§C** in [DESIGN_FIND_RELATED_SCOPES.md](./DESIGN_FIND_RELATED_SCOPES.md).

### Spike directions (if we ever schedule engineering)

1. **Constrained inheritance only:** e.g. only **`AND`** of **`IntoCondition`**s known to reference **parent** table — no full `SelectQuery` capture.
2. **Loader-specific:** a **`Loader::with_parent_filter(…)`** that takes a **closed-world** predicate type, not arbitrary `SelectQuery` state.
3. **Documentation-first:** keep **no implicit merge** until a design doc signs off on SQL shapes + one integration test matrix per relation kind (`belongs_to`, `has_many`, `has_many_through`).

### Index comparison (PRD §5.7a) — separate track

Deferred items (opclass, full `indexdef`, derive-time checks) are **not** part of the inherited-scope decision. See **[DESIGN_INDEX_COMPARE_ROADMAP.md](./DESIGN_INDEX_COMPARE_ROADMAP.md)**.

---

## Cross-links

- PRD: [§0.3 Follow-on priority](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md) (search **0.3**)
- Relations + scopes default: [DESIGN_FIND_RELATED_SCOPES.md](./DESIGN_FIND_RELATED_SCOPES.md)
- Deferred compare-schema index parity: [DESIGN_FIND_RELATED_SCOPES.md — Appendix](./DESIGN_FIND_RELATED_SCOPES.md#appendix-deferred-behavior-and-how-it-would-be-used) §A; phased backlog: [DESIGN_INDEX_COMPARE_ROADMAP.md](./DESIGN_INDEX_COMPARE_ROADMAP.md)
- Inherited parent scopes (spike completed): [DESIGN_INHERITED_PARENT_SCOPES_SPIKE.md](./DESIGN_INHERITED_PARENT_SCOPES_SPIKE.md)
