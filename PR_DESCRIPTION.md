# Implement ValueType Infrastructure (Phase 4: Value Type Infrastructure)

## Summary

This PR implements the complete ValueType infrastructure for Lifeguard, providing enhanced type safety and developer experience for value conversions. This completes **Phase 4** of the SEAORM_LIFEGUARD_MAPPING.md implementation plan.

## Changes

### Core Implementation

- **Created `ValueType` trait** - Type-safe conversions between Rust types and `sea_query::Value`
- **Created `TryGetable` trait** - Safe value extraction with error handling
- **Created `TryGetableMany` trait** - Extract multiple values from collections
- **Created `IntoValueTuple` and `FromValueTuple` traits** - Composite key conversions
- **Created `TryFromU64` trait** - Safe conversion from u64 with overflow handling

### Code Changes

1. **`src/value/mod.rs`** (NEW):
   - Module organization and public API exports
   - Comprehensive module documentation

2. **`src/value/types.rs`** (NEW):
   - `ValueType` trait with `into_value()` and `from_value()` methods
   - `null_value()` helper method for Option<T> support
   - Implementations for all supported Rust types:
     - Integer types: i8, i16, i32, i64, u8, u16, u32, u64
     - Floating point: f32, f64
     - Boolean: bool
     - String: String
     - Binary: Vec<u8>
     - JSON: serde_json::Value
     - Option<T> for all above types
   - Comprehensive test coverage (5 tests)

3. **`src/value/try_getable.rs`** (NEW):
   - `ValueExtractionError` enum with detailed error types:
     - `NullValue` - Value is null
     - `TypeMismatch` - Value type doesn't match expected type
     - `ConversionError` - Conversion failed (e.g., overflow)
   - `TryGetable` trait with `try_get()` and `try_get_opt()` methods
   - `TryGetableMany` trait with `try_get_many()` and `try_get_many_opt()` methods
   - Implementations for all supported types
   - Comprehensive test coverage (7 tests)

4. **`src/value/tuple.rs`** (NEW):
   - `IntoValueTuple` trait for converting Rust tuples to Value tuples
   - `FromValueTuple` trait for converting Value tuples back to Rust tuples
   - Implementations for tuples of size 2-6
   - Vec<Value> support for tuples with 6+ elements (matching PrimaryKeyArity::Tuple6Plus)
   - Comprehensive test coverage (6 tests)

5. **`src/value/u64.rs`** (NEW):
   - `TryFromU64` trait for safe conversion from u64
   - Overflow handling for all integer types (i8, i16, i32, i64, u8, u16, u32, u64)
   - Detailed error messages for overflow cases
   - Comprehensive test coverage (7 tests)

6. **`src/lib.rs`**:
   - Added `value` module export
   - Re-exported all public traits and types

7. **`lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md`**:
   - Updated all ValueType-related entries to ✅ Complete
   - Updated Phase 4 section to mark as complete
   - Added implementation notes and status

## Benefits

1. **Type Safety** - Compile-time guarantees for value conversions
2. **Better Error Handling** - Distinguishes between null values, type mismatches, and conversion errors
3. **Composite Key Support** - Full support for composite primary keys with tuple conversions
4. **Overflow Protection** - Safe conversion from u64 with proper overflow handling
5. **Developer Experience** - Clear error messages and intuitive API

## Example Usage

### ValueType Trait

```rust
use lifeguard::ValueType;
use sea_query::Value;

// Convert Rust type to Value
let value: Value = 42i32.into_value();
assert!(matches!(value, Value::Int(Some(42))));

// Convert Value back to Rust type
let extracted: Option<i32> = ValueType::from_value(value);
assert_eq!(extracted, Some(42));

// Handle Option<T>
let opt_value: Value = Some(42i32).into_value();
let extracted: Option<Option<i32>> = ValueType::from_value(opt_value);
assert_eq!(extracted, Some(Some(42)));
```

### TryGetable Trait

```rust
use lifeguard::{TryGetable, ValueExtractionError};
use sea_query::Value;

// Safe extraction with error handling
let value = Value::Int(Some(42));
let result: Result<i32, ValueExtractionError> = TryGetable::try_get(value);
assert_eq!(result, Ok(42));

// Handle null values
let null_value = Value::Int(None);
let result: Result<i32, ValueExtractionError> = TryGetable::try_get(null_value);
assert!(matches!(result, Err(ValueExtractionError::NullValue)));

// Handle type mismatches
let wrong_type = Value::String(Some("hello".to_string()));
let result: Result<i32, ValueExtractionError> = TryGetable::try_get(wrong_type);
assert!(matches!(result, Err(ValueExtractionError::TypeMismatch { .. })));
```

### TryGetableMany Trait

```rust
use lifeguard::TryGetableMany;
use sea_query::Value;

// Extract multiple values
let values = vec![
    Value::Int(Some(1)),
    Value::Int(Some(2)),
    Value::Int(Some(3)),
];
let result: Result<Vec<i32>, _> = TryGetableMany::try_get_many(values);
assert_eq!(result, Ok(vec![1, 2, 3]));
```

### IntoValueTuple and FromValueTuple

```rust
use lifeguard::{IntoValueTuple, FromValueTuple};
use sea_query::Value;

// Convert tuple to Value tuple
let tuple = (42i32, "hello".to_string());
let value_tuple: (Value, Value) = tuple.into_value_tuple();

// Convert Value tuple back to Rust tuple
let value_tuple = (
    Value::Int(Some(42)),
    Value::String(Some("hello".to_string())),
);
let result: Result<(i32, String), _> = FromValueTuple::from_value_tuple(value_tuple);
assert_eq!(result, Ok((42, "hello".to_string())));
```

### TryFromU64

```rust
use lifeguard::TryFromU64;

// Safe conversion from u64
let value: u64 = 42;
let result: Result<i32, _> = TryFromU64::try_from_u64(value);
assert_eq!(result, Ok(42));

// Overflow protection
let overflow: u64 = i32::MAX as u64 + 1;
let result: Result<i32, _> = TryFromU64::try_from_u64(overflow);
assert!(matches!(result, Err(ValueExtractionError::ConversionError(_))));
```

## Features

### Supported Types

All traits are implemented for:
- **Integer types**: i8, i16, i32, i64, u8, u16, u32, u64
- **Floating point**: f32, f64
- **Boolean**: bool
- **String**: String
- **Binary**: Vec<u8>
- **JSON**: serde_json::Value
- **Option<T>**: For all above types

### Tuple Support

- **Tuples 2-6**: Full support with type-safe conversions
- **Tuples 6+**: Vec<Value> representation (matching PrimaryKeyArity::Tuple6Plus)
- **Mixed types**: Supports tuples with different types (e.g., `(i32, String, bool)`)

### Error Handling

The `ValueExtractionError` enum provides detailed error information:
- **NullValue**: Value is null (None variant)
- **TypeMismatch**: Value type doesn't match expected type (includes expected and actual types)
- **ConversionError**: Conversion failed (e.g., overflow, with detailed message)

## Testing

- ✅ **43 tests** passing across all modules
- ✅ **ValueType tests**: 5 tests (i32, String, Option, bool, f64)
- ✅ **TryGetable tests**: 7 tests (success, null, type mismatch, optional extraction, many extraction)
- ✅ **Tuple tests**: 6 tests (2-tuple, 3-tuple, mixed types, error cases)
- ✅ **TryFromU64 tests**: 7 tests (success cases, overflow cases for all integer types)
- ✅ **All doctests** passing
- ✅ **Error messages** verified to be clear and actionable

## Implementation Phases

### Phase 4: Value Type Infrastructure ✅ **COMPLETE**

1. **ValueType trait** ✅
   - Core trait with `into_value()` and `from_value()` methods
   - `null_value()` helper for Option<T> support
   - Implementations for all supported types

2. **TryGetable trait** ✅
   - Error-aware extraction with `ValueExtractionError`
   - Implementations for all supported types
   - `try_get_opt()` for optional extraction

3. **TryGetableMany trait** ✅
   - Batch extraction from collections
   - `try_get_many()` and `try_get_many_opt()` methods

4. **IntoValueTuple and FromValueTuple** ✅
   - Tuple conversion for composite keys
   - Support for tuples 2-6 and Vec<Value> for 6+

5. **TryFromU64** ✅
   - Safe u64 conversion with overflow handling
   - Implementations for all integer types

## Related Issues

Completes Phase 4 of the implementation plan tracked in `SEAORM_LIFEGUARD_MAPPING.md`:
- `ValueType` trait ✅ **Completed**
- `TryGetable` trait ✅ **Completed**
- `TryGetableMany` trait ✅ **Completed**
- `IntoValueTuple` trait ✅ **Completed**
- `FromValueTuple` trait ✅ **Completed**
- `TryFromU64` trait ✅ **Completed**

## Breaking Changes

None - This is a purely additive feature. All existing code continues to work unchanged.

## Impact

### Value Types & Conversions (199-205) ✅ **COMPLETE**

- **Blocks:** Nothing (composite keys already work)
- **Enables:** Better developer experience, optimizations
- **Impact Score:** 🟡 **3/10** (Low - Optimization)
- **Status:** ✅ All value type traits implemented and tested

This implementation provides the foundation for enhanced type safety and developer experience, while maintaining full backward compatibility with existing code.
