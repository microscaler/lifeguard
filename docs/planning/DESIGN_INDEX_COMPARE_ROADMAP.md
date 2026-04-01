# Roadmap: `compare-schema` index parity (PRD §5.7a)

**Status:** Post-PRD backlog — **T2 (access method)** baseline is **shipped** in `schema_migration_compare`. Remaining tracks are **optional** hardening; see [ROADMAP.md](../../ROADMAP.md). Complements [`lifeguard-migrate/README.md`](../lifeguard-migrate/README.md).

**PRD:** [§5.7a](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md#57a-deferred-phase-a-stretch--end-of-backlog) — **PRD item closed** for access-method drift; **T1 / T3 / T4** remain here until picked up from the product roadmap.

---

## Shipped today (baseline)

- **Table / column** name reconciliation vs merged `*_generated_from_entities.sql`.
- **Index:** Parsed **btree key** + **`INCLUDE`** **column names** from `pg_indexes.indexdef` vs merged migration column map (expression indexes skipped when unparseable; PK indexes skipped per policy).
- **T2 (partial):** [`parse_pg_indexdef_access_method`](../lifeguard-migrate/src/schema_migration_compare.rs) + [`IndexAccessMethodDrift`](../lifeguard-migrate/src/schema_migration_compare.rs): live indexes whose access method is not **`btree`** are reported (entity/sql_generator assumes btree). Does **not** compare btree **opclass** variants (`jsonb_path_ops`, …).

---

## Deferred tracks (independent increments)

| Track | Goal | Rough approach | Risk |
|-------|------|----------------|------|
| **T1 — Full `indexdef` text** | Fail CI when live `pg_indexes.indexdef` ≠ normalized migration text for the same index name | Normalize whitespace / identifier quoting; compare strings or hashes | False positives on PG version formatting |
| **~~T2 — Access method (non-btree)~~** | **Shipped:** non-`btree` `USING` → drift | `parse_pg_indexdef_access_method` vs implicit btree | — |
| **T2b — Btree opclass tokens** | Detect `jsonb_path_ops` vs default opclass on same access method | Parse opclass after column in `indexdef` vs `IndexDefinition` | Parser maintenance |
| **T3 — Expression / functional keys** | Include expression indexes in drift when both sides represent them | IR in `IndexDefinition` or structured parse of `indexdef` | High complexity |
| **T4 — Derive-time field ↔ index** | Warn when a `#[column]` is not covered by any declared `#[index]` | Derive pass over entity attrs + `sql_generator` index list | Ergonomics vs noise |

---

## Suggested priority (product-neutral)

1. **T2b** if **btree opclass** drift (same access method, different operator class) matters for your deployments.
2. **T1** if teams want a **single-string** gate for whole `indexdef`.
3. **T4** as **developer feedback** on models, not a substitute for **T1/T2b** against live DB.
4. **T3** last unless expression indexes are common in target deployments.

---

## References

- `lifeguard_migrate::schema_migration_compare`
- `lifeguard_migrate::sql_generator` / `IndexDefinition`
