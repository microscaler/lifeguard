# Bug Tracker

## Fixed Bugs

### Single-Key Eager Loading Parameter Binding Issue
**Status:** ✅ Fixed  
**Date:** 2024-12-19  
**File:** `src/relation/eager.rs:194-199` (now fixed at lines 289-303)

**Description:**
The single-key eager loading path created SQL with placeholder parameters (`$1, $2, $3`) in the IN clause, but the actual `pk_values` were never bound to the query. The `Expr::cust(in_clause)` only inserted the raw SQL string without binding any values, resulting in runtime "missing parameter" database errors when the query was executed.

**Root Cause:**
The code used `Expr::cust()` with manually constructed placeholders, which bypasses sea_query's parameter binding system. This meant placeholders were in the SQL but no actual values were bound.

**Fix:**
Changed from:
```rust
let placeholders: Vec<String> = (0..pk_values.len()).map(|i| format!("${}", i + 1)).collect();
let in_clause = format!("{} IN ({})", fk_col_str, placeholders.join(", "));
query = query.filter(Expr::cust(in_clause));
```

To:
```rust
let fk_col = rel_def.from_col.iter().next().unwrap();
let fk_col_str = fk_col.to_string();
let fk_col_iden = sea_query::DynIden::from(fk_col_str);
query = query.filter(Expr::col(fk_col_iden).is_in(pk_values));
```

**Impact:**
- Single-key eager loading now properly binds parameters
- Prevents "missing parameter" runtime errors
- Uses sea_query's proper parameter binding API

**Tests Added:**
- `test_single_key_eager_loading_parameter_binding()` - Verifies parameter binding works correctly

**Related Files:**
- `src/relation/eager.rs` - Main fix location
- `src/relation/eager.rs` - Test added in same file
