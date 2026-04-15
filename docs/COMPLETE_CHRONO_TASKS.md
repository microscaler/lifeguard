# Implementation tasks: Complete Chrono (Lifeguard)

**PRD:** [`COMPLETE_CHRONO_IMPLEMENTATION.md`](./COMPLETE_CHRONO_IMPLEMENTATION.md)  
**Purpose:** Actionable checklist to deliver the PRD in iterations **A → E**; merge each iteration with **green `cargo test`** (workspace) before the next.

**Convention:** Check boxes as you complete. Link PRs or commits in the **Notes** column if helpful.

---

## How to use

1. Complete **Iteration A** fully (including tests) before starting B, unless you explicitly split work across people.
2. After each iteration: update **CHANGELOG** (Iteration E does the formal release note), run full test suite.
3. **Hauliage** work stays out of this repo — see PRD §8.

---

## Iteration A — Detection + `Value` mapping (derive)

**Goal:** `DateTime<Utc>` / `DateTime<Local>` map to `sea_query::Value::{ChronoDateTimeUtc, ChronoDateTimeLocal}` in all `generate_*_to_value` paths; Record/ActiveModel `get()` / setters agree.

| # | Task | Primary files | Done |
|---|------|---------------|------|
| A1 | Add `is_datetime_utc_type(ty: &Type) -> bool` — match `chrono::DateTime<Utc>`, imported `DateTime<Utc>`, and edge cases per existing `uuid` detection style | `lifeguard-derive/src/type_conversion.rs` | [x] |
| A2 | Add `is_datetime_local_type(ty: &Type) -> bool` for `DateTime<Local>` | `lifeguard-derive/src/type_conversion.rs` | [x] |
| A3 | Extend `generate_field_to_value`: UTC → `ChronoDateTimeUtc(Some(self.field))`, Local → `ChronoDateTimeLocal(Some(self.field))` | `lifeguard-derive/src/type_conversion.rs` | [x] |
| A4 | Extend `generate_option_field_to_value` + `generate_option_field_to_value_with_default` for `Option<DateTime<Utc>>` / `Local` (Some/None → typed `Value`) | `lifeguard-derive/src/type_conversion.rs` | [x] |
| A5 | Extend `generate_value_to_option_field` (and non-option `generate_value_to_field` if needed) to match on `ChronoDateTimeUtc` / `ChronoDateTimeLocal` arms | `lifeguard-derive/src/type_conversion.rs` | [x] |
| A6 | Update module doc comment at top of `type_conversion.rs` with canonical table (PRD §2) | `lifeguard-derive/src/type_conversion.rs` | [x] |
| A7 | Add unit tests: parsed types `DateTime<Utc>`, `Option<DateTime<Utc>>` produce expected `TokenStream` / no `String(None)` fallback | `lifeguard-derive/src/type_conversion.rs` (tests) or `lifeguard-derive/tests/` | [x] |
| A8 | Add derive integration-style test: minimal `LifeModel` with `created_at: DateTime<Utc>` — compile + assert `get(Column::CreatedAt)` maps to correct `Value` (pattern used elsewhere in crate) | `lifeguard-derive/tests/` or `test_minimal.rs` | [x] |

**Exit:** PRD Iteration A exit criteria met.

---

## Iteration B — `FromRow` + SQL type inference

**Goal:** Load `timestamptz` into `DateTime<Utc>`; optional columns; `infer_sql_type_from_rust_type` knows UTC/local.

| # | Task | Primary files | Done |
|---|------|---------------|------|
| B1 | `infer_sql_type_from_rust_type`: last-segment / generic handling for `DateTime` + `Utc` → `TIMESTAMP WITH TIME ZONE` (align with `#[column_type]` conventions in repo) | `lifeguard-derive/src/macros/life_model.rs` (`infer_sql_type_from_rust_type`) | [x] |
| B2 | `infer_sql_type_from_rust_type`: `DateTime<Local>` → document chosen SQL string (or defer Local inference if unsupported) | `lifeguard-derive/src/macros/life_model.rs` | [x] |
| B3 | `FromRow` generation: branch on `is_datetime_utc_type(inner_type)` — `row.try_get` to `DateTime<Utc>` / `Option<…>` (match nullable pattern used for UUID) | `lifeguard-derive/src/macros/life_model.rs` (~1464–1530) | [x] |
| B4 | Same for `DateTime<Local>` if in scope | `lifeguard-derive/src/macros/life_model.rs` | [x] |
| B5 | Document chosen strategy in PRD or `docs/` if `SystemTime` bridge kept for any path | `docs/COMPLETE_CHRONO_IMPLEMENTATION.md` or new note | [x] |
| B6 | DB integration tests: `timestamptz` round-trip (see PRD §6 T1, T2) — use existing lifeguard DB test harness if any | `lifeguard/tests/` or `lifeguard-derive` + `docker`/native per ADR | [x] |

**Exit:** PRD Iteration B exit criteria met.

---

## Iteration C — `LifeRecord::insert` / update parity

**Goal:** `Record::insert` / updates emit `ChronoDateTimeUtc` for `DateTime<Utc>` fields; soft-delete hooks respect TZ column semantics where derivable.

| # | Task | Primary files | Done |
|---|------|---------------|------|
| C1 | Audit generated `insert` in `life_record.rs`: `Expr::val` / `get()` path uses UTC `Value` when model field is `DateTime<Utc>` | `lifeguard-derive/src/macros/life_record.rs` | [x] |
| C2 | Soft-delete / `updated_at` / `deleted_at` generated snippets: plan migration from `naive_utc()` only — either column-type aware or document “naive columns only” until model migration | `lifeguard-derive/src/macros/life_record.rs` (~896–912) | [x] |
| C3 | Integration test: insert model with `DateTime<Utc>`, select back | tests | [x] |
| C4 | Regression: `NaiveDateTime` insert path unchanged (PRD T3) | tests | [x] |

**Exit:** PRD Iteration C exit criteria met.

---

## Iteration D — NULL semantics + `converted_params` hardening

**Goal:** Typed NULLs for chrono; reduce generic `nulls` bucket mistakes for mixed inserts.

| # | Task | Primary files | Done |
|---|------|---------------|------|
| D1 | Audit `generate_option_field_to_value`: `None` for `Option<DateTime<Utc>>` → `ChronoDateTimeUtc(None)` not ambiguous fallback | `lifeguard-derive/src/type_conversion.rs` | [x] |
| D2 | Review `converted_params.rs` first + second pass: comment any invariant about order / bucket per `Value` variant | `lifeguard/src/query/converted_params.rs` | [x] |
| D3 | Unit tests: mixed `String` / `Json` / `ChronoDateTimeUtc` NULLs in one `Values` slice (Hauliage-style) | `lifeguard/src/query/converted_params.rs` `#[cfg(test)]` | [x] |
| D4 | Optional follow-up: separate issue for `String(None)`/`Json(None)` vs typed nulls if out of scope for D | issue / PRD note | [ ] |

**Exit:** PRD Iteration D exit criteria met.

---

## Iteration E — Docs + release

| # | Task | Primary files | Done |
|---|------|---------------|------|
| E1 | Add `CHRONO_AND_POSTGRES_TYPES.md` **or** extend `UUID_AND_POSTGRES_TYPES.md` with PRD §2 table + examples | `lifeguard/docs/` | [x] |
| E2 | `CHANGELOG.md`: version, additive vs breaking | repo root | [x] |
| E3 | `lifeguard-migrate`: README or `schema_infer` doc — infer-schema requires Lifeguard ≥ *version* for full `DateTime<Utc>` support | `lifeguard-migrate/` | [x] |
| E4 | `rustdoc` examples compile (`cargo doc --no-deps` if CI uses it) | — | [x] |
| E5 | Mark PRD §9 success criteria checkboxes | `COMPLETE_CHRONO_IMPLEMENTATION.md` | [x] |

**Exit:** PRD Iteration E exit criteria met.

---

## Test matrix (PRD §6) — tracking

| Case | Description | Iteration | Done |
|------|-------------|-----------|------|
| T1 | `timestamptz` / `DateTime<Utc>` insert + select | B–C | [x] |
| T2 | `Option<DateTime<Utc>>` NULL + Some | B–D | [x] |
| T3 | `timestamp` / `NaiveDateTime` regression | A–C | [x] |
| T4 | `date` / `NaiveDate` regression | existing | [ ] |
| T5 | `DateTime<Local>` (if in scope) | A–B | [ ] |
| T6 | Mixed UUID + JSON + `DateTime<Utc>` + `i16` | C–D | [ ] |

---

## `lifeguard-migrate` verification (cross-cutting)

| # | Task | Primary files | Done |
|---|------|---------------|------|
| M1 | Confirm `map_pg_to_rust` timestamptz → `DateTime<Utc>` still matches derive after Iteration B | `lifeguard-migrate/src/schema_infer.rs` | [x] |
| M2 | Golden / unit test: inferred Rust type string for `timestamptz` | `lifeguard-migrate/tests/` or `schema_infer.rs` tests | [x] |

---

## Notes

- **Depends on:** `chrono` feature flags in `sea-query` / workspace — already used for `ChronoDateTimeUtc` in `converted_params`.
- **Do not** change Hauliage in this repository; track downstream migration separately.

---

*Last aligned with PRD: `COMPLETE_CHRONO_IMPLEMENTATION.md` (full file).*
