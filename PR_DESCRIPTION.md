# Implement ModelTrait::get_value_type() for Runtime Type Introspection

## Summary

This PR implements the `get_value_type()` method on `ModelTrait`, enabling runtime type introspection for model columns. This method returns the Rust type string representation for a given column, which is useful for dynamic serialization, type validation, and runtime type checking.

## Changes

### Core Implementation

- **Added `get_value_type()` method to `ModelTrait`** - Returns `Option<&'static str>` with the Rust type string for a column
- **Created `type_to_string()` helper function** - Converts `syn::Type` to string representation
- **Updated `LifeModel` macro** - Generates `get_value_type()` implementations for each column

### Code Changes

1. **`src/model.rs`**:
   - Added `get_value_type()` method to `ModelTrait` trait
   - Default implementation returns `None` (macro overrides with actual type strings)
   - Comprehensive documentation with examples

2. **`lifeguard-derive/src/type_conversion.rs`**:
   - Added `type_to_string()` function to convert `syn::Type` to string
   - Handles simple types, `Option<T>`, path types (e.g., `serde_json::Value`), and generic types (e.g., `Vec<u8>`)
   - Recursive handling of nested generics and tuples

3. **`lifeguard-derive/src/macros/life_model.rs`**:
   - Added `get_value_type_match_arms` vector to collect match arms
   - Generates match arm for each column returning its type string
   - Uses `type_to_string()` to convert field types to strings
   - Generates `get_value_type()` implementation in ModelTrait impl

4. **`lifeguard-derive/tests/test_minimal.rs`**:
   - Added `test_model_trait_get_value_type()` - Tests basic types (i32, String)
   - Added `test_model_trait_get_value_type_with_options()` - Tests Option<T> types

5. **`lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md`**:
   - Updated `get_value_type()` status from 🟡 Future to ✅ Complete
   - Added implementation notes

## Benefits

1. **Runtime Type Introspection** - Query column types at runtime without compile-time knowledge
2. **Dynamic Serialization** - Use type information for custom serialization logic
3. **Type Validation** - Validate values against expected types at runtime
4. **Developer Experience** - Better debugging and introspection capabilities
5. **API Completeness** - Completes ModelTrait API as documented in SEAORM_LIFEGUARD_MAPPING.md

## Example Usage

### Basic Usage

```rust
use lifeguard::ModelTrait;

let model = UserModel {
    id: 1,
    name: "John".to_string(),
    email: "john@example.com".to_string(),
};

// Get type for id column
let id_type = model.get_value_type(User::Column::Id);
assert_eq!(id_type, Some("i32"));

// Get type for name column
let name_type = model.get_value_type(User::Column::Name);
assert_eq!(name_type, Some("String"));
```

### With Option<T> Fields

```rust
use lifeguard::ModelTrait;

let model = UserWithOptionsModel {
    id: 1,
    name: Some("John".to_string()),
    age: Some(30),
    active: Some(true),
};

// Get type for optional fields
let name_type = model.get_value_type(UserWithOptions::Column::Name);
assert_eq!(name_type, Some("Option<String>"));

let age_type = model.get_value_type(UserWithOptions::Column::Age);
assert_eq!(age_type, Some("Option<i32>"));
```

### Dynamic Type Checking

```rust
use lifeguard::ModelTrait;

fn validate_column_type(model: &impl ModelTrait, column: Column, expected_type: &str) -> bool {
    model.get_value_type(column)
        .map(|actual_type| actual_type == expected_type)
        .unwrap_or(false)
}

// Usage
let model = UserModel { /* ... */ };
assert!(validate_column_type(&model, User::Column::Id, "i32"));
assert!(!validate_column_type(&model, User::Column::Id, "String"));
```

## Supported Type Representations

The method returns type strings in standard Rust syntax:

- **Simple types**: `"i32"`, `"String"`, `"bool"`, `"f64"`
- **Option types**: `"Option<i32>"`, `"Option<String>"`
- **Path types**: `"serde_json::Value"`
- **Generic types**: `"Vec<u8>"`
- **Complex types**: Full path representation (e.g., `"std::collections::HashMap<String, i32>"`)

## Testing

- ✅ **2 new tests** added and passing
- ✅ **Basic type tests**: i32, String types
- ✅ **Option type tests**: Option<String>, Option<i32>, Option<bool>
- ✅ **All existing tests** continue to pass
- ✅ **No breaking changes** - purely additive feature

## Implementation Details

### Type String Generation

The `type_to_string()` function recursively processes Rust types:

1. **Path types** (e.g., `i32`, `String`): Returns the identifier
2. **Generic types** (e.g., `Option<T>`, `Vec<T>`): Includes angle brackets with inner types
3. **Path segments** (e.g., `serde_json::Value`): Joins segments with `::`
4. **Tuples**: Formats as `"(T1, T2, T3)"`
5. **Other types**: Returns descriptive strings (e.g., `"array"`, `"slice"`)

### Macro Generation

The `LifeModel` macro generates a match statement for each column:

```rust
fn get_value_type(&self, column: Column) -> Option<&'static str> {
    match column {
        Column::Id => Some("i32"),
        Column::Name => Some("String"),
        Column::Email => Some("Option<String>"),
        // ... etc
    }
}
```

## Related Issues

Completes the `get_value_type()` feature request tracked in `SEAORM_LIFEGUARD_MAPPING.md`:
- `ModelTrait::get_value_type()` ✅ **Completed**

## Breaking Changes

None - This is a purely additive feature. All existing code continues to work unchanged.

## Impact

### ModelTrait Enhancement

- **Blocks:** Nothing
- **Enables:** Runtime type introspection, dynamic serialization, type validation
- **Impact Score:** 🟡 **4/10** (Medium - Developer Experience)
- **Status:** ✅ Implemented and tested

This implementation provides runtime type introspection capabilities while maintaining full backward compatibility with existing code.
