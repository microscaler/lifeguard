# PostgreSQL scalars: UUID and Chrono alignment

- **Status**: `verified`
- **Source docs**: [`docs/UUID_AND_POSTGRES_TYPES.md`](../../UUID_AND_POSTGRES_TYPES.md), [`docs/CHRONO_AND_POSTGRES_TYPES.md`](../../CHRONO_AND_POSTGRES_TYPES.md)
- **Code anchors**: `lifeguard/src/query/from_row` (and derive-generated `from_row`), [`src/lib.rs`](../../../src/lib.rs) crate docs on UUID, [`src/value/text_param.rs`](../../../src/value/text_param.rs)
- **Last updated**: 2026-06-12

## What it is

**UUID** columns must map to **`uuid::Uuid`**. **`timestamptz`** ↔ `chrono::DateTime<Utc>`; **`timestamp`** (without TZ) ↔ `NaiveDateTime` — mismatches cause **runtime** decode issues, not always compile errors.

## JSON/JSONB: strings bind with `text::jsonb` cast semantics (2026-06-12)

`postgres-types` only lets `String` bind to TEXT-family columns, so a
stringified JSON document passed as `Value::String` against a `jsonb`
parameter used to fail at bind time with *"cannot convert between the Rust
type `Option<String>` and the Postgres type `jsonb`"*. Downstream callers
worked around it with `($n::text)::jsonb` casts (seen in Tiffany's WAL sink).

Fixed by [`TextParam`](../../../src/value/text_param.rs): both dispatch paths
(`query/converted_params.rs` for direct executors, `pool/owned_param.rs` for
`LifeguardPool`) now carry `Value::String` as `TextParam`, which

- binds TEXT/VARCHAR exactly as `String` did (identical wire bytes), and
- for JSON/JSONB columns applies PostgreSQL's own `text::jsonb` cast
  semantics: parse the string as a JSON document and bind the parsed value;
  **invalid JSON is a bind-time error**, not a silently-wrapped string scalar.
- `Value::String(None)` is now a typed NULL accepted by TEXT *and* JSON/JSONB.

**`Value::Json` / `serde_json::Value` remains the idiomatic carrier for JSON
documents** (no parse round-trip); `TextParam` removes the footgun for raw
statements and `execute_values` callers at API boundaries. Live-verified
against Postgres 16 via Tiffany's `crates/executor/tests/wal_pg.rs`
(`stringified_json_binds_to_inferred_jsonb_param`).

## Cross-references

- [`entities/life-model-and-life-record.md`](../entities/life-model-and-life-record.md)
- [`brrtrouter-integration-pitfalls.md`](./brrtrouter-integration-pitfalls.md)
- Postmortem: [`docs/postmortem-lifeguard-derive-naivedate-chronodate-2026-04.md`](../../postmortem-lifeguard-derive-naivedate-chronodate-2026-04.md)
