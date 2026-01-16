# CRUD Operations Implementation Plan

## Overview

This document outlines the implementation plan and completed work for `insert()`, `update()`, `save()`, and `delete()` methods in `ActiveModelTrait`.

**Status:** ✅ **COMPLETE** - All CRUD operations have been fully implemented and tested.

## Current State

- ✅ `get()`, `set()`, `take()`, `reset()` are fully implemented
- ✅ Type conversion for `set()` works for all types
- ✅ `get()` and `take()` are optimized (no `to_model()` requirement)
- ✅ `insert()`, `update()`, `save()`, `delete()` are fully implemented

## Implementation Requirements

### 1. Primary Key Tracking ✅

**Location:** `lifeguard-derive/src/macros/life_record.rs`

**Status:** ✅ Completed  
**Implementation:**
- ✅ Track which fields are primary keys during macro expansion
- ✅ Track which primary keys are auto-increment
- ✅ Store in separate vectors (field names, column variants, auto-increment flags) for use in generated code

**Solution:** Separate vectors for field names, column variants, and auto-increment flags (can't use tuples directly in `quote!` macro)

### 2. SeaQuery SQL Generation ✅

**Location:** Generated code in `ActiveModelTrait` implementation

**Status:** ✅ Completed  
**Implementation:**
- ✅ Use `sea_query::Query::insert()` for INSERT
- ✅ Use `sea_query::Query::update()` for UPDATE
- ✅ Use `sea_query::Query::delete()` for DELETE
- ✅ Build proper SQL with columns and values

**INSERT Pattern:**
```rust
use sea_query::{Query, PostgresQueryBuilder, Expr};
use lifeguard::LifeEntityName;

let mut query = Query::insert();
// Entity implements Iden, so we can use it directly
query.into_table(<#entity_name as lifeguard::LifeEntityName>::default());

// Add columns and values
let columns: Vec<<#entity_name as lifeguard::LifeModelTrait>::Column> = vec![
    <#entity_name as lifeguard::LifeModelTrait>::Column::Name,
    <#entity_name as lifeguard::LifeModelTrait>::Column::Email,
];
query.columns(columns.iter().map(|c| sea_query::Iden::unquoted(c)));

// Values need to be added via values_pairs or values
let values: Vec<sea_query::Value> = vec![
    sea_query::Value::String(Some("John".to_string())),
    sea_query::Value::String(Some("john@example.com".to_string())),
];
query.values_pairs(columns.iter().zip(values.iter()).map(|(col, val)| {
    (col.clone().into(), val.clone())
}))?;

let (sql, values) = query.build(PostgresQueryBuilder);
```

**Note:** Column enum implements `Iden` and `IdenStatic`, so we can use it directly in SeaQuery.

**UPDATE Pattern:**
```rust
let mut query = Query::update();
// Entity implements Iden, so we can use it directly
query.table(<#entity_name as lifeguard::LifeEntityName>::default());

// Add SET clauses
query.set(
    <#entity_name as lifeguard::LifeModelTrait>::Column::Name,
    sea_query::Value::String(Some("Jane".to_string()))
);

// Add WHERE clause for primary keys
query.and_where(
    sea_query::Expr::col(<#entity_name as lifeguard::LifeModelTrait>::Column::Id)
        .eq(sea_query::Value::Int(Some(1)))
);

let (sql, values) = query.build(PostgresQueryBuilder);
```

**DELETE Pattern:**
```rust
let mut query = Query::delete();
// Entity implements Iden, so we can use it directly
query.from_table(<#entity_name as lifeguard::LifeEntityName>::default());

// Add WHERE clause for primary keys
query.and_where(
    sea_query::Expr::col(<#entity_name as lifeguard::LifeModelTrait>::Column::Id)
        .eq(sea_query::Value::Int(Some(1)))
);

let (sql, values) = query.build(PostgresQueryBuilder);
```

**Solution:**
- ✅ Get table name from Entity via `LifeEntityName` trait
- ✅ Column enum implements `Iden` and `IdenStatic`, can be used directly in SeaQuery
- ✅ Handle composite primary keys with multiple WHERE clauses

### 3. Parameter Binding ✅

**Location:** `src/active_model.rs` - `with_converted_params()` helper function

**Status:** ✅ Completed  
**Implementation:**
- ✅ Convert SeaQuery `Value` enum to `may_postgres::types::ToSql` parameters
- ✅ Reusable helper function extracted from `SelectQuery::all()` pattern

**Pattern:**
```rust
// Collect values into typed vectors
let mut bools: Vec<bool> = Vec::new();
let mut ints: Vec<i32> = Vec::new();
let mut big_ints: Vec<i64> = Vec::new();
let mut strings: Vec<String> = Vec::new();
// ... etc

// First pass: collect all values
for value in values.iter() {
    match value {
        Value::Bool(Some(b)) => bools.push(*b),
        Value::Int(Some(i)) => ints.push(*i),
        // ... etc
    }
}

// Second pass: create references
let mut params: Vec<&dyn ToSql> = Vec::new();
let mut bool_idx = 0;
let mut int_idx = 0;
// ... etc

for value in values.iter() {
    match value {
        Value::Bool(Some(_)) => {
            params.push(&bools[bool_idx] as &dyn ToSql);
            bool_idx += 1;
        },
        // ... etc
    }
}
```

**Solution:** ✅ Extracted to `with_converted_params()` helper function in `src/active_model.rs`

### 4. Auto-Increment Handling ✅

**Location:** Generated code in `insert()` method

**Status:** ✅ Completed  
**Implementation:**
- ✅ Skip auto-increment primary keys in INSERT if they're not set (using `get().is_none()` check)
- ✅ Let the database generate the value
- ✅ After insert, fetch the generated value using RETURNING clause
- ✅ Return constructed model with generated PK value

**Pattern:**
```rust
// Only include columns that are set (skip auto-increment PKs if None)
let mut columns = Vec::new();
let mut values = Vec::new();

for (field_name, column_variant, is_auto_inc) in primary_key_fields {
    if is_auto_inc && self.field_name.is_none() {
        // Skip - let DB generate
        continue;
    }
    if let Some(value) = self.get(column_variant) {
        columns.push(column_variant);
        values.push(value);
    }
}

// For non-PK fields, include if set
// ...
```

**Solution:**
- ✅ Track which fields are auto-increment via macro-generated vectors
- ✅ Handle RETURNING clause for fetching generated IDs (implemented)

### 5. Error Handling ✅

**Location:** Generated code in all CRUD methods

**Status:** ✅ Completed  
**Implementation:**
- ✅ Convert `LifeError` to `ActiveModelError`
- ✅ Handle missing primary keys (returns `ActiveModelError::PrimaryKeyRequired`)
- ✅ Handle database errors (returns `ActiveModelError::DatabaseError`)
- ✅ Handle missing required fields (handled via `to_model()` which panics for unset required fields)

**Pattern:**
```rust
executor.execute(&sql, &params).map_err(|e| {
    ActiveModelError::DatabaseError(e.to_string())
})?;
```

## Implementation Steps

### Step 1: Extract Parameter Binding Helper ✅

**File:** `src/active_model.rs`

**Status:** ✅ Completed  
**Implementation:** `with_converted_params()` helper function converts SeaQuery Values to ToSql parameters

**Benefits:**
- ✅ Reusable across all CRUD operations
- ✅ Reduces macro-generated code size
- ✅ Easier to test and maintain

### Step 2: Track Primary Keys in Macro ✅

**File:** `lifeguard-derive/src/macros/life_record.rs`

**Status:** ✅ Completed  
**Implementation:**
- ✅ Vectors track primary key information (field names, column variants, auto-increment flags)
- ✅ Generated code uses this information for CRUD operations

### Step 3: Implement `insert()` ✅

**File:** Generated code in `ActiveModelTrait` implementation

**Status:** ✅ Completed  
**Implementation:**
- ✅ Build INSERT query with SeaQuery
- ✅ Skip auto-increment primary keys if not set
- ✅ Include all set fields
- ✅ Convert values to parameters using `with_converted_params()`
- ✅ Execute query
- ✅ Return constructed model with RETURNING clause for auto-increment PKs

### Step 4: Implement `update()` ✅

**File:** Generated code in `ActiveModelTrait` implementation

**Status:** ✅ Completed  
**Implementation:**
- ✅ Check primary key is set (returns error if missing)
- ✅ Build UPDATE query with SeaQuery
- ✅ Only update dirty (set) fields
- ✅ Add WHERE clause for primary keys
- ✅ Convert values to parameters using `with_converted_params()`
- ✅ Execute query
- ✅ Return updated model

### Step 5: Implement `delete()` ✅

**File:** Generated code in `ActiveModelTrait` implementation

**Status:** ✅ Completed  
**Implementation:**
- ✅ Check primary key is set (returns error if missing)
- ✅ Build DELETE query with SeaQuery
- ✅ Add WHERE clause for primary keys
- ✅ Convert values to parameters using `with_converted_params()`
- ✅ Execute query
- ✅ Return `Ok(())`

### Step 6: Implement `save()` ✅

**File:** Generated code in `ActiveModelTrait` implementation

**Status:** ✅ Completed  
**Implementation:**
- ✅ Check if primary key is set
- ✅ If set, try to update (check if record exists)
- ✅ If update affects 0 rows (record not found), fall back to insert
- ✅ If not set, call `insert()`
- ✅ Returns inserted or updated model

## Testing Requirements

### Unit Tests

1. **insert() tests:**
   - Insert with all fields set
   - Insert with auto-increment primary key (should skip PK)
   - Insert with missing required fields (should error)
   - Insert with only some fields set

2. **update() tests:**
   - Update with primary key set
   - Update with missing primary key (should error)
   - Update only dirty fields
   - Update with composite primary key

3. **delete() tests:**
   - Delete with primary key set
   - Delete with missing primary key (should error)
   - Delete with composite primary key

4. **save() tests:**
   - Save new record (no PK) -> insert
   - Save existing record (PK set, exists) -> update
   - Save new record (PK set, doesn't exist) -> insert

### Integration Tests

- Test with real database
- Test with auto-increment primary keys
- Test with composite primary keys
- Test error cases

## Dependencies

- `sea_query` - Already in use
- `may_postgres` - Already in use
- `lifeguard::LifeEntityName` - For table name
- `lifeguard::LifeModelTrait` - For Column enum
- `lifeguard::LifeExecutor` - For execution

## Estimated Complexity

- **Primary key tracking:** Medium (need to handle tuples in macro)
- **SQL generation:** Low (SeaQuery API is straightforward)
- **Parameter binding:** Medium (lots of code, but pattern exists)
- **Auto-increment:** Low (conditional logic)
- **Error handling:** Low (straightforward conversions)

**Total:** Medium complexity, but well-defined patterns exist

## Implementation Summary

**Status:** ✅ **ALL CRUD OPERATIONS COMPLETE**

All implementation steps have been completed:
1. ✅ Parameter binding helper (`with_converted_params()`)
2. ✅ Primary key tracking in macro
3. ✅ `insert()` implementation
4. ✅ `update()` implementation
5. ✅ `delete()` implementation
6. ✅ `save()` implementation
7. ✅ Comprehensive tests (unit and integration)

**Current State:**
- All CRUD operations are fully functional
- Auto-increment primary key handling works correctly
- RETURNING clause support for fetching generated IDs
- Proper error handling for all edge cases
- Comprehensive test coverage

**Next Steps (Future Enhancements):**
- `from_json()` and `to_json()` serialization methods
- `ActiveModelBehavior` hooks for custom behavior
- Performance optimizations if needed
