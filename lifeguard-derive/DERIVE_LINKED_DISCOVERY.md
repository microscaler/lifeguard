# DeriveLinked Macro Discovery Document

**Date:** 2026-01-19  
**Branch:** `feature/derive-linked-macro`  
**Status:** 🔍 Discovery Phase

## Executive Summary

This document explores the feasibility, benefits, and design of a `DeriveLinked` macro that would automatically generate `Linked<I, T>` trait implementations, reducing boilerplate code for multi-hop relationship queries.

## Current State

### Manual Implementation Pattern

Currently, users must manually implement `Linked<I, T>` for each multi-hop relationship:

```rust
// Example: User → Posts → Comments
impl Linked<PostEntity, CommentEntity> for UserEntity {
    fn via() -> Vec<RelationDef> {
        vec![
            // First hop: User → Post
            <UserEntity as Related<PostEntity>>::to(),
            // Second hop: Post → Comment
            <PostEntity as Related<CommentEntity>>::to(),
        ]
    }
}
```

### Boilerplate Analysis

For each linked relationship, users must:
1. Write an `impl Linked<I, T> for Entity` block
2. Manually construct a `Vec<RelationDef>` with the path
3. Call `Related::to()` for each hop in the chain
4. Ensure the path is correct (Self → I → T)

**Boilerplate per relationship:** ~8-10 lines of code

## SeaORM Comparison

### SeaORM's Approach

SeaORM does **not** provide a derive macro for `Linked`. Users must manually implement `Linked` trait:

```rust
// SeaORM example
impl Linked<Post, Comment> for User {
    fn via() -> Vec<RelationDef> {
        vec![
            Relation::Posts.def(),
            Relation::Comments.def(),
        ]
    }
}
```

**Key Observations:**
- SeaORM requires manual `Linked` implementations
- SeaORM uses `Relation::Variant.def()` pattern (which Lifeguard now supports)
- No macro-based code generation for `Linked` in SeaORM
- This is a **potential competitive advantage** for Lifeguard

### SeaORM's Linked Trait

From SeaORM documentation:
- `Linked` is used for **chained relations** (multi-hop paths)
- Supports self-referencing chains
- Supports diamond relationships (multiple paths between entities)
- Manual implementation is the standard approach

## Benefits of DeriveLinked Macro

### 1. Reduced Boilerplate

**Before (Manual):**
```rust
impl Linked<PostEntity, CommentEntity> for UserEntity {
    fn via() -> Vec<RelationDef> {
        vec![
            <UserEntity as Related<PostEntity>>::to(),
            <PostEntity as Related<CommentEntity>>::to(),
        ]
    }
}

impl Linked<PostEntity, TagEntity> for UserEntity {
    fn via() -> Vec<RelationDef> {
        vec![
            <UserEntity as Related<PostEntity>>::to(),
            <PostEntity as Related<TagEntity>>::to(),
        ]
    }
}
```

**After (With DeriveLinked):**
```rust
#[derive(DeriveLinked)]
pub enum LinkedRelation {
    #[lifeguard(linked = "PostEntity -> CommentEntity")]
    Comments,
    
    #[lifeguard(linked = "PostEntity -> TagEntity")]
    Tags,
}
```

**Boilerplate reduction:** ~80-90% (from 8-10 lines to 1-2 lines per relationship)

### 2. Compile-Time Safety

- **Path validation:** Macro can verify that each hop in the path has a valid `Related` implementation
- **Type checking:** Ensures intermediate and target entities exist and implement `LifeModelTrait`
- **Error messages:** Clear compile-time errors if relationships don't exist

### 3. Consistency with DeriveRelation

- Matches the pattern users already know from `DeriveRelation`
- Consistent attribute syntax (`#[lifeguard(...)]`)
- Similar enum-based approach

### 4. Discoverability

- Users discover linked relationships through the enum
- IDE autocomplete for available linked paths
- Self-documenting code structure

### 5. Competitive Advantage

- **SeaORM doesn't have this** - this would be a unique feature
- Reduces friction for users migrating from SeaORM
- Demonstrates Lifeguard's focus on developer experience

## Design Considerations

### Option 1: Enum-Based (Recommended)

Similar to `DeriveRelation`, generate an enum with variants:

```rust
#[derive(DeriveLinked)]
pub enum LinkedRelation {
    #[lifeguard(linked = "PostEntity -> CommentEntity")]
    Comments,
    
    #[lifeguard(linked = "PostEntity -> TagEntity")]
    Tags,
    
    // Three-hop relationship
    #[lifeguard(linked = "PostEntity -> CommentEntity -> ReactionEntity")]
    Reactions,
}
```

**Pros:**
- Consistent with `DeriveRelation` pattern
- Easy to discover available linked paths
- Can generate helper methods (e.g., `LinkedRelation::Comments.via()`)

**Cons:**
- Requires an enum (additional type)
- Slightly more verbose than direct impl

### Option 2: Direct Impl Generation

Generate `impl Linked` directly on the entity:

```rust
#[derive(DeriveLinked)]
#[lifeguard(linked = "PostEntity -> CommentEntity")]
pub struct UserEntity;
```

**Pros:**
- No additional enum type
- Direct implementation on entity

**Cons:**
- Can only define one linked path per entity
- Less flexible for multiple paths
- Doesn't match `DeriveRelation` pattern

### Option 3: Hybrid Approach

Generate both enum and direct impls:

```rust
#[derive(DeriveLinked)]
pub enum LinkedRelation {
    #[lifeguard(linked = "PostEntity -> CommentEntity")]
    Comments,
}

// Also generates:
impl Linked<PostEntity, CommentEntity> for UserEntity {
    fn via() -> Vec<RelationDef> {
        LinkedRelation::Comments.via()
    }
}
```

**Pros:**
- Best of both worlds
- Enum for discovery, direct impl for convenience

**Cons:**
- More complex implementation
- Potential for confusion

## Recommended Design: Enum-Based

**Rationale:**
1. **Consistency:** Matches `DeriveRelation` pattern users already know
2. **Flexibility:** Supports multiple linked paths per entity
3. **Discoverability:** Enum variants show all available paths
4. **Extensibility:** Can add helper methods to enum later

## Implementation Approach

### Macro Input Format

```rust
#[derive(DeriveLinked)]
pub enum LinkedRelation {
    // Two-hop: User → Post → Comment
    #[lifeguard(linked = "PostEntity -> CommentEntity")]
    Comments,
    
    // Three-hop: User → Post → Comment → Reaction
    #[lifeguard(linked = "PostEntity -> CommentEntity -> ReactionEntity")]
    Reactions,
    
    // Alternative syntax with explicit intermediate
    #[lifeguard(linked(intermediate = "PostEntity", target = "CommentEntity"))]
    CommentsAlt,
}
```

### Generated Code

For each variant, generate:

```rust
impl Linked<PostEntity, CommentEntity> for UserEntity {
    fn via() -> Vec<RelationDef> {
        vec![
            <UserEntity as Related<PostEntity>>::to(),
            <PostEntity as Related<CommentEntity>>::to(),
        ]
    }
}
```

### Validation

The macro should:
1. **Parse path:** Extract intermediate and target entities from attribute
2. **Verify Related impls:** Check that `Related<I>` and `Related<T>` exist for each hop
3. **Validate path:** Ensure path is valid (e.g., no circular references in simple cases)
4. **Generate errors:** Provide clear compile-time errors for invalid paths

### Edge Cases

1. **Three+ hop paths:** Support arbitrary length paths
   ```rust
   #[lifeguard(linked = "A -> B -> C -> D")]
   ```

2. **Self-referential chains:** User → User (via parent relationship)
   ```rust
   #[lifeguard(linked = "Entity -> Entity")]
   ```

3. **Diamond relationships:** Multiple paths to same target
   ```rust
   #[lifeguard(linked = "PostEntity -> CommentEntity")]
   CommentsViaPosts,
   
   #[lifeguard(linked = "ArticleEntity -> CommentEntity")]
   CommentsViaArticles,
   ```

4. **Module-qualified paths:** Support paths in different modules
   ```rust
   #[lifeguard(linked = "super::posts::PostEntity -> super::comments::CommentEntity")]
   ```

## Comparison Matrix

| Feature | SeaORM | Lifeguard (Current) | Lifeguard (With DeriveLinked) |
|---------|--------|---------------------|-------------------------------|
| Manual `Linked` impl | ✅ Required | ✅ Required | ❌ Optional |
| Macro generation | ❌ No | ❌ No | ✅ Yes |
| Boilerplate | High | High | Low |
| Compile-time validation | Manual | Manual | ✅ Automatic |
| Discoverability | Low | Low | ✅ High (enum) |
| Consistency with Related | Partial | Partial | ✅ Full |

## Implementation Complexity

### Estimated Effort

- **Parsing & Validation:** Medium (similar to `DeriveRelation`)
- **Code Generation:** Low (straightforward `impl Linked` generation)
- **Testing:** Medium (various path lengths, edge cases)
- **Documentation:** Low (similar patterns to `DeriveRelation`)

**Total Estimated Effort:** 2-3 days

### Dependencies

- ✅ `DeriveRelation` exists (can reference `Related` impls)
- ✅ `Linked` trait exists
- ✅ `RelationDef` and `Related::to()` available
- ✅ Macro infrastructure in place

**No blocking dependencies** - all prerequisites are met.

## Testing Strategy

### Unit Tests

1. **Two-hop paths:** User → Post → Comment
2. **Three-hop paths:** User → Post → Comment → Reaction
3. **Self-referential:** Entity → Entity
4. **Module-qualified paths:** `super::module::Entity`
5. **Multiple paths:** Different paths to same target
6. **Error cases:** Invalid paths, missing `Related` impls

### Integration Tests

1. **Query building:** Verify generated `Linked` impls work with `find_linked()`
2. **SQL generation:** Ensure correct JOINs are generated
3. **Composite keys:** Test with composite primary/foreign keys

## Migration Path

### Backward Compatibility

- **Fully backward compatible:** Manual `impl Linked` still works
- **Optional feature:** Users can choose macro or manual implementation
- **No breaking changes:** Existing code continues to work

### Adoption Strategy

1. **Phase 1:** Implement macro with enum-based approach
2. **Phase 2:** Add to documentation and examples
3. **Phase 3:** Update migration guides to recommend macro
4. **Phase 4:** Consider making it the recommended approach

## Open Questions

1. **Enum naming:** Should it be `LinkedRelation` or match entity name (e.g., `UserLinked`)?
   - **Recommendation:** `LinkedRelation` for consistency with `Relation` enum

2. **Path syntax:** Arrow (`->`) vs comma-separated vs explicit attributes?
   - **Recommendation:** Arrow syntax (`->`) - most intuitive

3. **Helper methods:** Should enum have methods like `via()` or `def()`?
   - **Recommendation:** Yes, for consistency with `Relation::def()`

4. **Validation strictness:** How strict should path validation be?
   - **Recommendation:** Validate `Related` impls exist, but allow complex paths (diamonds, etc.)

## Recommendations

### ✅ Proceed with Implementation

**Rationale:**
1. **High value, low risk:** Significant boilerplate reduction with minimal complexity
2. **Competitive advantage:** SeaORM doesn't have this feature
3. **User experience:** Aligns with Lifeguard's focus on developer experience
4. **Consistency:** Matches existing `DeriveRelation` pattern
5. **No blockers:** All prerequisites are in place

### Implementation Priority

**Priority:** Medium (Nice-to-have enhancement)

**Timeline:** Can be implemented after core features are stable

### Success Criteria

- [ ] Macro generates valid `Linked<I, T>` implementations
- [ ] Compile-time validation catches invalid paths
- [ ] Reduces boilerplate by 80%+ compared to manual implementation
- [ ] Works with 2-hop, 3-hop, and self-referential paths
- [ ] Comprehensive test coverage
- [ ] Documentation and examples provided

## Implementation Plan

### File Structure

```
lifeguard-derive/
├── src/
│   ├── lib.rs                    # Add DeriveLinked proc macro registration
│   └── macros/
│       ├── mod.rs                # Export derive_linked function
│       └── linked.rs             # NEW: DeriveLinked macro implementation
├── tests/
│   └── test_derive_linked.rs    # NEW: Comprehensive test suite
└── tests/ui/
    └── compile_error_*.rs        # NEW: UI tests for error cases
```

### Implementation Phases

#### Phase 1: Core Infrastructure (Day 1)

**Goal:** Basic two-hop path parsing and code generation

**Tasks:**
1. **Create `linked.rs` module**
   - Set up basic macro structure
   - Implement enum parsing (similar to `DeriveRelation`)
   - Parse `#[lifeguard(linked = "...")]` attributes

2. **Path Parsing**
   - Parse arrow syntax: `"PostEntity -> CommentEntity"`
   - Extract intermediate and target entity paths
   - Support module-qualified paths: `"super::posts::PostEntity"`

3. **Basic Code Generation**
   - Generate `impl Linked<I, T> for Entity` blocks
   - Generate `via()` method returning `Vec<RelationDef>`
   - Use `Related::to()` for each hop

4. **Integration**
   - Register macro in `lib.rs`
   - Export from `mod.rs`
   - Basic documentation

**Deliverable:** Working macro for two-hop paths

#### Phase 2: Validation & Error Handling (Day 1-2)

**Goal:** Compile-time validation and helpful error messages

**Tasks:**
1. **Path Validation**
   - Verify entity paths are valid Rust paths
   - Check for empty or malformed paths
   - Validate arrow syntax

2. **Related Implementation Checking**
   - Attempt to verify `Related<I>` exists (compile-time check via generated code)
   - Generate clear error messages if `Related` impls are missing
   - Provide suggestions for fixing errors

3. **Error Messages**
   - Clear, actionable error messages
   - Point to specific variant causing the error
   - Suggest fixes (e.g., "Did you mean to use `Related<PostEntity>`?")

**Deliverable:** Robust validation with helpful errors

#### Phase 3: Multi-Hop Support (Day 2)

**Goal:** Support arbitrary-length paths (3+, self-referential)

**Tasks:**
1. **Path Parsing Enhancement**
   - Parse paths with 3+ hops: `"A -> B -> C -> D"`
   - Handle self-referential: `"Entity -> Entity"`
   - Support mixed module paths

2. **Code Generation for Multi-Hop**
   - Generate `via()` with correct number of hops
   - Ensure proper ordering: `Self -> I1 -> I2 -> ... -> T`
   - Handle edge cases (single hop, self-ref)

3. **Testing**
   - Test 2-hop, 3-hop, 4-hop paths
   - Test self-referential chains
   - Test module-qualified paths

**Deliverable:** Full multi-hop path support

#### Phase 4: Helper Methods & Polish (Day 2-3)

**Goal:** Enum helper methods and consistency with DeriveRelation

**Tasks:**
1. **Enum Helper Methods**
   - Generate `via()` method on enum variants (if needed)
   - Consider `def()` method for consistency (returns `Vec<RelationDef>`)
   - Optional: `path()` method returning path description

2. **Documentation**
   - Add comprehensive doc comments
   - Update main documentation
   - Add usage examples

3. **Edge Case Handling**
   - Diamond relationships (multiple paths to same target)
   - Duplicate path detection
   - Circular reference detection (basic)

**Deliverable:** Production-ready macro with helper methods

#### Phase 5: Testing & Documentation (Day 3)

**Goal:** Comprehensive test coverage and user documentation

**Tasks:**
1. **Unit Tests**
   - All path lengths (2-hop through 5-hop)
   - Self-referential chains
   - Module-qualified paths
   - Error cases (invalid paths, missing Related impls)

2. **Integration Tests**
   - Verify generated `Linked` impls work with `find_linked()`
   - Test SQL generation correctness
   - Test with composite keys

3. **UI Tests (Compile Errors)**
   - Invalid syntax errors
   - Missing Related impl errors
   - Malformed path errors

4. **Documentation**
   - Usage guide (similar to `DERIVE_RELATION_USAGE.md`)
   - Migration guide from manual impls
   - Examples in main docs

**Deliverable:** Complete test suite and documentation

### Detailed Implementation Steps

#### Step 1: Create `linked.rs` Module

```rust
// lifeguard-derive/src/macros/linked.rs

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DataEnum, DeriveInput, Variant};

/// Derive macro for `DeriveLinked` - generates Linked trait implementations
///
/// This macro generates:
/// - Linked trait implementations for each relationship variant in the enum
/// - Multi-hop relationship paths using Related trait implementations
///
/// # Example
///
/// ```ignore
/// use lifeguard_derive::DeriveLinked;
///
/// #[derive(DeriveLinked)]
/// pub enum LinkedRelation {
///     #[lifeguard(linked = "PostEntity -> CommentEntity")]
///     Comments,
/// }
/// ```
#[proc_macro_derive(DeriveLinked, attributes(lifeguard))]
pub fn derive_linked(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    
    let enum_name = &input.ident;
    
    // Extract enum variants
    let variants = match &input.data {
        Data::Enum(DataEnum { variants, .. }) => variants,
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "DeriveLinked can only be derived for enums",
            )
            .to_compile_error()
            .into();
        }
    };
    
    // Process each variant to extract linked path information
    let mut linked_impls = Vec::new();
    
    for variant in variants {
        if let Some(linked_impl) = process_linked_variant(variant, enum_name) {
            linked_impls.push(linked_impl);
        }
    }
    
    let expanded: TokenStream2 = quote! {
        #(#linked_impls)*
    };
    
    TokenStream::from(expanded)
}

/// Process a linked variant and generate Linked trait implementation
fn process_linked_variant(
    variant: &Variant,
    enum_name: &syn::Ident,
) -> Option<TokenStream2> {
    // Parse attributes to find linked path
    // Implementation details below...
    todo!()
}
```

#### Step 2: Path Parsing Logic

```rust
/// Parse linked path from attribute
/// 
/// Examples:
/// - "PostEntity -> CommentEntity" -> (PostEntity, CommentEntity)
/// - "PostEntity -> CommentEntity -> ReactionEntity" -> (PostEntity, [CommentEntity, ReactionEntity])
/// - "super::posts::PostEntity -> CommentEntity" -> (super::posts::PostEntity, CommentEntity)
fn parse_linked_path(path_str: &str) -> Result<LinkedPath, syn::Error> {
    // Split by "->" to get hops
    let hops: Vec<&str> = path_str.split("->")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    
    if hops.len() < 2 {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("Linked path must have at least 2 hops (intermediate and target), found {}", hops.len())
        ));
    }
    
    // First hop is intermediate, last is target
    let intermediate = hops[0];
    let target = hops[hops.len() - 1];
    
    // Parse entity paths
    let intermediate_path: syn::Path = syn::parse_str(intermediate)
        .map_err(|e| syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("Invalid intermediate entity path '{}': {}", intermediate, e)
        ))?;
    
    let target_path: syn::Path = syn::parse_str(target)
        .map_err(|e| syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("Invalid target entity path '{}': {}", target, e)
        ))?;
    
    // For multi-hop paths, collect all intermediate hops
    let mut all_hops = Vec::new();
    for hop in &hops[1..] {
        let hop_path: syn::Path = syn::parse_str(hop)
            .map_err(|e| syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("Invalid entity path in hop '{}': {}", hop, e)
            ))?;
        all_hops.push(hop_path);
    }
    
    Ok(LinkedPath {
        intermediate: intermediate_path,
        target: target_path,
        additional_hops: all_hops,
    })
}

struct LinkedPath {
    intermediate: syn::Path,
    target: syn::Path,
    additional_hops: Vec<syn::Path>,
}
```

#### Step 3: Code Generation Pattern

```rust
/// Generate Linked trait implementation
fn generate_linked_impl(
    variant: &Variant,
    enum_name: &syn::Ident,
    path: &LinkedPath,
) -> TokenStream2 {
    let variant_name = &variant.ident;
    
    // Build the path: Self -> I -> T (or Self -> I1 -> I2 -> ... -> T)
    let mut path_segments = Vec::new();
    
    // First hop: Self -> Intermediate
    path_segments.push(quote! {
        <Entity as lifeguard::Related<#intermediate_path>>::to(),
    });
    
    // Additional hops: I1 -> I2, I2 -> I3, etc.
    let mut prev = &path.intermediate;
    for next in &path.additional_hops {
        path_segments.push(quote! {
            <#prev as lifeguard::Related<#next>>::to(),
        });
        prev = next;
    }
    
    // Final hop: Last intermediate -> Target
    path_segments.push(quote! {
        <#prev as lifeguard::Related<#target_path>>::to(),
    });
    
    // Generate the impl block
    quote! {
        impl lifeguard::Linked<#intermediate_path, #target_path> for Entity {
            fn via() -> Vec<lifeguard::RelationDef> {
                vec![
                    #(#path_segments)*
                ]
            }
        }
    }
}
```

#### Step 4: Attribute Parsing

```rust
/// Process a linked variant and extract path information
fn process_linked_variant(
    variant: &Variant,
    enum_name: &syn::Ident,
) -> Option<TokenStream2> {
    let mut linked_path: Option<String> = None;
    
    // Parse attributes
    for attr in &variant.attrs {
        if attr.path().is_ident("lifeguard") {
            if let Err(err) = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("linked") {
                    // Parse: linked = "PostEntity -> CommentEntity"
                    let value: syn::LitStr = meta.value()?.parse()?;
                    linked_path = Some(value.value());
                    Ok(())
                } else {
                    Ok(())
                }
            }) {
                return Some(err.to_compile_error());
            }
        }
    }
    
    // Generate impl if path found
    if let Some(path_str) = linked_path {
        match parse_linked_path(&path_str) {
            Ok(path) => Some(generate_linked_impl(variant, enum_name, &path)),
            Err(err) => Some(err.to_compile_error()),
        }
    } else {
        // No linked attribute - skip this variant
        None
    }
}
```

### Integration Points

#### 1. Register in `lib.rs`

```rust
// lifeguard-derive/src/lib.rs

/// Derive macro for `DeriveLinked` - generates Linked trait implementations
///
/// # Example
///
/// ```ignore
/// use lifeguard_derive::DeriveLinked;
///
/// #[derive(DeriveLinked)]
/// pub enum LinkedRelation {
///     #[lifeguard(linked = "PostEntity -> CommentEntity")]
///     Comments,
/// }
/// ```
#[proc_macro_derive(DeriveLinked, attributes(lifeguard))]
pub fn derive_linked(input: TokenStream) -> TokenStream {
    macros::derive_linked(input)
}
```

#### 2. Export from `mod.rs`

```rust
// lifeguard-derive/src/macros/mod.rs

pub mod linked;

pub use linked::derive_linked;
```

### Error Handling Strategy

#### Compile-Time Validation

1. **Path Syntax Errors**
   - Invalid arrow syntax
   - Empty paths
   - Missing hops

2. **Entity Path Errors**
   - Invalid Rust paths
   - Non-existent entities
   - Module resolution failures

3. **Related Implementation Errors**
   - Missing `Related<I>` impls
   - Missing `Related<T>` impls
   - Type mismatches

#### Error Message Examples

```rust
// Example error messages:

// Invalid syntax
error: Linked path must have at least 2 hops (intermediate and target), found 1
  --> src/entity.rs:15:5
   |
15 |     #[lifeguard(linked = "PostEntity")]
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

// Missing Related impl
error: No `Related<PostEntity>` implementation found for `UserEntity`
  --> src/entity.rs:15:5
   |
15 |     #[lifeguard(linked = "PostEntity -> CommentEntity")]
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   |
   = help: Ensure you have a `Related<PostEntity>` implementation for `UserEntity`
   = help: This is typically generated by `DeriveRelation` macro
```

### Testing Plan

#### Unit Tests (`test_derive_linked.rs`)

```rust
#[test]
fn test_derive_linked_two_hop() {
    // Test: User -> Post -> Comment
}

#[test]
fn test_derive_linked_three_hop() {
    // Test: User -> Post -> Comment -> Reaction
}

#[test]
fn test_derive_linked_self_referential() {
    // Test: Entity -> Entity
}

#[test]
fn test_derive_linked_module_qualified() {
    // Test: super::posts::PostEntity -> CommentEntity
}

#[test]
fn test_derive_linked_multiple_paths() {
    // Test: Multiple variants with different paths
}

#[test]
fn test_derive_linked_diamond_relationship() {
    // Test: Multiple paths to same target
}
```

#### UI Tests (Compile Errors)

```rust
// tests/ui/compile_error_linked_invalid_path.rs
// Should fail with: "Linked path must have at least 2 hops"

// tests/ui/compile_error_linked_missing_related.rs
// Should fail with: "No Related<X> implementation found"
```

#### Integration Tests

```rust
#[test]
fn test_find_linked_with_generated_impl() {
    // Verify find_linked() works with macro-generated Linked impl
}

#[test]
fn test_linked_query_sql_generation() {
    // Verify correct SQL JOINs are generated
}
```

### Code Reuse Opportunities

#### Leverage Existing Infrastructure

1. **Path Parsing:** Reuse entity path parsing from `DeriveRelation`
2. **Error Handling:** Use similar error message patterns
3. **Module Resolution:** Reuse module path handling
4. **Testing Patterns:** Follow `test_derive_relation.rs` structure

#### Shared Utilities

```rust
// Can create shared utilities module if needed:
// lifeguard-derive/src/macros/utils.rs

pub fn parse_entity_path(entity_str: &str, error_context: &str) -> Result<syn::Path, TokenStream2> {
    // Reusable entity path parsing
}
```

### Performance Considerations

- **Compile Time:** Macro should be fast (similar to `DeriveRelation`)
- **Code Size:** Generated code is minimal (just `impl Linked` blocks)
- **Runtime:** No runtime overhead (all compile-time)

### Documentation Requirements

1. **API Documentation**
   - Doc comments on macro
   - Usage examples
   - Attribute syntax reference

2. **User Guide**
   - Create `DERIVE_LINKED_USAGE.md` (similar to `DERIVE_RELATION_USAGE.md`)
   - Migration guide from manual impls
   - Common patterns and examples

3. **Main Documentation**
   - Update `SEAORM_LIFEGUARD_MAPPING.md`
   - Add to main README if appropriate

### Risk Assessment

#### Low Risk
- ✅ Well-defined scope
- ✅ Similar to existing `DeriveRelation` pattern
- ✅ No breaking changes
- ✅ Backward compatible

#### Medium Risk
- ⚠️ Path validation complexity (multi-hop, self-ref)
- ⚠️ Error message quality (needs good UX)

#### Mitigation
- Start with simple 2-hop paths
- Iterate on error messages based on user feedback
- Comprehensive testing

### Success Metrics

- [ ] Macro compiles and generates valid code
- [ ] All unit tests pass
- [ ] UI tests verify error messages
- [ ] Integration tests verify `find_linked()` works
- [ ] Documentation is complete
- [ ] Boilerplate reduction: 80%+ (8-10 lines → 1-2 lines)
- [ ] Zero breaking changes to existing code

## Next Steps

1. **Design Review:** Review this document and gather feedback
2. **Prototype:** Implement basic two-hop path generation
3. **Testing:** Validate with real-world examples
4. **Refinement:** Iterate based on testing results
5. **Documentation:** Update guides and examples

---

**Document Status:** Ready for implementation with detailed plan
