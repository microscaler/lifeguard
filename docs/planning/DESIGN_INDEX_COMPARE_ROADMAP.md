# Roadmap: `compare-schema` index parity (PRD §5.7a)

**Status:** Planning — **not** a commitment to ship order or dates. Complements [`lifeguard-migrate/README.md`](../lifeguard-migrate/README.md) **limits and roadmap** and [DESIGN_FIND_RELATED_SCOPES.md § Appendix A](./DESIGN_FIND_RELATED_SCOPES.md#appendix-deferred-behavior-and-how-it-would-be-used).

**PRD:** [§5.7a](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md#57a-deferred-phase-a-stretch--end-of-backlog).

---

## Shipped today (baseline)

- **Table / column** name reconciliation vs merged `*_generated_from_entities.sql`.
- **Index:** Parsed **btree key** + **`INCLUDE`** **column names** from `pg_indexes.indexdef` vs merged migration column map (expression indexes skipped when unparseable; PK indexes skipped per policy).

---

## Deferred tracks (independent increments)

| Track | Goal | Rough approach | Risk |
|-------|------|----------------|------|
| **T1 — Full `indexdef` text** | Fail CI when live `pg_indexes.indexdef` ≠ normalized migration text for the same index name | Normalize whitespace / identifier quoting; compare strings or hashes | False positives on PG version formatting |
| **T2 — Operator class / access method** | Detect `USING gist` vs `btree`, `jsonb_path_ops`, etc. | Extend parser or compare extracted `USING` + opclass tokens vs `IndexDefinition` / emitted SQL | Parser maintenance |
| **T3 — Expression / functional keys** | Include expression indexes in drift when both sides represent them | IR in `IndexDefinition` or structured parse of `indexdef` | High complexity |
| **T4 — Derive-time field ↔ index** | Warn when a `#[column]` is not covered by any declared `#[index]` | Derive pass over entity attrs + `sql_generator` index list | Ergonomics vs noise |

---

## Suggested priority (product-neutral)

1. **T2** if **semantic** index drift (wrong index type) matters more than literal text (**T1**).
2. **T1** if teams want a **single-string** gate without investing in structured opclass first.
3. **T4** as **developer feedback** on models, not a substitute for **T1/T2** against live DB.
4. **T3** last unless expression indexes are common in target deployments.

---

## References

- `lifeguard_migrate::schema_migration_compare`
- `lifeguard_migrate::sql_generator` / `IndexDefinition`
