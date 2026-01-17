# Lifeguard Bug Tracker

This file tracks bugs discovered during development, especially those found via Cursor's "Verify this issue exists and fix it" workflow.

## Bug Tracking System

Each bug is tracked in its own markdown file in `.agent/bugs/` with the naming convention `BUG-YYYY-MM-DD-NN.md`. This file serves as an index with metadata and links to individual bug reports.

## Bug Entry Format

Each bug entry in this index includes:
- **ID**: Unique identifier (BUG-YYYY-MM-DD-NN) - links to detailed bug report
- **Date**: Discovery date
- **Source**: How the bug was discovered (e.g., "Cursor verification", "Test failure", "User report")
- **Status**: `open`, `fixed`, `verified`
- **Severity**: `critical`, `high`, `medium`, `low`
- **Location**: File and line numbers
- **Impact**: Brief description of what functionality is affected
- **Link**: Hyperlink to detailed bug report

---

## Bugs

### [BUG-2025-01-27-01](bugs/BUG-2025-01-27-01.md)

**Date**: 2025-01-27  
**Source**: Cursor verification  
**Status**: `fixed`  
**Severity**: `critical`  
**Location**: `lifeguard-derive/src/macros/life_record.rs:265` (was 264)  
**Impact**: Compilation error for entities with `#[auto_increment]` primary keys in the `insert()` method

Use of moved variable `record_for_hooks` in `returning_extractors` code. The variable was moved to `updated_record` before the generated code tried to use it.

---

### [BUG-2025-01-27-02](bugs/BUG-2025-01-27-02.md)

**Date**: 2025-01-27  
**Source**: Cursor verification  
**Status**: `fixed`  
**Severity**: `high`  
**Location**: `src/relation/def.rs:286-292`  
**Impact**: `build_where_condition` uses `to_tbl` instead of `from_tbl` for foreign key column, causing SQL errors when querying related entities

The `build_where_condition` function incorrectly uses `rel_def.to_tbl` when building WHERE clauses, but the foreign key column (`from_col`) exists in `rel_def.from_tbl`. For BelongsTo relationships, this generates incorrect SQL like `users.user_id = <pk>` instead of `posts.user_id = <pk>`, causing runtime SQL errors.

---

### [BUG-2025-01-27-03](bugs/BUG-2025-01-27-03.md)

**Date**: 2025-01-27  
**Source**: Cursor verification  
**Status**: `fixed`  
**Severity**: `critical`  
**Location**: `src/relation/def.rs:216-217`, `src/relation/def.rs:288`  
**Impact**: Both `join_tbl_on_condition` and `build_where_condition` use `format!("{:?}", table_ref)` which produces invalid SQL with debug representation instead of actual table names

Both `join_tbl_on_condition` and `build_where_condition` use `format!("{:?}", table_ref)` to convert `TableRef` to a string for SQL generation. The `{:?}` format specifier invokes Rust's `Debug` trait, which produces output like `Table(TableName(None, DynIden(...)), None)` rather than the actual table name (e.g., `"posts"`). This generates syntactically invalid SQL that cannot be executed against any database.

---

### [BUG-2025-01-27-04](bugs/BUG-2025-01-27-04.md)

**Date**: 2025-01-27  
**Source**: User verification request  
**Status**: `verified`  
**Severity**: `high`  
**Location**: `lifeguard-derive/src/macros/relation.rs:358-396`  
**Impact**: Default column inference in `DeriveRelation` macro generates incorrect column values when `from`/`to` attributes are not specified, causing incorrect JOIN and WHERE clauses

The `DeriveRelation` macro generates incorrect column values when `from`/`to` attributes are not specified. For `belongs_to` relationships, `from_col` defaults to `Column::Id` (the primary key) but should be the foreign key column. For `has_many`/`has_one` relationships, `to_col` defaults to `"id"` but should be the foreign key in the related table. This produces incorrect SQL like `posts.id = users.id` instead of `posts.user_id = users.id`.

**Verification**: Added comprehensive test `test_derive_relation_belongs_to_default_columns()` that verifies `belongs_to` relationships without `from`/`to` attributes correctly infer foreign key columns. All tests pass.

---

### [BUG-2025-01-27-05](bugs/BUG-2025-01-27-05.md)

**Date**: 2025-01-27  
**Source**: User verification request  
**Status**: `fixed`  
**Severity**: `high`  
**Location**: `lifeguard-derive/src/macros/life_model.rs:863-932`  
**Impact**: Inconsistent primary key identity and values for entities without primary keys causes `build_where_condition` to panic at runtime

When an entity has no primary key defined, `get_primary_key_identity()` returns `Identity::Unary("")` (arity 1) while `get_primary_key_values()` returns `vec![]` (length 0). This inconsistency causes `build_where_condition` to panic at runtime with "Number of primary key values must match primary key arity" since the assertion `pk_values.len() == pk_identity.arity()` fails (0 != 1).

**Fix**: Changed `get_primary_key_identity()` to return `Identity::Many(vec![])` (arity 0) instead of `Identity::Unary("")` (arity 1), ensuring consistency with `get_primary_key_values()` which returns `vec![]` (length 0).

---

## Bug Statistics

- **Total Bugs**: 5
- **Open**: 0
- **Fixed**: 4
- **Verified**: 1

## Status Legend

- **open**: Bug has been identified but not yet fixed
- **fixed**: Bug has been fixed but not yet verified with tests
- **verified**: Bug has been fixed and verified with passing tests

## Severity Levels

- **critical**: Prevents compilation or causes data loss/corruption
- **high**: Breaks core functionality or causes crashes
- **medium**: Breaks non-critical functionality or causes incorrect behavior
- **low**: Minor issues, edge cases, or cosmetic problems
