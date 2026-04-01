# Design: `compare-schema` index parity — **T2b** (btree opclass) & **T3** (expression keys)

**Audience:** implementers of `lifeguard_migrate::schema_migration_compare`, migration SQL generation, and optional derive extensions.  
**Status:** **T2b** / **T3** partially implemented in code — this document remains the deeper design; update code/README/roadmap when behavior changes.  
**Parent:** [DESIGN_INDEX_COMPARE_ROADMAP.md](./DESIGN_INDEX_COMPARE_ROADMAP.md) (shipped **T1** / **T2** / **T4** summarized there).  
**Related code:** [`lifeguard-migrate/src/schema_migration_compare.rs`](../../lifeguard-migrate/src/schema_migration_compare.rs), [`lifeguard-migrate/src/sql_generator.rs`](../../lifeguard-migrate/src/sql_generator.rs), [`src/query/table/definition.rs`](../../src/query/table/definition.rs) (`IndexDefinition`).

---

## 1. Why this document

Tracks **T2b** and **T3** are deliberately **not** covered by today’s shipped comparators. They need a clear contract on:

- what PostgreSQL can express on indexes,
- what Lifeguard entities and `sql_generator` can express today,
- whether comparison should use **string surfaces** (`pg_indexes.indexdef`, merged migration text) or **catalog IR** (`pg_index`, `pg_class`, `pg_opclass`, …),
- how new drifts interact with **T1** normalized string compare and **index column** drift.

This document scopes work so implementation can proceed in **independent phases** without re-litigating goals.

---

## 2. Current behavior (baseline)

| Mechanism | What it compares | Blind spots relevant to T2b / T3 |
|-----------|------------------|-----------------------------------|
| **T1** — `normalize_index_statement_for_compare` | Full normalized `CREATE INDEX` string when migration + live share index name | Collapses whitespace and strips redundant `USING btree`; does **not** canonicalize **opclass**, **collation**, **ASC/DESC**, **NULLS**, or **parenthesized expressions**. Many real divergences still surface as T1 text drift, but reporting is opaque and normalization may be insufficient for stable equality. |
| **Column / INCLUDE names** — `parse_pg_indexdef_simple_columns` / `parse_pg_indexdef_include_columns` | Plain column names when the key list parses as comma-separated **simple** segments | `first_simple_index_column` takes the **first** token of each segment after stripping a leading `COLLATE` clause. It **ignores** trailing **`jsonb_path_ops`**, **`text_pattern_ops`**, **`uuid_ops`**, etc. So **T2b** issues can be **invisible** to this path. Returns **`None`** when any key segment starts with `(` → **expression index** → **T3** skipped. |
| **T2** — `parse_pg_indexdef_access_method` | Access method ≠ `btree` | Correctly scoped; does not address **same** access method, **different** btree opclass. |

**Entity / codegen model:** [`IndexDefinition`](../../src/query/table/definition.rs) has **`key_parts: Vec<IndexKeyPart>`** (column vs expression, optional opclass / collation / sort / nulls), **`columns`** (flattened coverage for validation), legacy **`key_list_sql`** when parts are empty, `include_columns`, `unique`, `partial_where`. [`sql_generator`](../../lifeguard-migrate/src/sql_generator.rs) prefers **`format_index_key_list_sql`** over `key_list_sql` / bare `columns`. **`infer-schema`** emits **`#[index]`** from btree catalog metadata where expression coverage can be inferred.

---

## 3. Track **T2b** — Btree operator class parity

### 3.1 Definitions

- **Access method** (`USING btree` / `gin` / …): already covered by **T2**.
- **Operator class** (opclass): per **index key column** (or expression), PostgreSQL attaches an **opclass** that defines how values are ordered/compared for that index. For **btree**, types have a **default** opclass (e.g. `text_ops` for `text`, `jsonb_ops` for `jsonb`). Indexes may specify a **non-default** btree opclass, e.g. **`text_pattern_ops`** on `text`. **`jsonb_path_ops`** is for **`GIN`** on `jsonb`, not btree — do not use it in `USING btree (...)` (PostgreSQL error `E42704`).

### 3.2 Why teams care

- **JSONB / GIN:** `jsonb_path_ops` vs default (on **GIN**) changes index capabilities; that path is **T2** (access method), not btree **T2b**. **Btree** `text` + `text_pattern_ops` vs `text_ops` affects `LIKE` / pattern matching compatibility — migrations that omit it can pass column-name checks but be **wrong** for query plans.
- **Text / UUID / inet:** pattern ops and type-specific opclasses affect operator compatibility (`LIKE`, etc.).

### 3.3 Problem statement

For a **btree** index on a shared table, when the **migration** (or entity expectation) implies **default** opclasses for named columns, **live** may use **explicit** non-default opclasses. Today:

- **T2** does not fire (still btree).
- **T1** may or may not show a difference depending on how `pg_get_indexdef` formats the statement and how normalization behaves.
- **Column-name** parsing **drops** opclass tokens, so it does not detect the drift.

### 3.4 Desired outcomes

1. **Detect** per-key opclass mismatch for btree indexes where we can resolve **expected** vs **actual**.
2. **Report** structured drift (index name, table, key position, expected opclass name, actual opclass name) suitable for CI messages.
3. **Avoid** duplicating **T1** noise where possible: prefer **structured T2b** when both sides are “simple column keys”; keep **T1** as backstop for hand-written SQL.

### 3.5 Design options

#### Option A — Extend `indexdef` string parsing only

- After each simple column token (handling quoted identifiers), parse optional **`COLLATE`**, optional **opclass** (identifier or `schema.opclass`), optional **`ASC`/`DESC`**, optional **`NULLS FIRST/LAST`**.
- **Pros:** No extra DB round-trips beyond existing `pg_indexes` fetch.  
- **Cons:** Must track PostgreSQL **grammar variants** and `pg_get_indexdef` formatting differences across versions; easy to get wrong on edge cases.

#### Option B — Catalog query (recommended spine)

- For each non-pkey index on shared tables with `pg_index.indisvalid` and btree access method, join:

  - `pg_index` → `pg_class` (index relation),
  - `pg_index.indclass` (OID vector of opclasses per key column),
  - `pg_opclass` → `pg_am` to filter **`amname = 'btree'`**,
  - map key **attribute number** via `indkey` to table column names where the key is a **simple column** (not expression).

- For **expression** keys, `indkey` may reference **zero** or special markers — align with PostgreSQL docs for **expression / predicate** indexes; opclass still exists per index column slot.

- **Pros:** Authoritative; avoids parsing `indexdef` for opclass.  
- **Cons:** More SQL and Lifeguard executor code; must handle **partitioned** / **ONLY** / **invalid** indexes consistently with current `compare-schema` filters.

#### Option C — Hybrid

- Use **catalog** for opclass OIDs → names; use **`indexdef` parsing** only as a fallback when catalog access is restricted (unlikely in same tool that already runs `pg_indexes`).

### 3.6 Expected baseline (“migration side”)

| Source | Implied opclass |
|--------|------------------|
| Entity-driven `sql_generator` output | **Default** opclass for each column’s type (must match PostgreSQL’s default for that type in the target version). |
| Hand-written `CREATE INDEX` in merged migration | Parse from migration text (Option A) **or** treat as opaque and rely on **T1** only until parsed. |

**Product decision (to lock before coding):** For T2b v1, is the **expected** opclass always “**default for declared column type**” when `IndexDefinition` has only names? That requires either:

- **type-aware** comparison (need column types from `information_schema` or `pg_attribute`), or  
- **conservative** reporting: “live uses non-default opclass `X` on key position `k`” without asserting migration expectation (weaker but avoids type default table).

Recommendation: **phase 1** = report **actual** opclass per btree key slot from catalog + flag when **any** key uses non-default opclass; **phase 2** = compare to **default for `pg_attribute` type OID** for simple column keys.

### 3.7 Interaction with T1

- If **T1** already reports text mismatch for the same index, **T2b** may duplicate. **Policy:** when **T2b** fires for a specific index, optionally **suppress** T1 for that index **only if** T1 difference is **fully explained** by opclass/collation/direction (hard). **Pragmatic v1:** allow **both**; document precedence for human readers, or emit T2b **first** and skip T1 when catalog says btree + simple columns only (configuration flag).

### 3.8 Suggested deliverables (T2b)

1. **Spike SQL:** **Done** — see [`fetch_live_btree_index_key_opclasses`](../../lifeguard-migrate/src/schema_migration_compare.rs): `pg_index` + `generate_subscripts(indkey::int2[], 1)` + `indclass::oid[]` + `pg_opclass` + default opclass via `opcdefault` for btree. **Shipped in `compare-schema`:** non-default key opclasses on **shared** tables populate `MigrationDbCompareReport::index_btree_nondefault_opclass_drifts`.
2. **Rust types:** **Done** — `LiveBtreeIndexKeyOpclassRow` (raw catalog rows) and `IndexBtreeNonDefaultOpclassDrift` (report).
3. **Tests:** **Done** — `migration_db_compare_smoke`: `fetch_live_btree_index_key_opclasses_lists_text_pattern_ops`, `compare_reports_btree_non_default_opclass_when_live_uses_text_pattern_ops` (require DB URL; uses btree `text_pattern_ops` vs default `text_ops` — not `jsonb_path_ops`, which is GIN-only).
4. **Docs:** `lifeguard-migrate/README` limits table updated; cross-link this file.

**Follow-on (still T2b backlog):** migration-side **expected** opclass from entity/column types; dedupe with **T1** when text mismatch is only opclass formatting; collation / sort order reporting.

---

## 4. Track **T3** — Expression & functional index keys

### 4.1 Definitions

- **Expression index:** index key is not a bare column reference but an expression, e.g. `CREATE INDEX ON t ((lower(email)))`, `((a + b))`.
- **Functional** is used interchangeably in docs; PostgreSQL stores **expressions** in `pg_index` / system catalogs separately from simple column references.

### 4.2 Why teams care

- Common for **case-insensitive** search, **computed** keys, **partial** unique constraints on expressions.
- ORM-generated schemas often **omit** these unless hand-maintained; `compare-schema` should not stay silent.

### 4.3 Problem statement

- `parse_pg_indexdef_simple_columns` returns **`None`** for expression indexes → **no** column drift; **T1** compares full strings **if** the migration contains a matching `CREATE INDEX` line with **character-for-character** compatible text after normalization — often **false negatives** (migration missing) or **brittle** (whitespace, casting, implicit parentheses).

### 4.4 Desired outcomes

1. **Classify** each live index key slot as **simple column**, **expression**, or **unknown** (parser failure).
2. When migration/entity has **only** simple columns for that index name but live has **expressions**, emit a clear **structural** drift (not just “text differs”).
3. Longer term: optional **entity** representation for a **subset** of safe expressions (out of scope for v1 unless product insists).

### 4.5 Design options

#### Option A — Catalog-first (recommended)

- Use `pg_index` **`indkey`** and **`indexprs`** (or `pg_get_indexdef` / `pg_get_expr` with appropriate catalog OIDs) to recover **per-key** whether the key is a **column reference** or an **expression tree**.
- Serialize expression for comparison: e.g. `pg_get_expr(indexprs[i], …)` normalized (whitespace, optional parenthesis rules).
- **Migration side:** parse `CREATE INDEX` key list from merged baseline (already partially done for T1 extraction); normalize expression **substrings** for comparison.

**Pros:** Ground truth matches planner.  
**Cons:** Requires **superuser or sufficient privileges**? (Usually same as `pg_indexes` for app schema.) Implementation complexity in Rust + SQL.

#### Option B — String-only / fingerprint

- Normalize key-list substring from `indexdef` (between first `(` after table and matching `)` before `INCLUDE` / `WHERE`) and compare to migration substring with aggressive whitespace normalization.

**Pros:** Reuses text surfaces.  
**Cons:** Still brittle across PG versions and quoting; does not fix “missing index in migration” beyond T1/T2.

#### Option C — Extend `IndexDefinition` / `#[index]` (**v2 shipped**)

- **`IndexDefinition::key_parts`** — structured btree segments ([`IndexKeyPart::Column`](../../src/query/table/definition.rs) vs [`IndexKeyPart::Expression`](../../src/query/table/definition.rs)); optional **`key_list_sql`** when parts are empty (verbatim legacy).
- **Derive grammar:** top-level key segments split on commas at **paren depth 0**; per segment either **structured column** (`col`, `col text_pattern_ops`, `col COLLATE "C" pat_ops`, `col DESC NULLS FIRST`, …) or **`expr | col1, col2`** (space-pipe-space) for expressions / mixed lists.
- **`format_index_key_list_derive_value`** vs **`format_index_key_list_sql`:** derive round-trip strings use `expr | coverage` inside the attribute; `CREATE INDEX` SQL omits the coverage tail.
- **`infer-schema`:** non-primary **btree** indexes → `#[index = "..."]` using the same model (skips expression indexes when coverage tokens match no table column).
- **Security:** expression / raw fragments remain **trusted author input**.

**Pros:** Single source of truth in Rust; aligns generated SQL with **T2b/T3** compare when authors opt in.  
**Cons:** Authors must maintain valid PostgreSQL; no automatic normalization vs live `indexdef` beyond compare tooling.

### 4.6 Suggested drift taxonomy (T3)

| Case | Suggested reporting |
|------|---------------------|
| Live index has **expression** key; migration has **no** index line | `IndexOnlyInDatabase` (existing) **or** dedicated `IndexExpressionKeyDrift` with reason |
| Live **expression**; migration has **simple-column** index same name | **Structural** mismatch: “live uses expression key; migration lists columns …” |
| Live **expression**; migration has **expression** | Compare **normalized** expression text (Option A+B) or rely on **T1** until normalized expr compare exists |
| Parser / catalog failure | **Explicit** “unclassified index key” row (do not silently skip) |

### 4.7 Interaction with T1 / T2b

- **Expression** keys still have **opclasses** in btree → **T2b** applies per **key slot** where catalog exposes opclass for that slot.
- **T1** remains the **escape hatch** when structured compares are incomplete.

### 4.8 Suggested deliverables (T3)

1. **Spike:** SQL to list index keys with `pg_attribute.attname` **or** `pg_get_expr` output per ordinality. **Done (v1):** [`fetch_live_btree_expression_index_key_slots`](../../lifeguard-migrate/src/schema_migration_compare.rs) uses `pg_index.indkey` = `0` and `pg_get_indexdef(index_oid, key_ord, false)` per slot.
2. **Rust:** `IndexKeyKind` enum `SimpleColumn { name }` / `Expression { normalized_text }` / `Unknown`. **Partial:** [`IndexExpressionKeyVsSimpleMigrationDrift`](../../lifeguard-migrate/src/schema_migration_compare.rs) + report field (not a full per-key enum yet).
3. **Compare function:** given merged migration index statement + live catalog row → drift vec. **Partial:** [`compare_generated_dir_to_live_db`](../../lifeguard-migrate/src/schema_migration_compare.rs) emits T3 drift when migration parses as simple keys only; **T1** suppressed for that index.
4. **Tests:** expression index fixtures (`lower()`, binary op, cast); integration with scratch schema (pattern after `migration_db_compare_smoke.rs`). **Partial:** `fetch_live_btree_expression_index_key_slots_lists_lower_email`, `compare_reports_expression_key_when_migration_lists_simple_columns_only`, `compare_t3_v2_skips_t1_when_expression_indexdefs_normalize_equal`, `compare_reports_ordering_drift_when_migration_desc_not_live_asc`.
5. **T3 v2 (shipped):** [`fetch_live_btree_index_key_catalog_slots`](../../lifeguard-migrate/src/schema_migration_compare.rs) + [`normalize_index_key_slot_for_compare`](../../lifeguard-migrate/src/schema_migration_compare.rs); [`IndexKeyNormalizedSlotsMismatchDrift`](../../lifeguard-migrate/src/schema_migration_compare.rs); **T1** suppressed on slot match when either side has expression keys. **T1** opclass-only dedupe when simple-key lists match modulo btree opclass tokens and `INCLUDE`/`WHERE` tails match. **Follow-on:** cast / `::type` canonicalization. **ORM v1 (shipped):** `IndexDefinition::key_list_sql` + derive `expr | cols` grammar (see §4.5 Option C).

---

## 5. Cross-track matrix

| | **T1** (string) | **T2b** (opclass) | **T3** (expression) |
|---|-----------------|-------------------|---------------------|
| **Primary signal** | Whole statement | Per-key opclass vs default or parsed migration | Per-key column vs expr |
| **Best data source** | `pg_indexes.indexdef` + migration file | `pg_index` + `pg_opclass` (+ types for defaults) | `pg_index` / `pg_get_expr` + migration parse |
| **Entity model gap** | N/A | No structured opclass field (verbatim `key_list_sql` may include `_ops`) | Optional `key_list_sql`; derive: key text then ` \| ` then coverage columns |
| **Risk** | Formatting noise | Type-default table maintenance | Expr normalization + security if extending derive |

---

## 6. Testing strategy (shared)

- **Unit:** pure parsers on frozen `indexdef` strings from PG 15/16/17 (document version).
- **Integration:** scratch schema per test (existing smoke pattern); create indexes with known opclasses and expressions; assert drift kinds.
- **Regression:** ensure shipped behaviors (**T2**, column drift, **T1**) unchanged when new code paths are off **feature flag** or **default-on** with narrow drift types.

---

## 7. Open questions (need product / DBA input)

1. **T2b default opclass:** Is comparing to **PostgreSQL default per type** required in v1, or is “**non-default opclass present**” warning enough?
2. **Hand-written migrations:** Should parsed migration opclasses override entity defaults for **expected** side?
3. **T3 derive:** Is **any** first-class expression index in `#[index]` on the roadmap, or is **compare-only** sufficient for 12–24 months?
4. **Suppression rules:** Maximum acceptable **duplicate** drifts (T1 + T2b + T3) in one report?
5. **PG version support:** Minimum server version for catalog queries (affects `pg_get_expr` / partition index behavior)?

---

## 8. Implementation order (suggested)

1. **T2b spike** — catalog SQL + prove opclass list matches `indexdef` for sample indexes.  
2. **T2b v1** — emit drift for **non-default** opclass on btree simple-column keys (optional: expected from type).  
3. **T3 spike** — classify keys as column vs expression from catalog. **Shipped (v1):** expression slots via `indkey` = `0` + `pg_get_indexdef`.  
4. **T3 v1** — structural drift when migration index is simple-column-only and live has expressions. **Shipped:** [`IndexExpressionKeyVsSimpleMigrationDrift`](../../lifeguard-migrate/src/schema_migration_compare.rs).  
5. **T3 v2** — normalized expression compare; consider **T1** dedupe. **Shipped:** per-slot normalization + mismatch drift + opclass-only **T1** suppression; explicit migration **COLLATE** / **ASC|DESC** / **NULLS** vs `pg_index`.  
6. **Optional:** extend `IndexDefinition` / derive **after** compare path is stable.

---

## 9. References

- PostgreSQL: [CREATE INDEX](https://www.postgresql.org/docs/current/sql-createindex.html), [pg_opclass](https://www.postgresql.org/docs/current/catalog-pg-opclass.html), [pg_index](https://www.postgresql.org/docs/current/catalog-pg-index.html), [pg_get_indexdef](https://www.postgresql.org/docs/current/functions-info.html).  
- Internal: [DESIGN_INDEX_COMPARE_ROADMAP.md](./DESIGN_INDEX_COMPARE_ROADMAP.md), [PRD §5.7a](./PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md#57a-deferred-phase-a-stretch--end-of-backlog), [`lifeguard-migrate/README.md`](../../lifeguard-migrate/README.md) (`compare-schema`).
