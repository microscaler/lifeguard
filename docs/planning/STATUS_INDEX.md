# Planning doc status index

Status labels for 13 planning and design documents from the early ORM build-out (December 2024 to January 2026). Each document starts with a status block: status, the commit it was verified against, and file:line evidence. The document bodies are unchanged and are historical, so read the status block first.

**Verified against:** main @ bf68b3a, 2026-11-11. Items 1, 3 and 7 below were fixed afterwards; see the commit next to each.

Status values: **Implemented**, **Partially implemented**, **Proposed (not implemented)**, **Superseded**, **Discussion**.

| Document | Status | Summary | Still to implement |
|----------|--------|---------|--------------------|
| [root/MODULARIZATION_PROPOSAL.md](./root/MODULARIZATION_PROPOSAL.md) | Partially implemented | Split the large `query`, `active_model`, `relation` and `partial_model` files into module trees (phases 1-5). | `query/{insert,update,delete,join}.rs`, `active_model/{insert,update,delete}.rs`, `relation/def/builder.rs`; merge `src/model.rs` into `src/model/`; the 800-line file limit is exceeded by seven files again. |
| [root/SEAQUERY_IMPROVEMENTS_AUDIT.md](./root/SEAQUERY_IMPROVEMENTS_AUDIT.md) | Implemented | Asked sea-query for a way to replace SELECT columns while keeping WHERE / ORDER BY / LIMIT, which `select_partial()` needs. | None; `select_partial()` uses sea-query 1.0.2 `clear_selects()` (11c7882). |
| [lifeguard-derive/CODEGEN_ANALYSIS.md](./lifeguard-derive/CODEGEN_ANALYSIS.md) | Implemented | Why generating partial-model source files avoids E0284 errors from macro expansion. | Optional: codegen for other macros (only partial models are generated). |
| [lifeguard-derive/CRUD_IMPLEMENTATION_PLAN.md](./lifeguard-derive/CRUD_IMPLEMENTATION_PLAN.md) | Implemented | `insert` / `update` / `save` / `delete` on `ActiveModelTrait`, with PK tracking and RETURNING. | None. |
| [lifeguard-derive/DERIVE_RELATION_USAGE.md](./lifeguard-derive/DERIVE_RELATION_USAGE.md) | Implemented | Usage guide for `DeriveRelation`, including composite keys. | None; `has_many_through` has shipped, and `find_related` / `find_related_parent_scoped` join the through table (6f60608). |
| [lifeguard-derive/DESIGN_RELATION_METADATA_AND_COMPOSITE_KEYS.md](./lifeguard-derive/DESIGN_RELATION_METADATA_AND_COMPOSITE_KEYS.md) | Implemented | Replace the `RelationMetadata` trait with SeaORM-style `RelationDef` + `Identity`; composite keys in `find_related`. | Migration guide for the `Related::to()` return-type change; optional `find_related` benchmark; consider retiring the unused `RelationMetadata` trait. |
| [lifeguard-derive/DESIGN_TRY_INTO_MODEL.md](./lifeguard-derive/DESIGN_TRY_INTO_MODEL.md) | Partially implemented | `TryIntoModel` trait and `DeriveTryIntoModel` for DTO to Model conversion. | Validation hooks, field-renaming strategies, nested-struct conversion, the unused `optional` attribute, `TryFromModel`, collection conversions. |
| [lifeguard-derive/EDGE_CASES_ANALYSIS.md](./lifeguard-derive/EDGE_CASES_ANALYSIS.md) | Partially implemented | `ModelTrait` get/set edge cases: Option, JSON, unsigned ints, missing PK. | A better result than `String(None)` for entities without a PK; reject models with no fields. |
| [lifeguard-derive/EDGE_CASES_COVERAGE.md](./lifeguard-derive/EDGE_CASES_COVERAGE.md) | Partially implemented | Edge-case test inventory across partial models, relations, joins, hooks, JSON and the query builder. | The 37-test `lifeguard-derive/tests/test_edge_cases.rs` it describes was never committed; table aliasing for repeated joins; subqueries in JOIN conditions. |
| [lifeguard-derive/REGISTRY_PATTERN_AUDIT.md](./lifeguard-derive/REGISTRY_PATTERN_AUDIT.md) | Implemented | Audit and rule: no `TypeId`-keyed static registries; prefer struct metadata such as `RelationDef`. | Re-audit to classify `MIGRATION_REGISTRY` and `EXPR_CACHE`. |
| [lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md](./lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md) | Partially implemented | SeaORM/SeaQuery to Lifeguard parity map. | Missing derives: `DeriveModelEx`, `DeriveActiveModel(Ex)`, `DeriveIntoActiveModel`, `DeriveActiveEnum`, `FromJsonQueryResult`, `DeriveValueType`, `DeriveDisplay`, `DeriveIden`. Rows for FK / CHECK / composite unique / table comment / index are stale (now implemented). |
| [lifeguard-derive/TEST_ERRORS_AUDIT.md](./lifeguard-derive/TEST_ERRORS_AUDIT.md) | Implemented | Record of the derive test fixes, ending in the codegen approach. | None. |
| [lifeguard-derive/TEST_FAILURE_AUDIT.md](./lifeguard-derive/TEST_FAILURE_AUDIT.md) | Superseded | Debugging log for the 40-test derive failure (E0119 / E0223 / E0284 ...). | None; replaced by `TEST_ERRORS_AUDIT.md` and `CODEGEN_ANALYSIS.md`. |

## Not implemented yet (candidate tasks)

1. **Done (11c7882).** **Partial-model clause preservation:** `select_partial()` drops WHERE / ORDER BY / LIMIT / OFFSET (`src/partial_model/query.rs:147-187`). Fix it with sea-query 1.0.2's `SelectStatement::clear_selects()` on the existing statement; no upstream change is needed. Source: SEAQUERY_IMPROVEMENTS_AUDIT.
2. **TryIntoModel phase 3:** validation hooks, field-renaming strategies and nested-struct conversion; implement or remove the `optional` attribute (`lifeguard-derive/src/macros/try_into_model.rs:165`). Later: `TryFromModel`, `Vec<DTO>` conversion. Source: DESIGN_TRY_INTO_MODEL.
3. **Done (88c7d78).** **Checked `u64` in `ModelTrait::set`:** replace `v as u64` (`lifeguard-derive/src/macros/life_model.rs:1117, 1395`) with `try_from`, as `FromRow` already does. Source: EDGE_CASES_ANALYSIS. The fix also covers the `u8` / `u16` / `u32` arms, which wrapped the same way.
4. **Entities without a primary key:** `get_primary_key_value()` returns `Value::String(None)` (`life_model.rs:1782-1784`); consider a compile error or `Option`. Also reject `LifeModel` structs with no fields. Source: EDGE_CASES_ANALYSIS.
5. **Edge-case test suite:** write the `lifeguard-derive/tests/test_edge_cases.rs` coverage the doc describes, which was never committed. Source: EDGE_CASES_COVERAGE.
6. **Join aliasing and JOIN subqueries:** several joins to the same table, and subqueries in join conditions. Source: EDGE_CASES_COVERAGE.
7. **Done (6f60608).** **`has_many_through` in `find_related`:** `find_related` filters with `build_where_condition` only and ignores the through table (`src/relation/traits.rs:556-572`), so through relations can return wrong rows; `find_related_parent_scoped` rejects them (`traits.rs:531-533`). Join the through table as the eager loader does (`src/relation/eager.rs:309`), or reject through relations. Source: DERIVE_RELATION_USAGE.
8. **`Related::to()` migration note:** document the `SelectQuery` to `RelationDef` return-type change; decide whether to retire `RelationMetadata` (`src/relation/traits.rs:340`). Source: DESIGN_RELATION_METADATA_AND_COMPOSITE_KEYS.
9. **Remaining SeaORM derives:** `DeriveActiveEnum`, `DeriveValueType`, `FromJsonQueryResult`, `DeriveIntoActiveModel`, `DeriveIden`, `DeriveDisplay`, `DeriveModelEx` / `DeriveActiveModel(Ex)`. Several are marked "not needed" in the mapping, so prioritise first. Source: SEAORM_LIFEGUARD_MAPPING.
10. **Refresh the mapping doc:** update the stale FK / CHECK / composite unique / table comment / index rows and the §10 percentages. Source: SEAORM_LIFEGUARD_MAPPING.
11. **Modularization follow-up:** split the seven files over 800 lines and finish the proposed `insert` / `update` / `delete` / `join` modules, or drop that goal. Source: MODULARIZATION_PROPOSAL.
12. **Registry re-audit:** classify `MIGRATION_REGISTRY` (`src/migration/registry.rs:14`) and `EXPR_CACHE` (`src/query/column/definition.rs:19`). Source: REGISTRY_PATTERN_AUDIT.

## Duplicates elsewhere

- `audits/REGISTRY_PATTERN_AUDIT.md` is byte-identical to `lifeguard-derive/REGISTRY_PATTERN_AUDIT.md` (as of bf68b3a).
- `audits/TEST_FAILURE_AUDIT.md` is an earlier, shorter revision of `lifeguard-derive/TEST_FAILURE_AUDIT.md`.
