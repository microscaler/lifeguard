# Design spike: inherited parent `SelectQuery` + loader merge

**Status:** Spike / pre-implementation — **no API is committed**. Complements [PRD_FOLLOWON_NEXT_THREE.md §3](./PRD_FOLLOWON_NEXT_THREE.md) and [DESIGN_FIND_RELATED_SCOPES.md § Appendix C](./DESIGN_FIND_RELATED_SCOPES.md).

**PRD:** [§0.3](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md) follow-on row **4** (implicit parent merge + loaders — future / highest risk).

---

## 1. Objective of a future spike

Decide whether Lifeguard should offer **any** first-class API that carries **parent** `SelectQuery` / scope state into:

- **`find_related`**-style SQL, or
- **Relation loaders** (batch prefetch by parent keys),

and if yes, **which SQL shapes** are in scope for v1.

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

## 4. Open questions (must answer before coding)

1. **Semantics:** Does “inherit parent scope” mean (i) restrict which parents participate, (ii) add **`EXISTS`** from child to parent with parent predicates, or (iii) both?
2. **Soft delete:** If parent has **`deleted_at`**, is inheritance automatic when the parent query would have applied the soft-delete filter?
3. **`has_many_through`:** Required for parity, or ship **direct edges only** first?
4. **Performance:** Under what cardinality is **two-step** (ids then loader) acceptable vs mandatory single-query?

---

## 5. Exit criteria for “spike done”

- One **written** recommendation: **ship nothing** | **ship B** | **ship D** | … with **SQL examples** for **`has_many`** and at least one loader path.
- List of **integration tests** that would prove the chosen semantics (no implementation required in the spike doc itself).
- Update [DESIGN_FIND_RELATED_SCOPES.md](./DESIGN_FIND_RELATED_SCOPES.md) and rustdocs if a decision is made.

---

## 6. References

- [`FindRelated::find_related_parent_scoped`](../../src/relation/traits.rs)
- [`query::loader`](../../src/query/loader.rs) (batch loading)
- Integration tests: `tests/db_integration/related_trait.rs`
