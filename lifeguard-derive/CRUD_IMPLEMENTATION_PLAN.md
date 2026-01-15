# CRUD Operations Implementation Plan

## Overview

This document outlines the implementation plan for `insert()`, `update()`, `save()`, and `delete()` methods in `ActiveModelTrait`.

## Current State

- ✅ `get()`, `set()`, `take()`, `reset()` are fully implemented
- ✅ Type conversion for `set()` works for all types
- ✅ `get()` and `take()` are optimized (no `to_model()` requirement)
- ❌ `insert()`, `update()`, `save()`, `delete()` are placeholders

## Implementation Requirements

### 1. Primary Key Tracking

**Location:** `lifeguard-derive/src/macros/life_record.rs`

**What's needed:**
- Track which fields are primary keys during macro expansion
- Track which primary keys are auto-increment
- Store this information in a way that can be used in generated code

**Implementation:**
```rust
// In the field processing loop:
let is_primary_key = attributes::has_attribute(field, "primary_key");
let is_auto_increment = attributes::has_attribute(field, "primary_key") && 
                        attributes::has_attribute(field, "auto_increment");

// Store in vectors:
let mut primary_key_fields: Vec<(Ident, Ident, bool)> = Vec::new(); // (field_name, column_variant, is_auto_increment)
```

**Challenges:**
- Need to pass this information to the generated code
- Can't use tuples directly in `quote!` macro
- Solution: Generate separate vectors for field names, column variants, and auto-increment flags

### 2. SeaQuery SQL Generation

**Location:** Generated code in `ActiveModelTrait` implementation

**What's needed:**
- Use `sea_query::Query::insert()` for INSERT
- Use `sea_query::Query::update()` for UPDATE
- Use `sea_query::Query::delete()` for DELETE
- Build proper SQL with columns and values

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

**Challenges:**
- Need to get table name from Entity
- Need to convert Column enum to SeaQuery Iden
- Need to handle composite primary keys

### 3. Parameter Binding

**Location:** Generated code in `ActiveModelTrait` implementation

**What's needed:**
- Convert SeaQuery `Value` enum to `may_postgres::types::ToSql` parameters
- Reuse the pattern from `SelectQuery::all()` (lines 400-526 in `src/query.rs`)

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

**Challenges:**
- This is a lot of code to generate in the macro
- Solution: Extract to a helper function or macro that can be called from generated code

### 4. Auto-Increment Handling

**Location:** Generated code in `insert()` method

**What's needed:**
- Skip auto-increment primary keys in INSERT if they're not set
- Let the database generate the value
- After insert, fetch the generated value (optional, can return constructed model)

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

**Challenges:**
- Need to track which fields are auto-increment
- Need to handle RETURNING clause for fetching generated IDs (optional)

### 5. Error Handling

**Location:** Generated code in all CRUD methods

**What's needed:**
- Convert `LifeError` to `ActiveModelError`
- Handle missing primary keys
- Handle missing required fields
- Handle database errors

**Pattern:**
```rust
executor.execute(&sql, &params).map_err(|e| {
    ActiveModelError::DatabaseError(e.to_string())
})?;
```

## Implementation Steps

### Step 1: Extract Parameter Binding Helper

**File:** `src/active_model.rs` or new `src/query_helpers.rs`

Create a helper function that converts SeaQuery Values to ToSql parameters:
```rust
pub fn convert_values_to_params(values: &[Value]) -> Result<Vec<&dyn ToSql>, ActiveModelError> {
    // Implementation from SelectQuery::all() pattern
}
```

**Benefits:**
- Reusable across all CRUD operations
- Reduces macro-generated code size
- Easier to test and maintain

### Step 2: Track Primary Keys in Macro

**File:** `lifeguard-derive/src/macros/life_record.rs`

- Add vectors to track primary key information
- Generate code that uses this information

### Step 3: Implement `insert()`

**File:** Generated code in `ActiveModelTrait` implementation

- Build INSERT query with SeaQuery
- Skip auto-increment primary keys if not set
- Include all set fields
- Convert values to parameters
- Execute query
- Return constructed model (or fetch from DB if needed)

### Step 4: Implement `update()`

**File:** Generated code in `ActiveModelTrait` implementation

- Check primary key is set
- Build UPDATE query with SeaQuery
- Only update dirty (set) fields
- Add WHERE clause for primary keys
- Convert values to parameters
- Execute query
- Return updated model

### Step 5: Implement `delete()`

**File:** Generated code in `ActiveModelTrait` implementation

- Check primary key is set
- Build DELETE query with SeaQuery
- Add WHERE clause for primary keys
- Convert values to parameters
- Execute query
- Return `Ok(())`

### Step 6: Implement `save()`

**File:** Generated code in `ActiveModelTrait` implementation

- Check if primary key is set
- If set, try to find record (use `Entity::find().filter(...).one()`)
- If found, call `update()`
- If not found or not set, call `insert()`

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

## Next Steps

1. Create helper function for parameter binding
2. Update macro to track primary keys
3. Implement `insert()` first (simplest)
4. Implement `update()` and `delete()`
5. Implement `save()` last (depends on others)
6. Add comprehensive tests
