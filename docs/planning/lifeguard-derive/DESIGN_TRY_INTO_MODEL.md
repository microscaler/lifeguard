# Design Document: TryIntoModel Trait Implementation

**Date:** 2025-01-27  
**Status:** 🟡 **Design Phase**  
**Priority:** Medium  
**Related:** SeaORM `TryIntoModel` trait mapping

---

## Overview

This document describes the design and implementation plan for adding `TryIntoModel` trait support to Lifeguard, matching SeaORM's functionality for converting custom types (DTOs, partial models, query results) into Model instances with proper error handling.

---

## 1. Problem Statement

### Current State

Lifeguard has several conversion mechanisms:
- ✅ `FromRow` trait - converts database rows to Models
- ✅ `Record::from_model()` / `Record::to_model()` - Model ↔ Record conversions
- ✅ `into_model::<M>()` on `SelectQuery` - query result typing

However, there's no generic trait for converting arbitrary types (DTOs, partial models, external types) into Models with error handling.

### Use Cases

1. **API Request DTOs → Models**
   ```rust
   struct CreateUserRequest {
       name: String,
       email: String,
   }
   
   // Want: CreateUserRequest → UserModel
   let model: UserModel = request.try_into_model()?;
   ```

2. **Partial/Denormalized Data → Models**
   ```rust
   struct UserInput {
       name: String,
       // Missing: id, email, etc.
   }
   
   // Want: UserInput → UserModel (with defaults for missing fields)
   let model: UserModel = input.try_into_model()?;
   ```

3. **External Types → Models**
   ```rust
   struct ExternalUserData {
       user_name: String,  // Different field name
       user_email: String,
   }
   
   // Want: ExternalUserData → UserModel (with field mapping)
   let model: UserModel = external.try_into_model()?;
   ```

### SeaORM Reference

In SeaORM, `TryIntoModel<M>` is a trait that:
- Allows converting some type into a `Model<M>` (where `M: ModelTrait`)
- Returns `Result<M, DbErr>` on failure
- Has a trivial implementation for `M` → `M` (self-conversion)
- Requires manual implementation or custom derive for other types

---

## 2. Design

### 2.1 Trait Definition

```rust
/// Trait for converting types into Model instances
///
/// This trait provides a generic way to convert arbitrary types (DTOs, partial models,
/// external types) into Model instances with proper error handling.
///
/// # Example
///
/// ```rust
/// use lifeguard::TryIntoModel;
///
/// struct CreateUserRequest {
///     name: String,
///     email: String,
/// }
///
/// impl TryIntoModel<UserModel> for CreateUserRequest {
///     type Error = lifeguard::LifeError;
///
///     fn try_into_model(self) -> Result<UserModel, Self::Error> {
///         Ok(UserModel {
///             id: 0,  // Default for new records
///             name: self.name,
///             email: self.email,
///         })
///     }
/// }
/// ```
pub trait TryIntoModel<M>
where
    M: ModelTrait,
{
    /// The error type returned by conversion
    type Error: std::error::Error + Send + Sync + 'static;

    /// Attempt to convert `self` into a Model instance
    ///
    /// # Returns
    ///
    /// Returns `Ok(M)` if conversion succeeds, or `Err(Self::Error)` if conversion fails.
    fn try_into_model(self) -> Result<M, Self::Error>;
}
```

### 2.2 Default Implementation

Provide a trivial implementation for `M → M` (self-conversion):

```rust
impl<M> TryIntoModel<M> for M
where
    M: ModelTrait,
{
    type Error = std::convert::Infallible;

    fn try_into_model(self) -> Result<M, Self::Error> {
        Ok(self)
    }
}
```

### 2.3 Derive Macro

Create `DeriveTryIntoModel` macro to auto-generate implementations:

```rust
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel")]
struct CreateUserRequest {
    name: String,
    email: String,
    // Missing fields (id, etc.) will use Default::default()
}
```

**Generated code:**
```rust
impl TryIntoModel<UserModel> for CreateUserRequest {
    type Error = lifeguard::LifeError;

    fn try_into_model(self) -> Result<UserModel, Self::Error> {
        Ok(UserModel {
            id: Default::default(),  // Missing field - use default
            name: self.name,         // Direct field mapping
            email: self.email,       // Direct field mapping
        })
    }
}
```

### 2.4 Field Mapping Strategies

The macro should support several field mapping strategies:

#### 2.4.1 Direct Field Mapping (Default)
Fields with the same name map directly:
```rust
struct Input { name: String }
struct Model { name: String }
// name → name ✅
```

#### 2.4.2 Custom Field Mapping
Use `#[lifeguard(map_from = "...")]` attribute:
```rust
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel")]
struct Input {
    #[lifeguard(map_from = "name")]
    user_name: String,  // Maps to UserModel.name
}
```

#### 2.4.3 Missing Fields
Handle missing fields with:
- `Default::default()` for non-Option types
- `None` for Option types
- Error if field is required and no default available

#### 2.4.4 Type Conversions
Handle common type conversions:
- `String` → `i32` (via `parse()`)
- `Option<T>` → `T` (unwrap or error)
- `T` → `Option<T>` (wrap in Some)
- Custom conversions via `#[lifeguard(convert = "...")]` attribute

### 2.5 Error Handling

Use `LifeError` as the default error type:

```rust
impl TryIntoModel<M> for SomeType {
    type Error = LifeError;  // Default
    
    fn try_into_model(self) -> Result<M, LifeError> {
        // Conversion logic
    }
}
```

Allow custom error types via attribute:
```rust
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel", error = "CustomError")]
struct Input { ... }
```

---

## 3. Implementation Plan

### Phase 1: Core Trait (Foundation)

**Goal:** Define the trait and provide default implementation

**Tasks:**
1. ✅ Create `src/model/try_into_model.rs` module
2. ✅ Define `TryIntoModel<M>` trait with `Error` associated type
3. ✅ Implement trivial `M → M` conversion
4. ✅ Export trait from `src/model/mod.rs`
5. ✅ Add trait to public API (`src/lib.rs`)

**Files:**
- `src/model/try_into_model.rs` (new)
- `src/model/mod.rs` (update exports)
- `src/lib.rs` (update exports)

**Tests:**
- Test trivial `M → M` conversion
- Test trait bounds and error types

---

### Phase 2: Derive Macro (Code Generation)

**Goal:** Create `DeriveTryIntoModel` macro to auto-generate implementations

**Tasks:**
1. ✅ Create `lifeguard-derive/src/macros/try_into_model.rs`
2. ✅ Parse `#[lifeguard(model = "...")]` attribute
3. ✅ Parse `#[lifeguard(map_from = "...")]` field attributes
4. ✅ Parse `#[lifeguard(convert = "...")]` field attributes
5. ✅ Parse `#[lifeguard(error = "...")]` struct attribute
6. ✅ Generate field mapping code
7. ✅ Handle missing fields (use Default::default())
8. ✅ Handle type conversions
9. ✅ Generate error handling code
10. ✅ Register macro in `lifeguard-derive/src/lib.rs`

**Files:**
- `lifeguard-derive/src/macros/try_into_model.rs` (new)
- `lifeguard-derive/src/lib.rs` (register macro)
- `lifeguard-derive/src/macros/mod.rs` (export macro)

**Tests:**
- Test basic field mapping
- Test custom field mapping (`map_from`)
- Test missing fields (defaults)
- Test type conversions
- Test error handling
- Test with Option types
- Test with nested types

---

### Phase 3: Advanced Features (Enhancements)

**Goal:** Add advanced conversion features

**Tasks:**
1. ✅ Support custom conversion functions
2. ✅ Support validation hooks
3. ✅ Support field renaming strategies (snake_case, camelCase, etc.)
4. ✅ Support partial model conversions
5. ✅ Support nested struct conversions

**Files:**
- `lifeguard-derive/src/macros/try_into_model.rs` (enhance)

**Tests:**
- Test custom conversion functions
- Test validation hooks
- Test field renaming
- Test partial models
- Test nested structs

---

### Phase 4: Documentation & Examples

**Goal:** Document usage and provide examples

**Tasks:**
1. ✅ Add trait documentation with examples
2. ✅ Add macro documentation with examples
3. ✅ Create example file: `examples/try_into_model_example.rs`
4. ✅ Update `SEAORM_LIFEGUARD_MAPPING.md` to mark as ✅ Implemented
5. ✅ Add to changelog/release notes

**Files:**
- `src/model/try_into_model.rs` (documentation)
- `lifeguard-derive/src/macros/try_into_model.rs` (documentation)
- `examples/try_into_model_example.rs` (new)
- `lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md` (update)

---

## 4. API Design

### 4.1 Trait API

```rust
pub trait TryIntoModel<M>
where
    M: ModelTrait,
{
    type Error: std::error::Error + Send + Sync + 'static;
    fn try_into_model(self) -> Result<M, Self::Error>;
}
```

### 4.2 Macro API

```rust
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "TargetModel")]
struct SourceType {
    // Direct field mapping
    field1: Type1,
    
    // Custom field mapping
    #[lifeguard(map_from = "target_field_name")]
    field2: Type2,
    
    // Custom conversion
    #[lifeguard(convert = "custom_conversion_fn")]
    field3: Type3,
    
    // Optional field (missing is OK)
    #[lifeguard(optional)]
    field4: Option<Type4>,
}
```

### 4.3 Usage Examples

**Basic Usage:**
```rust
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel")]
struct CreateUserRequest {
    name: String,
    email: String,
}

let request = CreateUserRequest {
    name: "John".to_string(),
    email: "john@example.com".to_string(),
};

let model: UserModel = request.try_into_model()?;
```

**With Custom Mapping:**
```rust
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel")]
struct ExternalUserData {
    #[lifeguard(map_from = "name")]
    user_name: String,
    
    #[lifeguard(map_from = "email")]
    user_email: String,
}
```

**With Type Conversion:**
```rust
#[derive(DeriveTryIntoModel)]
#[lifeguard(model = "UserModel")]
struct StringIdUser {
    #[lifeguard(convert = "parse_id")]
    id: String,  // Converts to i32
    name: String,
}

fn parse_id(s: String) -> Result<i32, LifeError> {
    s.parse().map_err(|e| LifeError::Other(format!("Invalid ID: {}", e)))
}
```

---

## 5. Error Handling Strategy

### 5.1 Default Error Type

Use `LifeError` as the default error type:

```rust
impl TryIntoModel<M> for SomeType {
    type Error = LifeError;  // Default
}
```

### 5.2 Error Cases

Handle these error cases:
1. **Missing Required Field** - Field not in source, no default available
2. **Type Conversion Failure** - String → i32 parse error, etc.
3. **Custom Validation Failure** - User-provided validation fails
4. **Field Mapping Error** - Target field doesn't exist in Model

### 5.3 Error Messages

Provide clear error messages:
```rust
LifeError::Other(format!(
    "Failed to convert {} to {}: missing required field '{}'",
    std::any::type_name::<Self>(),
    std::any::type_name::<M>(),
    field_name
))
```

---

## 6. Testing Strategy

### 6.1 Unit Tests

Test each component:
- Trait definition and bounds
- Default `M → M` implementation
- Macro-generated code
- Field mapping strategies
- Type conversions
- Error handling

### 6.2 Integration Tests

Test real-world scenarios:
- DTO → Model conversion
- Partial data → Model conversion
- External types → Model conversion
- Error cases and edge cases

### 6.3 Compile-Fail Tests

Test macro error cases:
- Missing `model` attribute
- Invalid field mappings
- Type mismatches
- Invalid conversion functions

---

## 7. Migration Path

### 7.1 Backward Compatibility

- ✅ No breaking changes - trait is additive
- ✅ Existing code continues to work
- ✅ Optional feature - users opt-in via derive macro

### 7.2 Adoption Strategy

1. **Phase 1:** Core trait (users can manually implement)
2. **Phase 2:** Derive macro (convenience for common cases)
3. **Phase 3:** Advanced features (power users)

---

## 8. Future Enhancements

### 8.1 Advanced Features

- **Bidirectional Conversion:** `TryFromModel` trait (Model → DTO)
- **Automatic Field Mapping:** Infer mappings from field names/types
- **Validation Hooks:** `#[lifeguard(validate = "...")]` attribute
- **Nested Conversions:** Convert nested structs automatically
- **Collection Conversions:** `Vec<DTO> → Vec<Model>`

### 8.2 Performance Optimizations

- **Zero-Copy Conversions:** Where possible, avoid cloning
- **Lazy Validation:** Validate only when needed
- **Caching:** Cache conversion logic for repeated conversions

---

## 9. Implementation Checklist

### Phase 1: Core Trait
- [ ] Create `src/model/try_into_model.rs`
- [ ] Define `TryIntoModel<M>` trait
- [ ] Implement default `M → M` conversion
- [ ] Export from `src/model/mod.rs`
- [ ] Export from `src/lib.rs`
- [ ] Add unit tests
- [ ] Add documentation

### Phase 2: Derive Macro
- [ ] Create `lifeguard-derive/src/macros/try_into_model.rs`
- [ ] Parse `#[lifeguard(model = "...")]` attribute
- [ ] Parse field attributes (`map_from`, `convert`, `optional`)
- [ ] Generate field mapping code
- [ ] Handle missing fields (defaults)
- [ ] Handle type conversions
- [ ] Generate error handling
- [ ] Register macro in `lifeguard-derive/src/lib.rs`
- [ ] Add compile-fail tests
- [ ] Add integration tests
- [ ] Add documentation

### Phase 3: Advanced Features
- [ ] Custom conversion functions
- [ ] Validation hooks
- [ ] Field renaming strategies
- [ ] Partial model support
- [ ] Nested struct support

### Phase 4: Documentation
- [ ] Trait documentation
- [ ] Macro documentation
- [ ] Example file
- [ ] Update `SEAORM_LIFEGUARD_MAPPING.md`
- [ ] Update changelog

---

## 10. Open Questions

1. **Error Type:** Should we use `LifeError` or allow custom error types?
   - **Decision:** Use `LifeError` as default, allow custom via attribute

2. **Missing Fields:** Should missing fields error or use defaults?
   - **Decision:** Use `Default::default()` for non-Option, `None` for Option, error if no default available

3. **Type Conversions:** Which conversions should be automatic?
   - **Decision:** Start with manual (`convert` attribute), add common ones later

4. **Validation:** Should validation be part of conversion or separate?
   - **Decision:** Separate validation hooks (future enhancement)

---

## 11. References

- SeaORM `TryIntoModel` trait: https://docs.rs/sea-orm/latest/sea_orm/entity/trait.TryIntoModel.html
- Lifeguard `FromRow` trait: `src/query/traits.rs`
- Lifeguard `Record` conversions: `src/active_model/traits.rs`
- SeaORM/Lifeguard Mapping: `lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md`

---

## 12. Success Criteria

Implementation is complete when:
- ✅ `TryIntoModel` trait is defined and exported
- ✅ Default `M → M` implementation works
- ✅ `DeriveTryIntoModel` macro generates correct code
- ✅ Field mapping works (direct and custom)
- ✅ Missing fields handled correctly (defaults)
- ✅ Type conversions work
- ✅ Error handling is clear and helpful
- ✅ Tests pass (unit, integration, compile-fail)
- ✅ Documentation is complete
- ✅ `SEAORM_LIFEGUARD_MAPPING.md` updated to ✅ Implemented
