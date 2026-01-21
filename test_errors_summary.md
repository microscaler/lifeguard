# Test Errors Summary

## test-nextest Errors

### 1. lifeguard-migrate Compilation Errors

#### Path Issues (Old Entity Locations)
```
error: couldn't read `lifeguard-migrate/src/../../examples/entities/accounting/general-ledger/chart_of_accounts.rs`: No such file or directory (os error 2)
error[E0583]: file not found for module `chart_of_accounts`
error[E0583]: file not found for module `account`
error[E0583]: file not found for module `journal_entry`
```

**Root Cause**: Entities were moved from `examples/entities/accounting/general-ledger/` to `examples/entities/src/accounting/general_ledger/`, but there are still references to the old paths.

**Files to Fix**:
- `examples/test_sql_generation.rs` - has old module paths
- Any other files referencing old entity paths

#### Missing Variables in test_sql_generation.rs
```
error[E0425]: cannot find value `generated_sql` in this scope
```

**Root Cause**: The `generated_sql` variable is not defined in the test file. The code has `return Ok(())` before the variable is used.

**Location**: `examples/test_sql_generation.rs` lines 52, 64, 83, 91, 109, 117

#### API Issues
```
error[E0576]: cannot find associated type `Entity` in trait `LifeModelTrait`
```

**Root Cause**: The test is trying to access `Entity` as an associated type of `LifeModelTrait`, but it's not defined that way.

**Location**: `examples/test_sql_generation.rs:42`

#### Syntax Errors
```
error: expected `;`, found `println`
```

**Root Cause**: Missing semicolon before `println!` statements after `return Ok(())`.

**Location**: `examples/test_sql_generation.rs` lines 80, 106

---

## test-derive-nextest Errors

### 1. Test Failure: test_default_column_definition

```
assertion `left == right` failed
  left: Some("INTEGER")
 right: None
```

**Root Cause**: The test expects `column_type` to be `None` for a default column definition, but it's returning `Some("INTEGER")`.

**Location**: `lifeguard-derive/tests/test_column_attributes.rs:486`

**Issue**: The type inference is now automatically inferring "INTEGER" for integer types, but the test expects no type to be inferred.

### 2. Proc-Macro Errors (Many instances)

```
error: expected item after attributes
error: proc-macro derive produced unparsable tokens
```

**Root Cause**: The `LifeModel` derive macro is producing invalid tokens. This is happening when trying to compile entities in `examples/entities/src/`.

**Possible Causes**:
- Entities are in a separate crate (`examples/entities`) that's not part of the workspace
- The entities can't be compiled because they're not included in the main workspace
- Module resolution issues

---

## Priority Fixes

1. **HIGH**: Fix `examples/test_sql_generation.rs` - update paths and fix syntax errors
2. **HIGH**: Fix `test_default_column_definition` test - update expectation or fix type inference
3. **MEDIUM**: Fix proc-macro errors in examples/entities - ensure entities can compile
4. **LOW**: Clean up unused imports and warnings
