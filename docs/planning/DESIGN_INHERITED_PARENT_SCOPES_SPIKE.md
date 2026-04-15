# Design spike: inherited parent `SelectQuery` + loader merge

**Status:** **Spike completed (2026-03-28)** — recommendation below; **no new API shipped** by this document. Complements [PRD_FOLLOWON_NEXT_THREE.md §3](./PRD_FOLLOWON_NEXT_THREE.md) and [DESIGN_FIND_RELATED_SCOPES.md § Appendix C](./DESIGN_FIND_RELATED_SCOPES.md).

**PRD:** [§0.3](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md) follow-on row **4** (implicit parent merge + loaders — future / highest risk).

---

## 1. Objective of the spike

Decide whether Lifeguard should offer **any** first-class API that carries **parent** `SelectQuery` / scope state into **`find_related`**-style SQL or **relation loaders**, and if yes, **which SQL shapes** are in scope for v1.

**Outcome:** Adopt **Direction A** (no new API for implicit merge) as the default product position; treat **Direction D** as the **documented application pattern**; defer **B** and **C** until a future PRD revision with explicit SQL semantics per relation kind.

---

## 2. Problem restatement

Today, **`User::find().scope(active)`** and **`user.find_related::<Post>()`** are independent. Callers duplicate intent or use **`find_related_parent_scoped`** with an explicit **`IntoCondition`** on the parent table.

**Loaders** optimize **`IN (parent_ids)`**-style reads. An arbitrary parent **`WHERE`** clause is not equivalent to “filter by these ids” unless the application first runs the parent query and passes IDs — which may be large or expensive.

---

## 3. Candidate directions (not mutually exclusive)

| Direction | Idea | Main risk |
|-----------|------|-----------|
| **A. No new API** | Document + examples only; use **`find_related_parent_scoped`**, manual joins, or two-step “parent ids then loader” | Boilerplate remains |
| **B. Constrained parent filter** | New API accepts only **`IntoCondition`** known to reference **`from_tbl`** (parent), merged via **`JOIN`** / **`EXISTS`** — not a captured full `SelectQuery` | Still must define **`has_many_through`** |
| **C. Loader + parent predicate** | **`Loader::…`** variant that takes a **closed-world** parent predicate type (not arbitrary query state) | Batch size, plan regressions |
| **D. Two-step helper** | Helper returns parent PKs from a **`SelectQuery`**, then runs existing loader with those ids | Extra round-trip; explicit |

---

## 4. Open questions — **resolved for this spike**

| # | Question | **Resolution** |
|---|----------|----------------|
| 1 | Semantics (i restrict parents / (ii) `EXISTS` / (iii) both)? | **Ambiguous without an explicit API.** Each option implies different SQL. **No implicit merge** until a design picks one shape per relation type; use **`find_related_parent_scoped`** for (i)-style **JOIN** on parent, or manual SQL for (ii). |
| 2 | Soft delete inheritance? | **Not automatic.** Parent soft-delete behavior follows whatever **`IntoCondition`** / query the caller builds. Document only; no magic coupling to `LifeModelTrait::soft_delete_column` in loaders. |
| 3 | `has_many_through` parity vs direct edges first? | **`find_related_parent_scoped`** already **direct edges only**; **through** paths remain **explicit joins** or future targeted API — **not** blocked on a generic “inherit parent query” feature. |
| 4 | When is two-step (ids → loader) OK? | **Default acceptable** for bounded parent sets; document **pagination / streaming** for huge ID lists. Single-query optimization is **application-specific** — out of scope for a generic implicit merge. |

---

## 5. Recommendation (directions A–D)

| Direction | **Verdict** |
|-----------|-------------|
| **A. No new API** | **Adopt.** Existing surface: **`find_related`**, **`find_related_scoped`**, **`find_related_parent_scoped`**, manual **`SelectQuery`**, loaders with explicit parent keys. |
| **B. Constrained parent filter** | **Defer.** Revisit only with a per-relation-kind spec + tests; **`has_many_through`** must be specified first. |
| **C. Loader + parent predicate** | **Defer.** High risk of plan regressions; needs closed-world predicate type and benchmarks. |
| **D. Two-step helper** | **Document as pattern**, not necessarily a new crate API: run parent **`SelectQuery`** → collect PKs → pass to loader / child query. Optional **small helper** (e.g. `select_primary_keys_only`) could be a later **ergonomics** PR without “implicit merge” semantics. |

**SQL sketch (has_many, pattern D):**

1. `let parent_ids: Vec<_> = User::find().scope(tenant).select_pk_only().all(…)?` (conceptual — actual API is app-level column selection).
2. `Post::find().filter(PostColumn::UserId.is_in(parent_ids)).scope(…).all(…)` or existing **RelationLoader** with **`parent_ids`**.

---

## 6. Exit criteria — **met**

- **Written recommendation:** **A** + document **D**; **B/C** deferred.
- **Tests that would prove B if ever built:** integration cases for **`has_many`** + **`has_many_through`** with identical parent predicate on JOIN vs EXISTS; loader batch with filtered parent set vs full table scan (performance regression guard).
- **Docs:** This file + PRD §7.7 / §0.4 pointers; [DESIGN_FIND_RELATED_SCOPES.md](./DESIGN_FIND_RELATED_SCOPES.md) appendix §C unchanged in behavior (still “not implemented”).

---

## 7. References and related roadmaps

- [`FindRelated::find_related_parent_scoped`](../../src/relation/traits.rs)
- [`query::loader`](../../src/query/loader.rs) (batch loading)
- Integration tests: `tests/db_integration/related_trait.rs`

**PRD §5.7a (index comparison — separate workstream):** [DESIGN_INDEX_COMPARE_ROADMAP.md](./DESIGN_INDEX_COMPARE_ROADMAP.md) (opclass / full `indexdef` / derive checks — orthogonal to inherited scopes).
