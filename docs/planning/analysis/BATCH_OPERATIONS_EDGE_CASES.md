# Batch Operations Edge Cases Audit

## Overview

This document outlines all edge cases that need to be tested for batch operations (Epic 02 Story 06). These tests ensure robustness, correctness, and performance of `insert_many()`, `update_many()`, and `delete_many()` operations.

## insert_many() Edge Cases

### Empty Input
- [ ] Empty slice (already handled, returns `Ok(Vec::new())`)
- [ ] Verify it doesn't hit the database

### Single Record
- [ ] One record (ensures it works, not just large batches)

### Large Batches (Story requirement: 1000+ records)
- [ ] 1000+ records (test chunking if implemented)
- [ ] Very large batches (10,000+ records)
- [ ] Memory usage with large batches

### Field Variations
- [ ] Records with different field sets (some fields `None` vs `Some`)
- [ ] All fields `None` (should error: "No fields to insert")
- [ ] Mixed NULL and non-NULL values
- [ ] Primary key handling (auto-increment vs manual)

### Data Type Edge Cases
- [ ] Maximum value types (i64::MAX, u64::MAX)
- [ ] Minimum value types (i64::MIN)
- [ ] Very long strings (TEXT fields)
- [ ] Binary data (BYTEA)
- [ ] JSON/JSONB values
- [ ] Arrays
- [ ] Custom types

### Constraint Violations
- [ ] Duplicate primary key
- [ ] Unique constraint violation
- [ ] Foreign key constraint violation
- [ ] NOT NULL constraint violation
- [ ] Check constraint violation

### Transaction Scenarios
- [ ] Batch insert in transaction, then rollback
- [ ] Partial failure (some records fail)
- [ ] All-or-nothing behavior

### Return Value Verification
- [ ] Verify all inserted models are returned
- [ ] Verify primary keys are populated (auto-increment)
- [ ] Verify all fields match what was inserted

## update_many() Edge Cases

### Filter Edge Cases
- [ ] Filter matches 0 rows (should return `0`)
- [ ] Filter matches all rows
- [ ] Filter matches 1 row
- [ ] Complex filter expressions (AND, OR, nested)
- [ ] Filter with no parameters
- [ ] Filter with many parameters (IN with 1000+ values)

### Empty Update Values
- [ ] Record with all fields `None` (should error: "No fields to update")
- [ ] Record with only primary key set (should error)
- [ ] Record with no dirty fields

### Primary Key Handling
- [ ] Attempting to update primary key (should be skipped)
- [ ] Verify primary key is never in SET clause

### NULL Values
- [ ] Setting fields to NULL
- [ ] Updating NULL to non-NULL
- [ ] Updating non-NULL to NULL

### Type Conversions
- [ ] Same value types
- [ ] Compatible type conversions
- [ ] Invalid type conversions (should error)

### Constraint Violations
- [ ] Unique constraint violation
- [ ] Foreign key constraint violation
- [ ] Check constraint violation

### Transaction Scenarios
- [ ] Update in transaction, then rollback
- [ ] Verify row count is correct

### Return Value Verification
- [ ] Verify affected row count matches actual updates
- [ ] Verify only matching rows are updated

## delete_many() Edge Cases

### Filter Edge Cases
- [ ] Filter matches 0 rows (should return `0`)
- [ ] Filter matches all rows
- [ ] Filter matches 1 row
- [ ] Complex filter expressions
- [ ] Filter with no parameters
- [ ] Filter with many parameters

### Cascade Deletes
- [ ] Foreign key relationships
- [ ] ON DELETE CASCADE behavior
- [ ] ON DELETE RESTRICT behavior (should error)

### Transaction Scenarios
- [ ] Delete in transaction, then rollback
- [ ] Verify row count is correct

### Return Value Verification
- [ ] Verify affected row count matches actual deletes
- [ ] Verify only matching rows are deleted

## General Edge Cases

### Parameter Conversion
- [ ] All SeaQuery Value types
- [ ] Edge values (i64::MAX, u64::MAX, etc.)
- [ ] NULL handling for all types
- [ ] Unsupported value types (should error gracefully)

### Error Handling
- [ ] Database connection errors
- [ ] SQL syntax errors (shouldn't happen with SeaQuery, but test)
- [ ] Timeout errors
- [ ] Transaction errors

### Concurrency
- [ ] Concurrent batch operations
- [ ] Race conditions
- [ ] Lock contention

### Performance
- [ ] Large batch performance (1000+ records)
- [ ] Chunking for very large batches (if implemented)
- [ ] Memory usage
- [ ] Query execution time

### SQL Generation
- [ ] Verify SQL is correct (single query, not N queries)
- [ ] Verify parameter binding is correct
- [ ] Verify RETURNING clause for insert_many

### Type Safety
- [ ] Compile-time type checking
- [ ] Runtime type validation

## Test Implementation Status

### Compile-Time Tests (lifeguard-derive/tests/test_crud.rs)
- [x] test_insert_many_method_exists
- [x] test_update_many_method_exists
- [x] test_delete_many_method_exists
- [x] test_batch_operations_with_query_builder
- [x] test_insert_many_empty_slice
- [x] test_insert_many_single_record
- [x] test_insert_many_mixed_null_values
- [x] test_update_many_no_matches
- [x] test_update_many_empty_values
- [x] test_update_many_primary_key_skipped
- [x] test_update_many_null_values
- [x] test_update_many_complex_filter
- [x] test_delete_many_no_matches
- [x] test_delete_many_complex_filter
- [x] test_delete_many_in_clause
- [x] test_batch_operations_type_safety
- [x] test_batch_operations_all_data_types

### Integration Tests (To be implemented)
- [ ] All edge cases listed above (require actual database connection)

## Notes

- Compile-time tests verify method signatures exist and are callable
- Integration tests require actual database connection (testkit infrastructure)
- Performance tests should be benchmarked, not just pass/fail
- Transaction tests require transaction support in test infrastructure
