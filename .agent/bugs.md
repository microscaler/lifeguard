# Bug Tracker Index

This file serves as an index to all bugs found and fixed in the Lifeguard codebase. Each bug has its own detailed file in the `bugs/` directory.

**Purpose:** Complete bug documentation to prevent recurring issues.

---

## Fixed Bugs

### 2025-01-18

- [BUG-2025-01-18-01: load_related HasManyThrough Grouping Not Implemented](bugs/BUG-2025-01-18-01.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Silent failure, returns empty results

### 2025-01-27

- [BUG-2025-01-27-01: Use of moved variable `record_for_hooks` in `returning_extractors`](bugs/BUG-2025-01-27-01.md)  
  **Status:** ✅ FIXED | **Priority:** Critical | **Severity:** Compilation error

- [BUG-2025-01-27-02: build_where_condition uses wrong table reference](bugs/BUG-2025-01-27-02.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** SQL errors

- [BUG-2025-01-27-03: Invalid SQL Generated from TableRef Debug Formatting](bugs/BUG-2025-01-27-03.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** SQL errors

- [BUG-2025-01-27-04: Incorrect Default Column Inference in DeriveRelation Macro](bugs/BUG-2025-01-27-04.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Incorrect code generation

- [BUG-2025-01-27-05: Inconsistent Primary Key Identity and Values for Entities Without Primary Keys](bugs/BUG-2025-01-27-05.md)  
  **Status:** ✅ FIXED | **Priority:** Medium | **Severity:** Inconsistent behavior

- [BUG-2025-01-27-06: Incorrect Foreign Key Column Name Inference for Module-Qualified Entity Paths](bugs/BUG-2025-01-27-06.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Incorrect code generation

- [BUG-2025-01-27-07: Silent Ignoring of parse_nested_meta Errors in DeriveRelation Macro](bugs/BUG-2025-01-27-07.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Silent failure

- [BUG-2025-01-27-08: find_related Uses Wrong Relationship Direction](bugs/BUG-2025-01-27-08.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Incorrect query results

- [BUG-2025-01-27-09: DerivePartialModel Macro Panic on Invalid Entity Path Segments](bugs/BUG-2025-01-27-09.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Macro expansion panic

- [BUG-2025-01-27-10: DerivePartialModel Macro Incorrect column_name Attribute Parsing](bugs/BUG-2025-01-27-10.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Silent failure

- [BUG-2025-01-27-11: DeriveRelation Macro Missing Validation for Invalid Identifiers](bugs/BUG-2025-01-27-11.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Macro expansion panic

- [BUG-2025-01-27-12: Test Helpers test_get_connection_string_env_var Failing](bugs/BUG-2025-01-27-12.md)  
  **Status:** ✅ FIXED | **Priority:** Medium | **Severity:** Test failure

- [BUG-2025-01-27-13: DerivePartialModel Macro Missing Validation for Invalid Identifiers in Entity Path](bugs/BUG-2025-01-27-13.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Macro expansion panic

- [BUG-2025-01-27-14: DeriveRelation Macro Silently Discards Different Column Configurations for Same Entity](bugs/BUG-2025-01-27-14.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Silent failure leading to incorrect queries

### 2024-12-19

- [BUG-2024-12-19-01: Codegen Tool Incorrect Unsigned Integer Type Detection](bugs/BUG-2024-12-19-01.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Invalid code generation

- [BUG-2024-12-19-02: Missing `#[test]` Attribute on Option Composite Primary Key Test](bugs/BUG-2024-12-19-02.md)  
  **Status:** ✅ FIXED | **Priority:** Medium | **Severity:** Test coverage gap

- [BUG-2024-12-19-03: DerivePartialModel Macro Inconsistent FromRow Implementation](bugs/BUG-2024-12-19-03.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Inconsistency with codegen

- [BUG-2024-12-19-04: LifeModel Macro Conflicting IntoColumnRef Implementation](bugs/BUG-2024-12-19-04.md)  
  **Status:** ✅ FIXED | **Priority:** Critical | **Severity:** All tests failing

- [BUG-2024-12-19-05: LifeModel Macro Missing with-json Feature Flag](bugs/BUG-2024-12-19-05.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Compilation errors for JSON

- [BUG-2024-12-19-06: LifeModel Macro Incorrect String/Bytes Parameter Handling](bugs/BUG-2024-12-19-06.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Compilation errors

- [BUG-2024-12-19-07: LifeModel Macro Unsigned Integer FromSql Issues](bugs/BUG-2024-12-19-07.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Compilation errors

- [BUG-2024-12-19-08: DerivePartialModel Macro Expansion Errors (E0284)](bugs/BUG-2024-12-19-08.md)  
  **Status:** ✅ RESOLVED | **Priority:** High | **Severity:** Macro expansion failures

### 2025-01-27 (continued)

- [BUG-2025-01-27-15: is_dummy_path Heuristic Incorrectly Flags Valid Self-Referential Relationships](bugs/BUG-2025-01-27-15.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Missing RelatedEntity enum variants

### 2026-01-18

- [BUG-2026-01-18-01: FindRelated Trait Requires Impossible LifeModelTrait Bound on Models](bugs/BUG-2026-01-18-01.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** API unusable

- [BUG-2026-01-18-02: DeriveTryIntoModel Macro Incorrect Error Handling for Custom Error Types with Convert Attribute](bugs/BUG-2026-01-18-02.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Compilation error / API inconsistency

- [BUG-2026-01-19-01: DeriveTryIntoModel Macro extract_field_attribute Only Checks First Attribute and Custom Error Type Handling Issues](bugs/BUG-2026-01-19-01.md)  
  **Status:** ✅ FIXED | **Priority:** High | **Severity:** Silent failure / API inconsistency

---

## Open Bugs

*No open bugs at this time.*

---

## Bug Report Template

When creating a new bug file, use this template:

```markdown
# BUG-YYYY-MM-DD-NN: [Bug Title]

**Date:** YYYY-MM-DD  
**Status:** 🔴 OPEN / 🟡 IN PROGRESS / ✅ FIXED  
**Priority:** Low / Medium / High / Critical  
**Severity:** Bug / Regression / Performance / Security

## Summary

[Brief description of the bug]

## Discovery

**Date:** YYYY-MM-DD  
**Source:** [How it was discovered]  
**Severity:** `low` / `medium` / `high` / `critical`  
**Status:** `open` / `in_progress` / `fixed`

## Location

- **File:** `path/to/file.rs`
- **Lines:** XXX-YYY

## Description

[Detailed description of the bug]

## Root Cause

[What caused the bug]

## Fix

[How it was fixed]

## Testing

[Tests added/updated]

## Impact

[What was affected]

## Related Files

- `file1.rs` - Description
- `file2.rs` - Description

## Verification

- [ ] Bug identified and root cause analyzed
- [ ] Fix implemented
- [ ] Test added to verify fix
- [ ] Tests run and passing
- [ ] Integration tests verify fix works in real scenarios
```
