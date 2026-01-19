# Implement DeriveLinked Macro for Multi-Hop Relationship Queries

## Summary

This PR implements the `DeriveLinked` macro, which automatically generates `Linked<I, T>` trait implementations from enum variants, reducing boilerplate for multi-hop relationship queries by 80%+. This is a **competitive advantage** feature - SeaORM doesn't have an equivalent macro.

## Changes

### Core Implementation

- **Created `DeriveLinked` macro** - Generates `Linked<I, T>` trait implementations from enum variants with `#[lifeguard(linked = "...")]` attributes
- **Path parsing** - Supports arrow syntax (`PostEntity -> CommentEntity`) with module-qualified paths
- **Multi-hop support** - Handles 2-hop, 3-hop, and arbitrary-length paths
- **Self-referential paths** - Supports self-referential chains (`Entity -> Entity`)
- **Comprehensive error handling** - Clear, actionable compile-time error messages

### Code Changes

1. **`lifeguard-derive/src/macros/linked.rs`** (NEW):
   - Core macro implementation with path parsing and code generation
   - Parses arrow syntax: `"PostEntity -> CommentEntity"`
   - Generates `impl Linked<I, T> for Entity` blocks
   - Supports module-qualified paths: `"super::posts::PostEntity"`
   - Validates path syntax and provides helpful error messages

2. **`lifeguard-derive/src/lib.rs`**:
   - Registered `DeriveLinked` macro with full documentation
   - Added example usage in doc comments

3. **`lifeguard-derive/src/macros/mod.rs`**:
   - Added `linked` module export

4. **`lifeguard-derive/tests/test_derive_linked.rs`** (NEW):
   - Comprehensive test suite with 4 passing tests:
     - `test_derive_linked_two_hop`: User → Post → Comment
     - `test_derive_linked_three_hop`: User → Post → Comment → Reaction
     - `test_derive_linked_multiple_paths`: Multiple variants in one enum
     - `test_derive_linked_self_referential`: Self-referential chains

5. **`lifeguard-derive/tests/ui/compile_error_linked_*.rs`** (NEW):
   - UI tests for compile error cases:
     - `compile_error_linked_invalid_path`: Single hop validation
     - `compile_error_linked_empty_path`: Empty path validation
     - `compile_error_linked_invalid_entity_path`: Invalid entity path syntax

6. **`lifeguard-derive/DERIVE_LINKED_USAGE.md`** (NEW):
   - Comprehensive usage guide with examples
   - Error cases and error messages
   - Migration guide from manual implementations
   - Best practices

7. **`lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md`**:
   - Updated `find_linked` entry: ✅ Implemented with DeriveLinked macro
   - Added `DeriveLinked` to derive macros section: ✅ Complete
   - Added `DeriveLinked` to relations section: ✅ Completed
   - Noted competitive advantage: SeaORM doesn't have this feature

## Benefits

1. **Massive boilerplate reduction** - 80%+ reduction in code (from ~20 lines to ~4 lines per linked relationship)
2. **Type-safe** - Compile-time validation of relationship paths
3. **Discoverable** - Enum variants serve as documentation of available linked paths
4. **Competitive advantage** - SeaORM doesn't have this feature
5. **Easy migration** - Simple migration path from manual `Linked` implementations

## Example Usage

### Before (Manual Implementation)

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

**Lines of code:** ~20 lines

### After (With DeriveLinked)

```rust
#[derive(DeriveLinked)]
pub enum LinkedRelation {
    #[lifeguard(linked = "PostEntity -> CommentEntity")]
    Comments,
    
    #[lifeguard(linked = "PostEntity -> TagEntity")]
    Tags,
}
```

**Lines of code:** ~4 lines

**Boilerplate reduction:** ~80%

### Usage with find_linked()

```rust
let user: UserModel = ...;
let executor: &dyn LifeExecutor = ...;

// Find comments through posts
let comments: Vec<CommentModel> = user
    .find_linked::<PostEntity, CommentEntity>()
    .all(executor)?;
```

## Features

### Supported Path Syntax

1. **Two-hop paths**: `#[lifeguard(linked = "PostEntity -> CommentEntity")]`
2. **Three-hop paths**: `#[lifeguard(linked = "PostEntity -> CommentEntity -> ReactionEntity")]`
3. **Arbitrary-length paths**: Supports any number of hops
4. **Self-referential**: `#[lifeguard(linked = "Entity -> Entity")]`
5. **Module-qualified paths**: `#[lifeguard(linked = "super::posts::PostEntity -> CommentEntity")]`
6. **Multiple paths**: Multiple variants in one enum

### Error Handling

The macro provides clear, actionable error messages:

- **Invalid path syntax**: `Linked path must have at least 2 hops (intermediate and target), found 1`
- **Empty path**: `Linked path cannot be empty. Use format: Entity1 -> Entity2`
- **Invalid entity path**: `Invalid entity path in hop 1 'Post-Entity': unexpected token`
- **Missing Related impl**: Rust compiler reports trait bound errors when `Related` implementations are missing

## Testing

- ✅ **4 unit tests** passing (two-hop, three-hop, multiple paths, self-referential)
- ✅ **3 UI tests** passing (compile error cases)
- ✅ **All doctests** passing
- ✅ **Error messages** verified to be clear and actionable

## Implementation Phases

### Phase 1: Core Infrastructure ✅
- Basic two-hop path parsing and code generation
- Enum parsing and variant processing
- Code generation for `Linked<I, T>` implementations

### Phase 2: Validation & Error Handling ✅
- UI tests for compile errors
- Self-referential path support
- Enhanced error messages

### Phase 3: Multi-hop Support ✅
- Three-hop and arbitrary-length paths
- Module-qualified paths

### Phase 5: Documentation ✅
- Comprehensive usage guide
- Updated mapping documentation
- Examples and best practices

## Related Issues

Completes the enhancement tracked in `DERIVE_LINKED_DISCOVERY.md`:
- `DeriveLinked` macro for generating `Linked<I, T>` implementations ✅ **Completed**

## Breaking Changes

None - This is a purely additive feature.

## Competitive Advantage

**SeaORM doesn't have this feature.** Users must manually implement `Linked<I, T>` for each multi-hop relationship, which is verbose and error-prone. The `DeriveLinked` macro provides:

- **80%+ boilerplate reduction**
- **Compile-time safety**
- **Discoverability** through enum variants
- **Easy migration** from manual implementations

This makes Lifeguard more developer-friendly than SeaORM for multi-hop relationship queries.
