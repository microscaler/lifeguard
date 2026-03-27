# CRUD Edge Cases - Epic 02 Story 03

## Overview

This document lists edge cases for CRUD operations that should be tested in integration tests with actual database connections. Some edge cases cannot be fully tested at compile-time due to macro-generated code limitations.

## Edge Cases to Test

### 1. Insert Operations

#### Insert with No Fields
- **Scenario**: Struct with only primary key, no other fields
- **Expected**: Runtime error "No fields to insert"
- **Status**: ✅ Handled in generated code (`columns.is_empty()` check)
- **Test Location**: Integration tests

#### Insert with Only Primary Key Set
- **Scenario**: Record with only primary key field set, no other fields
- **Expected**: Runtime error "No fields to insert" (primary key is skipped)
- **Status**: ✅ Handled in generated code
- **Test Location**: Integration tests

#### Insert with Nullable Fields Set to None
- **Scenario**: Insert record with `Option<T>` fields set to `None`
- **Expected**: Should insert `NULL` values for nullable fields
- **Status**: ✅ Supported
- **Test Location**: Integration tests

### 2. Update Operations

#### Update with No Dirty Fields
- **Scenario**: Update record with all fields `None` (no changes)
- **Expected**: Should generate SQL with no SET clauses, or skip update
- **Status**: ⚠️ Current implementation may generate empty UPDATE
- **Test Location**: Integration tests

#### Update with Nullable Fields Set to None
- **Scenario**: Update record setting nullable field to `None` (clearing value)
- **Expected**: Should set column to `NULL`
- **Status**: ✅ Supported
- **Test Location**: Integration tests

### 3. Composite Primary Keys

#### CRUD with Composite Primary Keys
- **Scenario**: Struct with multiple `#[primary_key]` fields
- **Expected**: 
  - `find_by_id()` currently uses first primary key only
  - `delete()` currently uses first primary key only
  - Full composite PK support needs enhancement
- **Status**: ⚠️ Partial support (uses first PK only)
- **Test Location**: Integration tests
- **Future Enhancement**: Support tuple or struct for composite PKs

### 4. No Primary Key

#### CRUD without Primary Key
- **Scenario**: Struct with no `#[primary_key]` fields
- **Expected**:
  - `find_by_id()` should not be generated
  - `delete()` should not be generated
  - `insert()` should still work
  - `update()` should not be generated (requires PK)
- **Status**: ✅ Handled correctly
- **Test Location**: Compile-time tests

### 5. All PostgreSQL Types

#### CRUD with All Field Types
- **Scenario**: Struct with all supported PostgreSQL types
  - `i16`, `i32`, `i64` (integers)
  - `String` (text)
  - `bool` (boolean)
  - `f32`, `f64` (floating point)
  - `Option<T>` (nullable)
- **Expected**: All types should work correctly in CRUD operations
- **Status**: ✅ Supported
- **Test Location**: Integration tests

### 6. Custom Column Names

#### CRUD with Custom Column Names
- **Scenario**: Struct with `#[column_name = "..."]` attributes
- **Expected**: CRUD operations should use custom column names in SQL
- **Status**: ✅ Supported
- **Test Location**: Integration tests

### 7. Dirty Field Tracking

#### Insert with Partial Fields
- **Scenario**: Insert record with only some fields set
- **Expected**: Should only insert set fields (skip `None` fields)
- **Status**: ✅ Supported
- **Test Location**: Integration tests

#### Update with Partial Fields
- **Scenario**: Update record with only some fields changed
- **Expected**: Should only update dirty fields
- **Status**: ✅ Supported
- **Test Location**: Integration tests

### 8. Error Cases

#### Find by Non-Existent ID
- **Scenario**: `find_by_id()` with ID that doesn't exist
- **Expected**: Should return appropriate error (not found)
- **Status**: ⚠️ Needs error handling enhancement
- **Test Location**: Integration tests

#### Delete Non-Existent Record
- **Scenario**: `delete()` with ID that doesn't exist
- **Expected**: Should return 0 rows affected
- **Status**: ✅ Supported (returns `u64`)
- **Test Location**: Integration tests

#### Update Non-Existent Record
- **Scenario**: `update()` with ID that doesn't exist
- **Expected**: Should return appropriate error or 0 rows
- **Status**: ⚠️ Needs error handling enhancement
- **Test Location**: Integration tests

## Known Limitations

### Macro-Generated Code Limitations

1. **Type Inference Issues**: Structs with only primary key cause type inference issues in macro-generated code when `columns` and `values` vectors are empty.

2. **ColumnName Conflicts**: Multiple structs with nullable fields in the same test file can cause `ColumnName` type conflicts due to macro-generated struct names.

3. **Composite Primary Keys**: Current implementation only supports single primary keys for `find_by_id()` and `delete()`. Full composite PK support requires enhancement.

## Testing Strategy

### Compile-Time Tests
- Verify methods exist and have correct signatures
- Verify code compiles for various struct configurations
- Test basic type checking

### Integration Tests (Recommended)
- Test actual database operations
- Test error handling
- Test edge cases with real data
- Test performance and correctness

## Next Steps

1. Create integration test suite with actual PostgreSQL database
2. Test all edge cases listed above
3. Enhance error handling for not-found cases
4. Add support for composite primary keys in CRUD operations
5. Add validation for required fields in insert/update operations
