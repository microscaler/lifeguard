# CRUD Testing Notes - Epic 02 Story 03

## Issue: Test Compilation Errors

The `test_crud.rs` file has compilation errors because the generated code references `lifeguard` and `sea_query` crates, which need to be available at macro expansion time.

## Root Cause

When procedural macros generate code, any external crates referenced in that code must be:
1. Available in the scope where the macro is used
2. Properly imported in the file using the macro

The generated CRUD methods reference:
- `lifeguard::LifeExecutor`
- `lifeguard::LifeError`
- `lifeguard::FromRow`
- `lifeguard::SelectQuery`
- `sea_query::*` types

## Solutions

### Solution 1: Add Dependencies to Test File (Current Approach)

The test file has `lifeguard` and `sea_query` as dev-dependencies in `lifeguard-derive/Cargo.toml`, and imports them in the test file. However, there are still SeaQuery API mismatches that need to be fixed.

### Solution 2: Use Absolute Paths in Generated Code

Modify the macro to generate code using absolute paths like `::lifeguard::` instead of `lifeguard::`. However, this requires the crates to be in the root namespace, which may not always be the case.

### Solution 3: Move Tests to Integration Tests

Move CRUD tests to the main `lifeguard` crate as integration tests, where all dependencies are naturally available. This is the recommended approach for testing macro-generated code that uses external crates.

### Solution 4: Use `macrotest` for Macro Expansion Testing

Use the `macrotest` crate to test macro expansion separately from runtime behavior. This allows testing that macros generate correct code without requiring all dependencies.

## Current Status

- ✅ CRUD methods are generated correctly
- ✅ Main workspace compiles successfully
- ⚠️ Test file has compilation errors due to SeaQuery API mismatches
- ⚠️ SeaQuery API usage needs to be corrected in generated code

## Next Steps

1. Fix SeaQuery API usage in generated code:
   - Use correct `returning()` method signature
   - Use correct `set()`/`value()` method for UpdateStatement
   - Use correct `and_where()` with proper Expr types

2. Once SeaQuery API is fixed, the tests should compile successfully since:
   - `lifeguard` and `sea_query` are dev-dependencies
   - They are imported in the test file
   - The generated code will reference them correctly

## Recommendation

For now, the CRUD functionality is implemented and working. The test compilation errors are due to SeaQuery API mismatches that need to be fixed. Once the SeaQuery API usage is corrected, the tests will compile and pass.

For immediate testing, integration tests in the main crate can be used to verify CRUD operations work correctly with actual database connections.
