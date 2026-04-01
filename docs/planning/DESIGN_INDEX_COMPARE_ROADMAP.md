# Roadmap: `compare-schema` index parity (PRD §5.7a)

**Status:** **T1**, **T2**, **T4** shipped; **T2b** **partial** (**live** opclass vs **merged explicit or type default**; [`fetch_live_btree_index_key_opclasses`](../lifeguard-migrate/src/schema_migration_compare.rs)). **T3** **partial:** v1 [`fetch_live_btree_expression_index_key_slots`](../lifeguard-migrate/src/schema_migration_compare.rs); v2 normalized slot compare + **T1** opclass-only dedupe + explicit **ordering/collation** vs `pg_index` ([`fetch_live_btree_index_key_catalog_slots`](../lifeguard-migrate/src/schema_migration_compare.rs)). **T3** derive / expression IR still optional; see [ROADMAP.md](../../ROADMAP.md). Complements [`lifeguard-migrate/README.md`](../lifeguard-migrate/README.md).

**PRD:** [§5.7a](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md#57a-deferred-phase-a-stretch--end-of-backlog) — access-method drift, **T1**-style text compare, and **T4** derive coverage are implemented; **T2b** / **T3** extensions (expected opclass from migration, normalized expression-on-both-sides) remain backlog if product needs them.

---

## Shipped today (baseline)

- **Table / column** name reconciliation vs merged `*_generated_from_entities.sql`.
- **Index (name-level):** Parsed **btree key** + **`INCLUDE`** **column names** from `pg_indexes.indexdef` vs merged migration column map **when there is no `CREATE INDEX` line for that index name** in the merged baseline (expression indexes skipped when unparseable; PK indexes skipped per policy).
- **T1:** [`normalize_index_statement_for_compare`](../lifeguard-migrate/src/schema_migration_compare.rs) + [`index_statements_for_table_from_merged_baseline`](../lifeguard-migrate/src/generated_migration_diff.rs): for shared tables, when the same **index name** appears in merged migration SQL and in `pg_indexes`, normalized statements are compared; mismatch → [`IndexDefinitionTextDrift`](../lifeguard-migrate/src/schema_migration_compare.rs). Index names only in live DB or only in merged SQL → `IndexOnlyInDatabaseDrift` / `IndexOnlyInMigrationDrift` (with dedupe vs access-method / unknown-column drifts as implemented in `compare_generated_dir_to_live_db`).
- **T2 (partial):** [`parse_pg_indexdef_access_method`](../lifeguard-migrate/src/schema_migration_compare.rs) + [`IndexAccessMethodDrift`](../lifeguard-migrate/src/schema_migration_compare.rs): live indexes whose access method is not **`btree`** are reported.
- **T2b (partial):** [`fetch_live_btree_index_key_opclasses`](../lifeguard-migrate/src/schema_migration_compare.rs) + [`IndexBtreeNonDefaultOpclassDrift`](../lifeguard-migrate/src/schema_migration_compare.rs): btree key slots whose opclass is not the column type’s default (`pg_opclass.opcdefault`). Skips expression keys; does not yet assert **expected** opclass from merged migration text or entity types.
- **T3 (partial):** [`fetch_live_btree_expression_index_key_slots`](../lifeguard-migrate/src/schema_migration_compare.rs) + [`IndexExpressionKeyVsSimpleMigrationDrift`](../lifeguard-migrate/src/schema_migration_compare.rs): when merged migration `CREATE INDEX` parses as **simple** key columns only but live **`pg_index.indkey`** has an **expression** slot, structured drift; **T1** omitted for that index.
- **T4:** `#[require_index_coverage]` on the struct (see `lifeguard-derive` / `attributes.rs`): compile-time check that every DB column is covered by **`#[primary_key]`**, **`#[indexed]`**, a table **`#[index = "..."]`** key or INCLUDE list, or **`#[composite_unique = "..."]`**.

---

## Deferred tracks (independent increments)

| Track | Goal | Rough approach | Risk |
|-------|------|----------------|------|
| **~~T1 — Full `indexdef` text~~** | **Shipped (partial):** normalized string compare when index names align; optional `IF NOT EXISTS` / `CONCURRENTLY` / explicit **`USING btree`** stripped | [`normalize_index_statement_for_compare`](../lifeguard-migrate/src/schema_migration_compare.rs) | False positives on PG version formatting, quoting, or `WHERE` / opclass details still possible |
| **~~T2 — Access method (non-btree)~~** | **Shipped:** non-`btree` `USING` → drift | `parse_pg_indexdef_access_method` vs implicit btree | — |
| **T2b — Btree opclass tokens** | **Partial:** expected = merged explicit opclass token or type default; live mismatch → drift; **T1** suppressed for opclass-only key differences | [`fetch_live_btree_index_key_opclasses`](../lifeguard-migrate/src/schema_migration_compare.rs) + merged key parse | **Follow-on:** entity-type opclass without hand-written SQL |
| **T3 — Expression / functional keys** | **Partial:** v1 simple-vs-expression; v2 normalized slot compare when either side has expressions; explicit **COLLATE** / **ASC/DESC** / **NULLS** vs catalog | [`fetch_live_btree_index_key_catalog_slots`](../lifeguard-migrate/src/schema_migration_compare.rs), [`normalize_index_key_slot_for_compare`](../lifeguard-migrate/src/schema_migration_compare.rs) | **Follow-on:** `IndexDefinition` / derive; cast noise in expressions |
| **~~T4 — Derive-time field ↔ index~~** | **Shipped (opt-in):** `#[require_index_coverage]` | `validate_require_index_coverage` in `lifeguard-derive` | Ergonomics vs noise — attribute is optional |

---

## Suggested priority (product-neutral)

1. **T2b** if **btree opclass** drift (same access method, different operator class) matters for your deployments.
2. **T3** when expression indexes are common in target deployments (**v1** shipped for live-expression vs migration-simple; **v2**+ still optional).

---

## References

- **`T2b` / `T3` (detailed design):** [DESIGN_INDEX_COMPARE_T2B_T3.md](./DESIGN_INDEX_COMPARE_T2B_T3.md) — btree opclass parity and expression / functional index keys (catalog vs string parsing, drift taxonomy, phases, open questions).
- `lifeguard_migrate::schema_migration_compare`
- `lifeguard_migrate::sql_generator` / `IndexDefinition`
