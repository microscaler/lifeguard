# Bug Tracker

This file tracks bugs found and fixed in the Lifeguard codebase.

---

## Fixed Bugs

### Codegen Tool: Incorrect Unsigned Integer Type Detection

**Date:** 2024-12-19  
**Status:** ✅ **FIXED**  
**Priority:** High  
**Severity:** Bug - Could generate invalid Rust code

#### Issue

The `lifeguard-codegen` tool used `starts_with()` to detect unsigned integer types (`u8`, `u16`, `u32`, `u64`), while the equivalent macro code uses exact matching via `matches!()`. This inconsistency caused:

1. **False Positives:** Types like `u128`, `u8x4`, or any custom type starting with "u8"/"u16"/"u32"/"u64" would incorrectly enter the unsigned handling block
2. **Invalid Code Generation:** These false positives would then fall through to the `_ => "i32"` default case in the match statement, generating invalid code like:
   ```rust
   let val: i32 = row.try_get::<&str, i32>("field")?;
   val as u8x4  // ❌ Invalid - u8x4 is not a valid Rust type conversion
   ```
3. **Compilation Failures:** The generated code would fail to compile

#### Root Cause

**File:** `lifeguard-codegen/src/main.rs:162-163`

**Before (Buggy Code):**
```rust
let get_expr = if field.ty.starts_with("u8") || field.ty.starts_with("u16") || 
                  field.ty.starts_with("u32") || field.ty.starts_with("u64") {
    let signed_type = match field.ty.as_str() {
        "u8" => "i16",
        "u16" => "i32",
        "u32" | "u64" => "i64",
        _ => "i32",  // ❌ Fallback for non-matching types that passed starts_with()
    };
    // ... generates invalid code for u128, u8x4, etc.
}
```

**Problem:**
- `starts_with("u8")` matches `"u8"`, `"u8x4"`, `"u8CustomType"`, etc.
- `starts_with("u16")` matches `"u16"`, `"u16x2"`, etc.
- `starts_with("u32")` matches `"u32"`, `"u32x4"`, etc.
- `starts_with("u64")` matches `"u64"`, `"u64x2"`, etc.
- But the inner `match` only handles exact matches, so anything else falls through to `_ => "i32"`

**Macro Code (Correct):**
The macro code in `lifeguard-derive/src/macros/partial_model.rs:102` uses exact matching:
```rust
matches!(ident_str.as_str(), "u8" | "u16" | "u32" | "u64")
```

#### Fix

**File:** `lifeguard-codegen/src/main.rs:163-177`

**After (Fixed Code):**
```rust
let get_expr = match field.ty.as_str() {
    "u8" | "u16" | "u32" | "u64" => {
        let signed_type = match field.ty.as_str() {
            "u8" => "i16",
            "u16" => "i32",
            "u32" | "u64" => "i64",
            _ => unreachable!(), // This should never happen due to outer match
        };
        // ... generates correct conversion code
    }
    _ => {
        // Direct type for all other types (u128, u8x4, custom types, etc.)
        format!("            {}: row.try_get::<&str, {}>(\"{}\")?,", field.name, field.ty, column_name)
    }
};
```

**Changes:**
1. ✅ Replaced `starts_with()` checks with exact `match` on `field.ty.as_str()`
2. ✅ Only exact matches (`"u8"`, `"u16"`, `"u32"`, `"u64"`) enter the unsigned handling block
3. ✅ All other types (including `u128`, `u8x4`, custom types) use direct type handling
4. ✅ Matches the macro code's behavior exactly

#### Tests Added

**File:** `lifeguard-codegen/src/main.rs` (test module)

1. ✅ `test_unsigned_integer_exact_matches` - Verifies u8, u16, u32, u64 generate conversion code
2. ✅ `test_unsigned_integer_edge_cases_not_matched` - Verifies u128, u8x4, u8CustomType do NOT generate conversion code
3. ✅ `test_regular_types_use_direct` - Verifies i32, String, i64 use direct types
4. ✅ `test_mixed_types` - Verifies mixed unsigned/exact/other types work correctly

**Test Results:**
```
running 4 tests
test tests::test_regular_types_use_direct ... ok
test tests::test_mixed_types ... ok
test tests::test_unsigned_integer_edge_cases_not_matched ... ok
test tests::test_unsigned_integer_exact_matches ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured
```

#### Impact

- **Before Fix:** Types like `u128`, `u8x4` would generate invalid code that fails to compile
- **After Fix:** All types are handled correctly:
  - Exact unsigned types (`u8`, `u16`, `u32`, `u64`) → Conversion code
  - Other types (`u128`, `u8x4`, custom types, signed types, strings) → Direct type code

#### Related Files

- `lifeguard-codegen/src/main.rs` - Fixed codegen logic
- `lifeguard-derive/src/macros/partial_model.rs:102` - Reference implementation (uses exact matching)
- `lifeguard-derive/src/macros/life_model.rs:804` - Reference implementation (uses exact matching)
- `lifeguard-derive/src/macros/from_row.rs:69` - Reference implementation (uses exact matching)

---

### Missing `#[test]` Attribute on Option Composite Primary Key Test

**Date:** 2024-12-19  
**Status:** ✅ **FIXED**  
**Priority:** Medium  
**Severity:** Bug - Test coverage gap

#### Issue

The function `test_option_composite_primary_key_value_type` in the `option_composite_pk_entity` module was missing its `#[test]` attribute. This meant the test case for composite primary keys with `Option` types would never be executed by the test runner, silently reducing test coverage for an important edge case.

**File:** `lifeguard-derive/tests/test_minimal.rs:1294`

**Before (Buggy Code):**
```rust
fn test_option_composite_primary_key_value_type() {
    // EDGE CASE: Composite key with Option types - Option should be unwrapped in ValueType tuple
    // ValueType should be (i32, String), not (Option<i32>, Option<String>)
    let _value: <PrimaryKey as PrimaryKeyTrait>::ValueType = (42i32, "test".to_string());
}
```

**Problem:**
- Function was defined without `#[test]` attribute
- Test runner would skip this function entirely
- Important edge case (Option types in composite primary keys) was not being tested
- Reduced test coverage without any indication

#### Root Cause

The `#[test]` attribute was likely lost during a refactoring or merge. The function body and logic were correct, but without the attribute, Rust's test runner doesn't recognize it as a test function.

#### Fix

**File:** `lifeguard-derive/tests/test_minimal.rs:1280-1318`

**After (Fixed Code):**
```rust
#[test]
fn test_option_composite_primary_key_value_type() {
    // EDGE CASE: Composite key with Option types - Option should be unwrapped in ValueType tuple
    // ValueType should be (i32, String), not (Option<i32>, Option<String>)
    let _value: <PrimaryKey as PrimaryKeyTrait>::ValueType = (42i32, "test".to_string());
}
```

**Changes:**
1. ✅ Added `#[test]` attribute to `test_option_composite_primary_key_value_type`
2. ✅ Added missing imports (`PrimaryKeyArity`, `PrimaryKeyArityTrait`, `PrimaryKeyToColumn`)
3. ✅ Added comprehensive test coverage following the pattern from other composite primary key modules:
   - `test_option_composite_primary_key_arity` - Verifies Tuple2 arity for Option-based composite keys
   - `test_option_composite_primary_key_to_column` - Verifies column conversion
   - `test_option_composite_primary_key_auto_increment` - Verifies auto_increment behavior

#### Tests Added

**File:** `lifeguard-derive/tests/test_minimal.rs` (option_composite_pk_entity module)

1. ✅ `test_option_composite_primary_key_value_type` - Now properly marked with `#[test]` attribute
2. ✅ `test_option_composite_primary_key_arity` - Verifies that Option-based composite keys return Tuple2 arity
3. ✅ `test_option_composite_primary_key_to_column` - Verifies to_column conversion for Option-based keys
4. ✅ `test_option_composite_primary_key_auto_increment` - Verifies auto_increment returns false (no attributes present)

**Test Coverage:**
- All tests follow the same pattern as other composite primary key modules (`composite_pk_entity`, `mixed_type_composite_pk_entity`)
- Tests verify that Option types are properly unwrapped in ValueType tuples
- Tests ensure Option-based composite keys behave correctly with all PrimaryKeyTrait methods

#### Impact

- **Before Fix:** Test was silently skipped, reducing coverage for Option-based composite primary keys
- **After Fix:** 
  - Test is now executed by test runner
  - Comprehensive coverage for Option-based composite primary keys
  - Matches test coverage pattern from other composite primary key modules
  - Edge case is now properly validated

#### Related Files

- `lifeguard-derive/tests/test_minimal.rs:1280-1318` - Fixed test module
- `lifeguard-derive/tests/test_minimal.rs:960-1009` - Reference: `composite_pk_entity` module (similar tests)
- `lifeguard-derive/tests/test_minimal.rs:1238-1278` - Reference: `mixed_type_composite_pk_entity` module (similar tests)

---

### DerivePartialModel Macro: Inconsistent FromRow Implementation

**Date:** 2024-12-19  
**Status:** ✅ **FIXED**  
**Priority:** High  
**Severity:** Bug - Inconsistency with codegen and LifeModel macro

#### Issue

The `DerivePartialModel` macro generated `FromRow` implementations using `row.get(column_name)?`, while both the `lifeguard-codegen` tool and the `LifeModel` macro use `row.try_get::<&str, Type>(column_name)?`. This inconsistency could lead to:

1. **Different Error Handling:** `row.get()` and `row.try_get()` have different signatures and error handling behavior
2. **Type Inference Issues:** `row.get()` relies on type inference, while `row.try_get::<&str, Type>()` is explicit
3. **Maintenance Confusion:** Developers might expect consistent patterns across all code generation paths

**File:** `lifeguard-derive/src/macros/partial_model.rs:132-139`

**Before (Inconsistent Code):**
```rust
// For unsigned types:
let val: #signed_type = row.get(#column_name_str)?;

// For regular types:
row.get(#column_name_str)?
```

**Codegen (Correct Pattern):**
```rust
// For unsigned types:
let val: i16 = row.try_get::<&str, i16>("id")?;

// For regular types:
row.try_get::<&str, i32>("id")?
```

**LifeModel Macro (Correct Pattern):**
```rust
// For unsigned types:
let val: #signed_type = row.try_get::<&str, #signed_type>(#column_name_str)?;

// For regular types:
row.try_get::<&str, #field_type>(#column_name_str)?
```

#### Root Cause

The `DerivePartialModel` macro was implemented before the codegen tool, and used `row.get()` which is a simpler API. However, when the codegen tool was created, it used `row.try_get::<&str, Type>()` to match the `LifeModel` macro's pattern. The macro was never updated to match.

#### Fix

**File:** `lifeguard-derive/src/macros/partial_model.rs:131-141`

**After (Fixed Code):**
```rust
// For unsigned types:
let val: #signed_type = row.try_get::<&str, #signed_type>(#column_name_str)?;

// For regular types:
row.try_get::<&str, #field_type>(#column_name_str)?
```

**Changes:**
1. ✅ Updated unsigned type handling to use `row.try_get::<&str, #signed_type>()`
2. ✅ Updated regular type handling to use `row.try_get::<&str, #field_type>()`
3. ✅ Now consistent with both `lifeguard-codegen` and `LifeModel` macro implementations

#### Tests

**Existing Tests:**
- `test_derive_partial_model.rs` - Tests macro-generated partial models
- `test_derive_partial_model_codegen.rs` - Tests codegen-generated partial models

**Verification:**
- Both test files should continue to pass after this fix
- Generated code now matches between macro and codegen approaches

#### Impact

- **Before Fix:** Inconsistent API usage between macro and codegen-generated code
- **After Fix:** 
  - All code generation paths use the same `row.try_get::<&str, Type>()` pattern
  - Consistent error handling and type inference across all partial model implementations
  - Easier maintenance and debugging

#### Related Files

- `lifeguard-derive/src/macros/partial_model.rs:131-141` - Fixed macro implementation
- `lifeguard-codegen/src/main.rs:171-175` - Reference: codegen implementation (already correct)
- `lifeguard-derive/src/macros/life_model.rs:834-840` - Reference: LifeModel macro implementation (already correct)

---

### DerivePartialModel Macro: Panic on Invalid Entity Path Segments

**Date:** 2025-01-27  
**Status:** ✅ **FIXED**  
**Priority:** High  
**Severity:** Bug - Macro expansion panic with unhelpful error message

#### Issue

When `syn::parse_str::<syn::Path>()` fails for the entity path attribute, the fallback code splits the string by `"::"` and creates identifiers from each segment. If the entity path is empty (`""`), has trailing/leading colons (`"foo::"`, `"::bar"`), or contains consecutive colons (`"foo::::bar"`), the split produces empty string segments. Calling `syn::Ident::new("")` with an empty string panics, crashing the macro expansion with an unhelpful error instead of returning a proper compile-time error message.

**File:** `lifeguard-derive/src/macros/partial_model.rs:213-224`

**Before (Buggy Code):**
```rust
let segments: Vec<&str> = entity_path_str.split("::").collect();
let mut path = syn::Path {
    leading_colon: None,
    segments: syn::punctuated::Punctuated::new(),
};
for segment in segments {
    path.segments.push(syn::PathSegment {
        ident: syn::Ident::new(segment, proc_macro2::Span::call_site()),  // ❌ Panics if segment is ""
        arguments: syn::PathArguments::None,
    });
}
```

**Problem:**
- Empty string `""` → `[""]` → `syn::Ident::new("")` panics
- Leading colons `"::foo"` → `["", "foo"]` → `syn::Ident::new("")` panics
- Trailing colons `"foo::"` → `["foo", ""]` → `syn::Ident::new("")` panics
- Consecutive colons `"foo::::bar"` → `["foo", "", "", "bar"]` → `syn::Ident::new("")` panics
- Panic provides no helpful error message to the user
- Macro expansion crashes instead of reporting a compile error

#### Root Cause

The fallback path construction code didn't validate path segments before attempting to create `syn::Ident` instances. The `syn::Ident::new()` function panics when given an empty string, which can occur when splitting malformed entity paths.

#### Fix

**File:** `lifeguard-derive/src/macros/partial_model.rs:206-261`

**After (Fixed Code):**
```rust
if let Some(entity_path_str) = entity_path_str {
    // Validate that the entity path is not empty
    if entity_path_str.trim().is_empty() {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "Entity path cannot be empty. Use #[lifeguard(entity = \"path::to::Entity\")] with a valid path.",
        )
        .to_compile_error());
    }
    
    // ... parse path ...
    
    // Validate segments: check for empty segments that would cause syn::Ident::new to panic
    for (idx, segment) in segments.iter().enumerate() {
        if segment.is_empty() {
            let error_msg = if segments.len() == 1 {
                format!("Entity path cannot be empty. Found empty string in #[lifeguard(entity = \"{}\")].", entity_path_str)
            } else if idx == 0 {
                format!("Entity path has leading colons. Found empty segment at start in #[lifeguard(entity = \"{}\")]. Use a valid path like \"foo::Entity\" or \"Entity\".", entity_path_str)
            } else if idx == segments.len() - 1 {
                format!("Entity path has trailing colons. Found empty segment at end in #[lifeguard(entity = \"{}\")]. Use a valid path like \"foo::Entity\" or \"Entity\".", entity_path_str)
            } else {
                format!("Entity path has consecutive colons. Found empty segment at position {} in #[lifeguard(entity = \"{}\")]. Use a valid path like \"foo::Entity\" or \"Entity\".", idx + 1, entity_path_str)
            };
            
            return Err(syn::Error::new_spanned(
                &input.ident,
                error_msg,
            )
            .to_compile_error());
        }
    }
    
    // At this point, we've validated that segment is not empty
    for segment in segments {
        path.segments.push(syn::PathSegment {
            ident: syn::Ident::new(segment, proc_macro2::Span::call_site()),
            arguments: syn::PathArguments::None,
        });
    }
}
```

**Changes:**
1. ✅ Added validation for empty entity path strings
2. ✅ Added validation loop to check all path segments before creating identifiers
3. ✅ Generate helpful error messages for each type of malformed path:
   - Empty string: Clear message about empty path
   - Leading colons: Message with example of valid path
   - Trailing colons: Message with example of valid path
   - Consecutive colons: Message indicating position of error
4. ✅ Return compile errors instead of panicking
5. ✅ Only create `syn::Ident` instances after validation passes

#### Tests Added

**File:** `lifeguard-derive/tests/ui.rs`

**Negative Test Cases (should fail to compile):**
1. ✅ `compile_error_partial_model_empty_entity` - Verifies empty string `""` produces compile error
2. ✅ `compile_error_partial_model_leading_colons` - Verifies `"::foo"` produces compile error
3. ✅ `compile_error_partial_model_trailing_colons` - Verifies `"foo::"` produces compile error
4. ✅ `compile_error_partial_model_consecutive_colons` - Verifies `"foo::::bar"` produces compile error

**Positive Test Cases (should compile successfully):**
1. ✅ `compile_pass_partial_model_valid_paths` - Verifies valid paths work:
   - Simple identifier: `"UserEntity"`
   - Qualified path: `"users::Entity"`
   - Fully qualified path: `"crate::users::Entity"`
   - Super path: `"super::UserEntity"`
   - Multi-segment path: `"crate::models::users::Entity"`

**Test Files:**
- `lifeguard-derive/tests/ui/compile_error_partial_model_empty_entity.rs`
- `lifeguard-derive/tests/ui/compile_error_partial_model_leading_colons.rs`
- `lifeguard-derive/tests/ui/compile_error_partial_model_trailing_colons.rs`
- `lifeguard-derive/tests/ui/compile_error_partial_model_consecutive_colons.rs`
- `lifeguard-derive/tests/ui/compile_pass_partial_model_valid_paths.rs`

**Test Results:**
- All negative test cases produce helpful compile errors (no panics)
- All positive test cases compile successfully
- Error messages are clear and actionable

#### Impact

- **Before Fix:** 
  - Invalid entity paths caused macro expansion to panic
  - No helpful error messages for users
  - Panic messages were cryptic and unhelpful
  - Users couldn't understand what went wrong

- **After Fix:**
  - Invalid entity paths produce clear compile errors
  - Error messages explain the problem and suggest fixes
  - Macro expansion fails gracefully with helpful diagnostics
  - Users can easily understand and fix their code

#### Related Files

- `lifeguard-derive/src/macros/partial_model.rs:206-261` - Fixed validation logic
- `lifeguard-derive/tests/ui.rs` - Added test cases
- `lifeguard-derive/tests/ui/compile_error_partial_model_*.rs` - Negative test cases
- `lifeguard-derive/tests/ui/compile_pass_partial_model_valid_paths.rs` - Positive test cases

---

## Open Bugs

*No open bugs at this time.*

---

## Bug Report Template

```markdown
### [Bug Title]

**Date:** YYYY-MM-DD  
**Status:** 🔴 OPEN / 🟡 IN PROGRESS / ✅ FIXED  
**Priority:** Low / Medium / High / Critical  
**Severity:** Bug / Regression / Performance / Security

#### Issue
[Description of the bug]

#### Root Cause
[What caused the bug]

#### Fix
[How it was fixed]

#### Tests
[Tests added/updated]

#### Impact
[What was affected]
```
