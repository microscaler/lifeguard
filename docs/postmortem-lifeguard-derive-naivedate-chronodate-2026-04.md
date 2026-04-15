# Postmortem: PostgreSQL `DATE` vs Rust bind mismatch on `LifeRecord::update` (`NaiveDate` / `ChronoDate`)

**Date:** 2026-04-13  
**Scope:** `lifeguard-derive` (`type_conversion.rs`), consumers with `Option<chrono::NaiveDate>` on `DATE` columns  
**Severity:** High — valid HTTP requests returned **500** after API errors were surfaced correctly (e.g. fleet vehicle edit via BRRTRouter `HttpJson`).

---

## Executive summary

`LifeRecord` → `sea_query::Value` conversion for `**Option<chrono::NaiveDate>`** was missing in `lifeguard-derive`. Types fell through to generic branches and produced **incorrect `Value` variants** for `UPDATE` statements. PostgreSQL then rejected binds with errors such as:

```text
error serializing parameter N: cannot convert between the Rust type `core::option::Option<i32>` and the Postgres type `date`
```

The **observed Rust type** (`Option<i32>`) is a symptom of **placeholder/value misalignment** across the statement, not proof that an integer was intentionally sent to a date column.

**Fix:** Explicit handling of `NaiveDate` → `sea_query::Value::ChronoDate` (and symmetric `Value` → field paths), aligned with existing `NaiveDateTime` → `ChronoDateTime` handling. See `lifeguard-derive/src/type_conversion.rs` (`is_naive_date_type`, `generate_option_field_to_value`, `generate_value_to_option_field`, etc.).

---

## Impact

- **User-visible:** Fleet (and any similar) **PUT** flows updating rows with `DATE` fields failed at persistence with **500** and a database serialization error in the JSON body.
- **Operational:** Errors became visible only after upstream HTTP semantics were fixed (e.g. no longer masking DB failures as **200**), so the ORM bug surfaced in production-like paths.

---

## Root cause

1. `**LifeModel` / schema:** Columns declared as `#[column_type = "DATE"]` with Rust `Option<chrono::NaiveDate>` are correct for PostgreSQL `date`.
2. **Derive gap:** `lifeguard-derive` `type_conversion` implemented `**NaiveDateTime`** → `ChronoDateTime` but **not** `**NaiveDate`** → `**ChronoDate**` (sea-query 1.x `Value::ChronoDate`).
3. `**LifeRecord::update`:** Builds `UPDATE ... SET` from `ActiveModelTrait::get()` per column. Wrong `Value` kinds for date columns led to **driver/type OID mismatch** at bind time.

---

## Why it was hard to see earlier

- End-to-end paths that **hid** DB errors (e.g. **200** with empty bodies or panics converted inconsistently) masked persistence failures.
- The error message mentions `**Option<i32>`**, which pointed readers toward `**fuel_litres_per_100km**`-style fields; the actual defect was **missing `NaiveDate` support** in codegen, not necessarily a single wrong field in application code.

---

## Verification

- `cargo test -p lifeguard-derive` (including `naive_date_type_detection_matches_bare_and_qualified_paths`).
- Rebuild affected services (e.g. `hauliage_fleet`) and re-run flows that `UPDATE` `DATE` columns.

---

## Prevention and follow-ups

1. **Types:** For PostgreSQL `DATE`, use `**chrono::NaiveDate`** (or the project’s agreed type) consistently on `LifeModel` fields; see [UUID_AND_POSTGRES_TYPES.md](./UUID_AND_POSTGRES_TYPES.md) for the same class of “Rust type must match driver” discipline.
2. **Derive:** When adding SQL scalar types, extend `**type_conversion.rs`** for **both** directions (`field` → `Value` and `Value` → `Option<T>`) and add a small **type-detection unit test** (pattern already used for `NaiveDateTime` / `Uuid`).
3. **Integration tests:** Where feasible, add a minimal **UPDATE** test against a real DB for entities with `DATE` columns (in addition to derive unit tests).

---

## Related documents

- [UUID_AND_POSTGRES_TYPES.md](./UUID_AND_POSTGRES_TYPES.md) — driver/type alignment for UUID; same principles apply to dates.
- [planning/lifeguard-derive/ISSUE_UUID_NAIVEDATETIME_TYPE_INFERENCE.md](./planning/lifeguard-derive/ISSUE_UUID_NAIVEDATETIME_TYPE_INFERENCE.md) — historical type-inference notes (UUID / `NaiveDateTime`).
- Hauliage context: fleet vehicle `**update_vehicle`** handler and `**Vehicle**` model (`mot_expiry_date`, `insurance_expiry_date`, `tax_expiry_date`).
- [AGENT.md](../AGENT.md) — linked from the **Historical Postmortems and ADRs** section for agent discoverability.

