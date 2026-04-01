# Roadmap: `compare-schema` index parity (PRD §5.7a)

**Status:** **T1**, **T2**, **T4** shipped; **T2b** **partially** shipped (**catalog** non-default btree opclass vs type default on shared tables — see [`fetch_live_btree_index_key_opclasses`](../lifeguard-migrate/src/schema_migration_compare.rs)). **T3** and **T2b** follow-ons (migration-expected opclass, expression keys) remain optional; see [ROADMAP.md](../../ROADMAP.md). Complements [`lifeguard-migrate/README.md`](../lifeguard-migrate/README.md).

**PRD:** [§5.7a](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md#57a-deferred-phase-a-stretch--end-of-backlog) — access-method drift, **T1**-style text compare, and **T4** derive coverage are implemented; **T2b** / **T3** remain backlog if product needs opclass / expression parity.

---

## Shipped today (baseline)

- **Table / column** name reconciliation vs merged `*_generated_from_entities.sql`.
- **Index (name-level):** Parsed **btree key** + **`INCLUDE`** **column names** from `pg_indexes.indexdef` vs merged migration column map **when there is no `CREATE INDEX` line for that index name** in the merged baseline (expression indexes skipped when unparseable; PK indexes skipped per policy).
- **T1:** [`normalize_index_statement_for_compare`](../lifeguard-migrate/src/schema_migration_compare.rs) + [`index_statements_for_table_from_merged_baseline`](../lifeguard-migrate/src/generated_migration_diff.rs): for shared tables, when the same **index name** appears in merged migration SQL and in `pg_indexes`, normalized statements are compared; mismatch → [`IndexDefinitionTextDrift`](../lifeguard-migrate/src/schema_migration_compare.rs). Index names only in live DB or only in merged SQL → `IndexOnlyInDatabaseDrift` / `IndexOnlyInMigrationDrift` (with dedupe vs access-method / unknown-column drifts as implemented in `compare_generated_dir_to_live_db`).
- **T2 (partial):** [`parse_pg_indexdef_access_method`](../lifeguard-migrate/src/schema_migration_compare.rs) + [`IndexAccessMethodDrift`](../lifeguard-migrate/src/schema_migration_compare.rs): live indexes whose access method is not **`btree`** are reported.
- **T2b (partial):** [`fetch_live_btree_index_key_opclasses`](../lifeguard-migrate/src/schema_migration_compare.rs) + [`IndexBtreeNonDefaultOpclassDrift`](../lifeguard-migrate/src/schema_migration_compare.rs): btree key slots whose opclass is not the column type’s default (`pg_opclass.opcdefault`). Skips expression keys; does not yet assert **expected** opclass from merged migration text or entity types.
- **T4:** `#[require_index_coverage]` on the struct (see `lifeguard-derive` / `attributes.rs`): compile-time check that every DB column is covered by **`#[primary_key]`**, **`#[indexed]`**, a table **`#[index = "..."]`** key or INCLUDE list, or **`#[composite_unique = "..."]`**.

---

## Deferred tracks (independent increments)

| Track | Goal | Rough approach | Risk |
|-------|------|----------------|------|
| **~~T1 — Full `indexdef` text~~** | **Shipped (partial):** normalized string compare when index names align; optional `IF NOT EXISTS` / `CONCURRENTLY` / explicit **`USING btree`** stripped | [`normalize_index_statement_for_compare`](../lifeguard-migrate/src/schema_migration_compare.rs) | False positives on PG version formatting, quoting, or `WHERE` / opclass details still possible |
| **~~T2 — Access method (non-btree)~~** | **Shipped:** non-`btree` `USING` → drift | `parse_pg_indexdef_access_method` vs implicit btree | — |
| **T2b — Btree opclass tokens** | **Partial:** catalog non-default vs type default on shared tables | [`fetch_live_btree_index_key_opclasses`](../lifeguard-migrate/src/schema_migration_compare.rs) | **Follow-on:** expected opclass from migration/entity; **T1** dedupe; collation |
| **T3 — Expression / functional keys** | Include expression indexes in drift when both sides represent them | IR in `IndexDefinition` or structured parse of `indexdef` | High complexity |
| **~~T4 — Derive-time field ↔ index~~** | **Shipped (opt-in):** `#[require_index_coverage]` | `validate_require_index_coverage` in `lifeguard-derive` | Ergonomics vs noise — attribute is optional |

---

## Suggested priority (product-neutral)

1. **T2b** if **btree opclass** drift (same access method, different operator class) matters for your deployments.
2. **T3** when expression indexes are common in target deployments.

---

## References

- **`T2b` / `T3` (detailed design):** [DESIGN_INDEX_COMPARE_T2B_T3.md](./DESIGN_INDEX_COMPARE_T2B_T3.md) — btree opclass parity and expression / functional index keys (catalog vs string parsing, drift taxonomy, phases, open questions).
- `lifeguard_migrate::schema_migration_compare`
- `lifeguard_migrate::sql_generator` / `IndexDefinition`
