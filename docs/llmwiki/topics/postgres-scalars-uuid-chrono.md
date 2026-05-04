# PostgreSQL scalars: UUID and Chrono alignment

- **Status**: `verified`
- **Source docs**: [`docs/UUID_AND_POSTGRES_TYPES.md`](../../UUID_AND_POSTGRES_TYPES.md), [`docs/CHRONO_AND_POSTGRES_TYPES.md`](../../CHRONO_AND_POSTGRES_TYPES.md)
- **Code anchors**: `lifeguard/src/query/from_row` (and derive-generated `from_row`), [`src/lib.rs`](../../../src/lib.rs) crate docs on UUID
- **Last updated**: 2026-04-17

## What it is

**UUID** columns must map to **`uuid::Uuid`**. **`timestamptz`** ↔ `chrono::DateTime<Utc>`; **`timestamp`** (without TZ) ↔ `NaiveDateTime` — mismatches cause **runtime** decode issues, not always compile errors.

## Cross-references

- [`entities/life-model-and-life-record.md`](../entities/life-model-and-life-record.md)
- [`brrtrouter-integration-pitfalls.md`](./brrtrouter-integration-pitfalls.md)
- Postmortem: [`docs/postmortem-lifeguard-derive-naivedate-chronodate-2026-04.md`](../../postmortem-lifeguard-derive-naivedate-chronodate-2026-04.md)
