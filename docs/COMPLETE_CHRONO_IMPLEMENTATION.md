# PRD: Complete Chrono support in Lifeguard (derive + runtime alignment)

> **Document:** `docs/COMPLETE_CHRONO_IMPLEMENTATION.md`  
> **Status:** Draft — implementation plan  
> **Audience:** Lifeguard maintainers; downstream (e.g. Hauliage) after library releases  
> **Task tracker:** `[COMPLETE_CHRONO_TASKS.md](./COMPLETE_CHRONO_TASKS.md)` — actionable checklist (Iterations A–E, test matrix T1–T6, migrate checks)  
> **Related:** `[UUID_AND_POSTGRES_TYPES.md](./UUID_AND_POSTGRES_TYPES.md)`, `[lifeguard/src/query/converted_params.rs](../src/query/converted_params.rs)`, `[lifeguard-derive/src/type_conversion.rs](../lifeguard-derive/src/type_conversion.rs)`

---

## 1. Executive summary

### 1.1 Problem

Lifeguard’s **derive layer** (`lifeguard-derive`) only treats `**chrono::NaiveDateTime`** and `**chrono::NaiveDate**` as first-class types when mapping Rust fields to `**sea_query::Value**` and when generating `**FromRow**`. Types such as `**chrono::DateTime<chrono::Utc>**` and `**chrono::DateTime<chrono::Local>**` are **not** recognized: the last path segment is `DateTime`, not `NaiveDateTime`, so codegen falls through to generic integer/string branches or `**Value::String(None)`**, causing **silent bugs** or **PostgreSQL bind errors** at runtime.

The **runtime** path (`lifeguard::query::converted_params`) already supports `**Value::ChronoDateTime`**, `**ChronoDateTimeUtc**`, and `**ChronoDateTimeLocal**` with correct `ToSql` buckets. The gap is **not** Postgres or `may_postgres` fundamentals; it is **derive + LifeModel `FromRow` + SQL type inference** staying naive-only.

### 1.2 Product inconsistency (must fix)

`**lifeguard-migrate` schema inference** already maps:

- `timestamp with time zone` / `timestamptz` → `**chrono::DateTime<chrono::Utc>`**
- `timestamp` / `timestamp without time zone` → `**chrono::NaiveDateTime**`

So `**infer-schema` can emit structs that `LifeModel` / `LifeRecord` do not fully support** until this PRD is delivered.

### 1.3 Goal

Implement **end-to-end** support for the chrono types Lifeguard should officially support, with **clear PostgreSQL semantics**, **iterative delivery**, and **tests per iteration**. After Lifeguard releases, **Hauliage** (and other consumers) can migrate `**TIMESTAMP WITH TIME ZONE`** columns to `**DateTime<Utc>**` (and optional `Local` where appropriate) and remove ad-hoc workarounds.

### 1.4 Non-goals (initial phases)

- Replacing `**NaiveDateTime**` for `**timestamp without time zone**` — remains valid; no forced migration.
- Full `**time` crate** support — out of scope unless explicitly added later.
- Automatic migration of all Hauliage models — **follow-up project** after Lifeguard ships (see [§8](#8-follow-up-hauliage-consumer-updates)).

---

## 2. Type mapping policy (canonical)


| PostgreSQL                                     | Rust type (canonical)             | `sea_query::Value` variant |
| ---------------------------------------------- | --------------------------------- | -------------------------- |
| `timestamp without time zone`                  | `chrono::NaiveDateTime`           | `ChronoDateTime`           |
| `timestamp with time zone`                     | `chrono::DateTime<chrono::Utc>`   | `ChronoDateTimeUtc`        |
| `date`                                         | `chrono::NaiveDate`               | `ChronoDate`               |
| (rare) app choice for tz-aware local wall time | `chrono::DateTime<chrono::Local>` | `ChronoDateTimeLocal`      |


**Optional columns:** `Option<T>` for each of the above, with **typed NULL** paths (`ChronoDateTimeUtc(None)`, etc.) — see [§5.2](#52-null-handling-generic-vs-typed).

`**DateTime<FixedOffset>`:** defer to a later iteration unless a concrete consumer requires it; detection and `Value` mapping must be designed if added (may map to `ChronoDateTimeUtc` after normalization or use a dedicated strategy).

---

## 3. Technical scope (by crate)

### 3.1 `lifeguard-derive` — `type_conversion.rs`

- Add detectors, e.g.:
  - `is_datetime_utc_type(ty)` — matches `chrono::DateTime<Utc>` (qualified and `use` patterns).
  - `is_datetime_local_type(ty)` — matches `chrono::DateTime<Local>`.
- Extend `**generate_field_to_value`**, `**generate_option_field_to_value**`, `**generate_option_field_to_value_with_default**` so:
  - `DateTime<Utc>` → `Value::ChronoDateTimeUtc(Some(...))`
  - `DateTime<Local>` → `Value::ChronoDateTimeLocal(Some(...))`
  - Keep existing naive mappings unchanged.
- Extend `**generate_value_to_field**` / `**generate_value_to_option_field**` (Record setters / `get`) to **match** the same `Value` variants (mirrors existing `ChronoDateTime` / `ChronoDate` arms).
- **Documentation** at top of `type_conversion.rs`: list supported chrono types and required `Value` variants (per [§2](#2-type-mapping-policy-canonical)).

### 3.2 `lifeguard-derive` — `life_model.rs`

- `**infer_sql_type_from_rust_type`:** map `DateTime<Utc>` → e.g. `**TIMESTAMP WITH TIME ZONE`** (exact string must match project conventions / `#[column_type]` usage).
- `**FromRow` generation:** for `DateTime<Utc>` / `DateTime<Local>`:
  - Prefer `**row.try_get::<'_, _, chrono::DateTime<chrono::Utc>>`** (and `Local`) where the `postgres` stack supports it for `timestamptz` / `timestamp`, **or** document a single intermediate (`SystemTime`) path with explicit conversion — **choose one strategy per type and test it**.
- Today’s `**NaiveDateTime`** path uses `SystemTime` → `naive_utc()`; keep behavior for `**timestamp without time zone**`; do not change semantics without tests.

**FromRow strategy (implemented):**

- **`DateTime<Utc>`** and **`DateTime<Local>`** use **direct** `row.try_get` to `chrono::DateTime<chrono::{Utc,Local}>` (nullable: `Option<…>`). This relies on `may_postgres` / `postgres` **chrono** `FromSql` for PostgreSQL `timestamptz` (and compatible timestamp OIDs).
- **`NaiveDateTime`** remains on the **`SystemTime` → `DateTime::<Utc>::from(st).naive_utc()`** bridge so **`timestamp without time zone`** behavior stays unchanged and does not assume tz-aware Rust types.

**Soft-delete `UPDATE` (implemented):** For `#[soft_delete]` (and optional `auto_timestamp` on the soft-delete update), `deleted_at` / `updated_at` “now” values use `type_conversion::generate_expr_val_now_for_field_type` so **`Option<DateTime<Utc>>`** columns get **`ChronoDateTimeUtc`**, **`NaiveDateTime`** stays **`ChronoDateTime`**. Table-level **`#[auto_timestamp]`** `before_insert` / `before_update` still assign **stringified naive** timestamps when enabled (legacy `Option<String>` pattern); migrate those hooks separately if models use chrono fields.

### 3.3 `lifeguard-derive` — `life_record.rs`

- Soft-delete / auto `**updated_at` / `deleted_at**` helpers currently use `**Utc::now().naive_utc()**` and thus `**ChronoDateTime**`. Decide per field SQL type:
  - If column is `**timestamptz**`, generated code should use `**DateTime<Utc>**` and `**Value::ChronoDateTimeUtc**` once models migrate.
  - Iteration 1 may keep naive if models remain naive; Iteration 2+ aligns with [§2](#2-type-mapping-policy-canonical).

### 3.4 `lifeguard` runtime — `converted_params.rs`

- **No change required** for UTC/local **if** derives emit correct `Value` variants.
- **Optional hardening (later iteration):** audit generic `**nulls` (`Option<i32>`)** usage for `String(None)` / `Json(None)` vs typed null buckets — orthogonal but related to optional columns (see [§5.2](#52-null-handling-generic-vs-typed)).

### 3.5 `lifeguard-migrate`

- **Already** maps `timestamptz` → `DateTime<Utc>` in `map_pg_to_rust` — verify **golden tests** after derive changes.
- **Docs:** note that inferred types assume Lifeguard ≥ version **TBD** for full support.

---

## 4. Phased delivery (iterations)

Each iteration must: **merge with green CI**, **add/adjust tests**, and **update this doc’s checklist** (or issue links).

### Iteration A — Detection + `Value` mapping (derive only)

**Objective:** `DateTime<Utc>` / `DateTime<Local>` on **non-option** and `**Option<>`** fields produce correct `**sea_query::Value**` in `**get()**` / Record paths; no `FromRow` change yet if tests use manual models.

**Tasks**

- Implement `is_datetime_utc_type` / `is_datetime_local_type` (syn path + generic arg parsing; cover `chrono::DateTime<Utc>` and `use` re-exports).
- Wire `**generate_field_to_value`** and `**generate_option_field_to_value**` (+ with_default if applicable).
- Wire `**generate_value_to_field**` / `**generate_value_to_option_field**` for ActiveModel set paths.
- Unit tests in `lifeguard-derive` (expand or `trybuild`-style) proving **no fallthrough** to `String(None)` for `DateTime<Utc>`.

**Testing**

- Derive tests: model with `created_at: DateTime<Utc>`, `updated_at: Option<DateTime<Utc>>`, assert generated `get(Column::...)` returns expected `Value` variant (snapshot or helper).
- **Integration (optional this iteration):** `execute_values` round-trip against Postgres for a minimal `INSERT` built from `Value::ChronoDateTimeUtc` (can live in `lifeguard` tests if harness exists).

**Exit criteria:** Codegen compiles; tests prove Value mapping; no Hauliage change required yet.

---

### Iteration B — `FromRow` + SQL type inference

**Objective:** `LifeModel` can **load** `timestamptz` into `**DateTime<Utc>`** and optional variants; `**infer_sql_type_from_rust_type**` matches [§2](#2-type-mapping-policy-canonical).

**Tasks**

- `infer_sql_type_from_rust_type`: `DateTime<Utc>` → `TIMESTAMP WITH TIME ZONE` (or project-standard string).
- `FromRow`: implement extraction for `DateTime<Utc>` / `Option<DateTime<Utc>>` / `Local` variants.
- Align with `**postgres` / `tokio-postgres`** `FromSql` for `timestamptz`; add fallback or document if `SystemTime` bridge remains for edge backends.

**Testing**

- DB integration tests: table with `timestamptz`, insert known instant, `SELECT` into `DateTime<Utc>`, compare (timezone boundary case: UTC vs offset).
- Nullable column: `NULL` → `Option::None`.

**Exit criteria:** Inferred-schema structs using `DateTime<Utc>` round-trip on read.

---

### Iteration C — `LifeRecord::insert` / update path parity

**Objective:** `**Record::insert`** and common update paths use `**ChronoDateTimeUtc**` when the model field is `**DateTime<Utc>**` (no silent naive coercion).

**Tasks**

- Audit `**life_record.rs`** `insert_column_checks` / `Expr::val` — ensure `**get()` → Expr::val** chain uses UTC `Value` for UTC fields.
- Soft-delete / timestamp hooks: if column metadata is `timestamptz`, emit `**Utc::now()`** as `**ChronoDateTimeUtc**`, not `naive_utc()` only.

**Testing**

- Integration: `insert` row with `DateTime<Utc>`, read back via `SELECT` / model.
- Regression: existing `**NaiveDateTime`** inserts unchanged.

**Exit criteria:** ORM insert/update matches [§2](#2-type-mapping-policy-canonical) without manual `sea_query` in apps.

---

### Iteration D — Optional / NULL semantics and `converted_params` hardening

**Objective:** Optional `**Option<DateTime<Utc>>`** and SQL **NULL** use **typed** null `Value` variants end-to-end; reduce reliance on generic `**nulls`** for typed columns where bugs were observed downstream.

**Tasks**

- Audit `**generate_option_field_to_value`** for `None` → `ChronoDateTimeUtc(None)` vs generic null.
- Review `**converted_params.rs**` second pass: ensure every `Value::* (None)` uses the **correct** null vector (document in code comments).
- Add tests for `**String(None)`**, `**Json(None)**`, `**ChronoDateTimeUtc(None)**` in sequence (reproduce Hauliage-style multi-column inserts).

**Testing**

- Unit: `with_converted_value_slice` / executor mock for mixed NULL types.
- Integration: `INSERT` omitting vs explicit NULL for optional timestamptz (as supported by API).

**Exit criteria:** No `**Option<i32>` vs `varchar` / `timestamptz`** class of errors for supported types.

---

### Iteration E — Documentation, migration guide, release

**Tasks**

- Update `[UUID_AND_POSTGRES_TYPES.md](./UUID_AND_POSTGRES_TYPES.md)` (or add `**CHRONO_AND_POSTGRES_TYPES.md`**) with the [§2](#2-type-mapping-policy-canonical) table and examples for `LifeModel` / `LifeRecord`.
- `**CHANGELOG.md**` / release notes: minimum Lifeguard version, breaking vs additive notes (naive remains).
- Optional: `lifeguard-migrate` README note on `**infer-schema**` + derive version coupling.

**Testing**

- Doc examples compile in `rustdoc` where applicable.

**Exit criteria:** Consumers can adopt without reading source.

---

## 5. Cross-cutting requirements

### 5.1 Consistency across three conversion functions

Per existing comment in `type_conversion.rs`, `**generate_field_to_value`**, `**generate_option_field_to_value**`, and `**generate_option_field_to_value_with_default**` must use the **same** `Value` variant for each Rust type so Model vs Record `**get()`** agree.

### 5.2 NULL handling (generic vs typed)

Downstream observed failures when `**Value::String(None)**` / `**Json(None)**` routed through the **generic `nulls`** bucket (`Option<i32>` placeholders). Full chrono support must not repeat that pattern for `**ChronoDateTimeUtc(None)**`: use **typed** null slots already present in `**converted_params.rs`**.

### 5.3 Backward compatibility

- Existing models using `**NaiveDateTime**` for `**timestamp**` / `**timestamp without time zone**` remain supported.
- Adding `**DateTime<Utc>**` support is **additive** unless we intentionally deprecate a pattern (not in initial phases).

---

## 6. Test matrix (minimum)


| Case | PG type       | Rust                                  | Operation                        |
| ---- | ------------- | ------------------------------------- | -------------------------------- |
| T1   | `timestamptz` | `DateTime<Utc>`                       | Insert + select                  |
| T2   | `timestamptz` | `Option<DateTime<Utc>>`               | NULL + Some                      |
| T3   | `timestamp`   | `NaiveDateTime`                       | Insert + select (regression)     |
| T4   | `date`        | `NaiveDate`                           | Insert + select (regression)     |
| T5   | `timestamptz` | `DateTime<Local>`                     | If in scope — optional iteration |
| T6   | Mixed row     | UUID + JSON + `DateTime<Utc>` + `i16` | Bind order / NULL typing         |


**Performance:** no new allocations on hot path beyond current `Value` mapping.

---

## 7. Risks and mitigations


| Risk                                                         | Mitigation                                                                            |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------- |
| `FromRow` / `try_get` differences between sync/async drivers | Integration tests on target stack (`may_postgres`); document any `SystemTime` bridge. |
| `infer-schema` vs hand-written models diverge                | Golden tests for `map_pg_to_rust`; CI job for infer + compile.                        |
| Scope creep (`FixedOffset`, `time`)                          | Explicit non-goals; separate PRD spike if needed.                                     |


---

## 8. Follow-up: Hauliage consumer updates

**After** a Lifeguard release containing Iterations **A–E** (or agreed subset):

1. **Inventory:** grep Hauliage `microservices/**/models` for `NaiveDateTime` + `#[column_type = "TIMESTAMP WITH TIME ZONE"]` (or `timestamptz` in migrations).
2. **Migrate:** switch to `**DateTime<Utc>`** where DB is `**timestamptz**`; keep `**NaiveDateTime**` where DB is `**timestamp**` without TZ.
3. **Remove workarounds:** e.g. manual `sea_query::Query::insert` in services that only existed to bypass derive — replace with `**Record::insert`** where safe.
4. **Re-run** service BDD/integration tests and **E2E** where timestamps matter.

Track as a **separate milestone** (Hauliage repo); this PRD stops at **Lifeguard deliverables**.

---

## 9. Success criteria (Lifeguard)

- `chrono::DateTime<chrono::Utc>` and `Option<…>` are **first-class** in derive: correct `**sea_query::Value`**, `**FromRow**`, and **SQL type inference** for TZ-aware columns.
- `**lifeguard-migrate` infer-schema** output compiles and round-trips against Postgres without manual fixes.
- Tests cover mapping table in [§6](#6-test-matrix-minimum).
- Documentation and changelog allow Hauliage to plan the [§8](#8-follow-up-hauliage-consumer-updates) rollout.

---

## 10. Checklist summary


| Iteration | Theme                              | Ship?                |
| --------- | ---------------------------------- | -------------------- |
| A         | Value mapping + type detection     | ✅                    |
| B         | FromRow + infer_sql_type           | ✅                    |
| C         | Insert/update / soft-delete parity | ✅                    |
| D         | NULL / converted_params hardening  | ✅                    |
| E         | Docs + release                     | ✅                    |
| —         | Hauliage migration                 | **Separate project** |


---

*End of PRD.*