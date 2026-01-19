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

## Next Steps

1. **Design Review:** Review this document and gather feedback
2. **Prototype:** Implement basic two-hop path generation
3. **Testing:** Validate with real-world examples
4. **Refinement:** Iterate based on testing results
5. **Documentation:** Update guides and examples

---

**Document Status:** Ready for review and implementation planning
