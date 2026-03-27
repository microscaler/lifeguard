# Unwrap/Expect Usage Audit Report

This report audits all `unwrap()` and `expect()` calls in the Lifeguard codebase to assess panic risks in production database code.

**Date**: 2026-01-27  
**Total Usage Points**: ~230 instances

## Summary by Category

| Category | Count | Risk Level | Action Required |
|----------|-------|------------|-----------------|
| **Production Code (src/)** | 45 | 🔴 HIGH | Must fix - database tools cannot panic |
| **Test Code** | 120+ | 🟢 LOW | Acceptable - tests can panic |
| **Macro/Codegen** | 50+ | 🟡 MEDIUM | Review - may affect generated code |
| **Build Scripts** | 15+ | 🟡 MEDIUM | Review - affects build-time only |

---

## Production Code (`src/`) - CRITICAL FIXES REQUIRED

### 1. Mutex Lock Failures (Poisoned Mutex)

| File | Line | Context | What It Does | Implications | Risk |
|------|------|---------|--------------|--------------|------|
| `src/query/execution.rs` | 563 | `MockExecutor::get_captured_sql()` | Locks Mutex to read captured SQL | If mutex is poisoned (thread panicked while holding lock), entire test infrastructure fails | 🔴 HIGH |
| `src/query/execution.rs` | 567 | `MockExecutor::get_captured_param_counts()` | Locks Mutex to read param counts | Same as above - poisoned mutex causes panic | 🔴 HIGH |
| `src/query/execution.rs` | 571-572 | `MockExecutor::clear()` | Locks Mutex to clear captured data | Same as above | 🔴 HIGH |
| `src/query/execution.rs` | 584-585, 590-591, 598-599 | `MockExecutor::query_*()` methods | Locks Mutex to record SQL queries | Same as above - 6 occurrences | 🔴 HIGH |
| `src/query/column/definition.rs` | 44 | `get_static_expr()` | Locks static EXPR_CACHE Mutex | If cache mutex is poisoned, all expression caching fails | 🔴 HIGH |
| `src/query/column/definition.rs` | 556, 561 | Test code | Locks cache for assertions | Test-only, but still should handle errors | 🟡 MEDIUM |

**Impact**: If any thread panics while holding these mutexes, all subsequent operations will panic. This is a critical failure mode for database operations.

**Recommendation**: Replace with proper error handling:
```rust
self.captured_sql.lock()
    .map_err(|e| LifeError::Other(format!("Mutex lock failed: {e}")))?
    .clone()
```

---

### 2. HasManyThrough Relationship Validation

| File | Line | Context | What It Does | Implications | Risk |
|------|------|---------|--------------|--------------|------|
| `src/relation/eager.rs` | 304-311 | `load_related()` for HasManyThrough | Validates required fields exist | If relationship definition is incomplete, entire eager load fails with panic | 🔴 HIGH |
| `src/relation/eager.rs` | 504-506 | `load_related()` for HasManyThrough | Same validation | Same issue - duplicate code path | 🔴 HIGH |
| `src/relation/def/struct_def.rs` | 195-203 | `join_on_exprs()` | Validates HasManyThrough fields | If called on incomplete relationship, panics | 🔴 HIGH |

**Impact**: If a `HasManyThrough` relationship is defined without required fields (`through_tbl`, `through_from_col`, `through_to_col`), the entire query execution panics instead of returning an error.

**Recommendation**: Return `Result` with descriptive error:
```rust
let through_tbl = rel_def.through_tbl.as_ref()
    .ok_or_else(|| LifeError::Other("HasManyThrough relationship must have through_tbl set".to_string()))?;
```

---

### 3. Iterator Assumptions

| File | Line | Context | What It Does | Implications | Risk |
|------|------|---------|--------------|--------------|------|
| `src/relation/eager.rs` | 330-331, 333 | `load_related()` | Assumes iterators have at least one element | If relationship definition has empty column arrays, panics | 🔴 HIGH |
| `src/relation/eager.rs` | 438 | `load_related()` for belongs_to | Assumes `to_col` has elements | If FK column definition is empty, panics | 🔴 HIGH |
| `src/relation/eager.rs` | 2282 | `load_related()` for belongs_to | Same assumption | Same issue | 🔴 HIGH |
| `src/relation/eager.rs` | 741 | HashMap lookup | Assumes key exists after query | If query returns unexpected data, panics | 🔴 HIGH |
| `src/relation/def/condition.rs` | 237, 240 | `build_where_condition()` | Assumes expressions exist | If called with empty vec (shouldn't happen), panics | 🟡 MEDIUM |

**Impact**: These assume data structures are correctly populated. If relationship definitions are malformed or queries return unexpected results, the system panics instead of handling gracefully.

**Recommendation**: Add validation and return errors:
```rust
let fk_col = rel_def.to_col.iter().next()
    .ok_or_else(|| LifeError::Other("Foreign key column definition is empty".to_string()))?;
```

---

### 4. Regex Capture Groups

| File | Line | Context | What It Does | Implications | Risk |
|------|------|---------|--------------|--------------|------|
| `src/migration/file.rs` | 47-48 | `parse_filename()` | Extracts version and name from filename | If regex matches but capture groups are missing (shouldn't happen), panics | 🟡 MEDIUM |

**Impact**: The regex is validated to match, but if capture groups are somehow missing, this panics. However, the regex pattern guarantees groups exist if match succeeds.

**Recommendation**: Use `ok_or_else` for safety, but this is lower risk:
```rust
let version_str = caps.get(1)
    .ok_or_else(|| MigrationError::InvalidFormat("Missing version in filename".to_string()))?
    .as_str();
```

---

### 5. String Formatting (Write Trait)

| File | Line | Context | What It Does | Implications | Risk |
|------|------|---------|--------------|--------------|------|
| `src/relation/eager.rs` | 728 | `write!()` macro | Formats foreign key value to string | `write!()` returns `Result`, but we use `unwrap_or_default()` | 🟢 LOW |

**Impact**: Already handled with `unwrap_or_default()`, so this is safe.

**Recommendation**: No change needed - already safe.

---

### 6. Prometheus Exporter Initialization

| File | Line | Context | What It Does | Implications | Risk |
|------|------|---------|--------------|--------------|------|
| `src/metrics.rs` | 64 | `LifeguardMetrics::init()` | Initializes Prometheus exporter | If OpenTelemetry initialization fails, metrics system panics | 🔴 HIGH |

**Impact**: If metrics initialization fails at startup, the entire application panics. This should be handled gracefully.

**Recommendation**: Return `Result` or use a fallback:
```rust
let exporter = opentelemetry_prometheus::exporter()
    .build()
    .map_err(|e| {
        log::error!("Failed to initialize Prometheus exporter: {e}");
        // Return a no-op exporter or handle gracefully
    })?;
```

---

## Test Code - ACCEPTABLE (but should be marked)

### Test Infrastructure

| File | Line | Context | What It Does | Implications | Risk |
|------|------|---------|--------------|--------------|------|
| `src/query/execution.rs` | 1383, 1391 | Paginator test | Asserts cached count works | Test code - panics are acceptable | 🟢 LOW |
| `src/json_helpers.rs` | Multiple | Deserialization tests | Tests JSON parsing | Test code - panics are acceptable | 🟢 LOW |
| `src/value/try_getable.rs` | 498, 505 | Value extraction tests | Tests type conversions | Test code - panics are acceptable | 🟢 LOW |
| `src/migration/registry.rs` | 265+ | Registry tests | Tests migration registration | Test code - many `expect()` calls | 🟢 LOW |
| `src/active_model/traits.rs` | 750-755 | Hook order test | Tests lifecycle hooks | Test code - panics are acceptable | 🟢 LOW |
| `src/test_helpers.rs` | 216, 223, 252 | Test database setup | Environment variable handling | Test code - but should handle errors | 🟡 MEDIUM |
| `src/value/integration_tests.rs` | 72, 84, 161 | Integration tests | Value extraction tests | Test code - panics are acceptable | 🟢 LOW |
| `src/model/try_into_model.rs` | 227 | Conversion test | Model conversion test | Test code - panics are acceptable | 🟢 LOW |
| `src/value/tuple.rs` | 320 | Tuple extraction test | Value tuple test | Test code - panics are acceptable | 🟢 LOW |

**Recommendation**: Add `#[allow(clippy::unwrap_used)]` or `#[cfg(test)]` attributes to test modules to explicitly mark them as acceptable.

---

## Macro/Code Generation Code - REVIEW REQUIRED

### Procedural Macros (`lifeguard-derive/src/`)

| File | Line | Context | What It Does | Implications | Risk |
|------|------|---------|--------------|--------------|------|
| `lifeguard-derive/src/macros/relation.rs` | 119, 153, 168+ | Macro code generation | Assumes validated identifiers exist | If macro input validation fails, generated code panics | 🟡 MEDIUM |
| `lifeguard-derive/src/macros/life_record.rs` | 234 | Generated code | Required field validation | Generated code will panic if required field is None | 🔴 HIGH |
| `lifeguard-derive/src/utils.rs` | 10, 23, 44 | String conversion | Assumes chars exist | Should never fail, but could with invalid input | 🟡 MEDIUM |
| `lifeguard-derive/src/type_conversion.rs` | 1323+ | Type parsing tests | Test code in macro crate | Test code - acceptable | 🟢 LOW |

**Critical Issue**: `life_record.rs:234` generates code that uses `expect()` for required fields. This generated code will be in user's codebase and can panic.

**Recommendation**: 
- For macro code itself: Add validation before unwrap
- For generated code: Consider returning `Result` instead of panicking

---

## Build Scripts (`lifeguard-migrate/src/`)

| File | Line | Context | What It Does | Implications | Risk |
|------|------|---------|--------------|--------------|------|
| `lifeguard-migrate/src/build_script.rs` | 44, 80, 91, 121 | Build-time entity discovery | File system operations | Build fails if filesystem is unexpected | 🟡 MEDIUM |
| `lifeguard-migrate/src/entity_loader.rs` | 45, 53, 119, 130, 158 | Entity loading | Regex and file parsing | Build fails if entity files are malformed | 🟡 MEDIUM |
| `lifeguard-migrate/src/dependency_ordering.rs` | 116, 187 | Topological sort | Dependency resolution | Build fails if dependencies are circular | 🟡 MEDIUM |
| `lifeguard-migrate/src/main.rs` | 122, 139, 617+ | CLI tool | Various operations | CLI tool can panic - affects developer experience | 🟡 MEDIUM |

**Impact**: These are build-time tools, so panics affect developer experience but not production. However, better error messages would improve UX.

**Recommendation**: Add proper error handling with descriptive messages for better developer experience.

---

## Examples Code

| File | Line | Context | What It Does | Implications | Risk |
|------|------|---------|--------------|--------------|------|
| `examples/entities/build.rs` | 34 | Build script | Environment variable check | Build fails if OUT_DIR not set | 🟡 MEDIUM |

**Impact**: Example code - low priority, but should still handle errors gracefully.

---

## Priority Action Items

### 🔴 CRITICAL - Must Fix Before Production

1. **Mutex locks in `src/query/execution.rs`** (9 occurrences)
   - Replace all `Mutex::lock().unwrap()` with proper error handling
   - These are in test infrastructure but could affect production if used

2. **HasManyThrough validation** (6 occurrences)
   - Replace `expect()` with `Result` returns
   - Files: `src/relation/eager.rs`, `src/relation/def/struct_def.rs`

3. **Iterator assumptions** (5+ occurrences)
   - Add validation before calling `.next().unwrap()`
   - Files: `src/relation/eager.rs`, `src/relation/def/condition.rs`

4. **Prometheus initialization** (1 occurrence)
   - Handle initialization failure gracefully
   - File: `src/metrics.rs`

5. **Generated code in macros** (1 occurrence)
   - Change generated code to return `Result` instead of panicking
   - File: `lifeguard-derive/src/macros/life_record.rs`

### 🟡 MEDIUM - Should Fix

1. **Regex captures** - Add error handling even though regex guarantees groups
2. **Build scripts** - Better error messages for developer experience
3. **Test helpers** - Handle environment variable errors gracefully

### 🟢 LOW - Acceptable

1. **Test code** - Mark with `#[allow]` or `#[cfg(test)]`
2. **String formatting** - Already safe with `unwrap_or_default()`

---

## Recommendations

1. **Immediate Actions**:
   - Fix all 🔴 CRITICAL items in production code
   - Add `#[allow(clippy::unwrap_used)]` to test modules
   - Update generated code to use `Result` instead of `expect()`

2. **Code Review Process**:
   - Add pre-commit hook to catch new `unwrap()` usage
   - Review all PRs for panic-causing code
   - Document panic policy in CONTRIBUTING.md

3. **Testing**:
   - Add tests for error paths (poisoned mutex, incomplete relationships)
   - Test that all production code paths return errors instead of panicking

4. **Documentation**:
   - Document that database tools must never panic
   - Provide examples of proper error handling patterns

---

## Statistics

- **Total unwrap/expect calls**: ~230
- **Production code (src/)**: 45 (must fix)
- **Test code**: 120+ (acceptable with annotations)
- **Macro code**: 50+ (review required)
- **Build scripts**: 15+ (improve error messages)

**Estimated Fix Time**: 
- Critical fixes: 2-4 hours
- Medium priority: 4-6 hours
- Total: 6-10 hours
