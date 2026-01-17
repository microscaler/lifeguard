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

## Bug Statistics

- **Total Bugs**: 1
- **Open**: 0
- **Fixed**: 1
- **Verified**: 0 (pending runtime tests)

## Status Legend

- **open**: Bug has been identified but not yet fixed
- **fixed**: Bug has been fixed but not yet verified with tests
- **verified**: Bug has been fixed and verified with passing tests

## Severity Levels

- **critical**: Prevents compilation or causes data loss/corruption
- **high**: Breaks core functionality or causes crashes
- **medium**: Breaks non-critical functionality or causes incorrect behavior
- **low**: Minor issues, edge cases, or cosmetic problems
