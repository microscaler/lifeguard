# Registry Pattern Audit

## Overview

This document tracks registry-based implementations in the Lifeguard codebase. Registry patterns (using static HashMaps with TypeId keys) are considered an anti-pattern because they:
- Require runtime lookups instead of compile-time type information
- Add complexity with initialization order concerns
- Are harder to test and reason about
- Can be replaced with simpler struct-based patterns (like SeaORM's `RelationDef`)

**Status:** ✅ No registry-based implementations found in current codebase  
**Last Updated:** 2025-01-27

---

## Anti-Pattern Definition

A **registry pattern** in this context is:
- A static `HashMap` or similar collection keyed by `TypeId` or type identifiers
- Used to store metadata that could be known at compile time
- Requires runtime lookup to access metadata
- Often initialized via macro-generated code or lazy initialization

**Why it's an anti-pattern:**
1. **Runtime overhead**: HashMap lookups vs compile-time type information
2. **Initialization complexity**: Requires careful ordering, lazy initialization, or constructor functions
3. **Type safety**: Loses compile-time guarantees that Rust provides
4. **Testing complexity**: Harder to test due to global state
5. **Better alternatives**: Struct-based patterns (like `RelationDef`) store metadata directly in types

---

## Audit Results

### ✅ No Registry Implementations Found

After comprehensive search of the codebase, **no registry-based implementations were found**.

**Searched for:**
- `HashMap` with `TypeId` keys
- Static `Lazy<HashMap>` patterns
- `TypeId::of` usage in registry contexts
- Macro-generated registry initialization

**Files Checked:**
- `src/**/*.rs` - All source files
- `lifeguard-derive/src/**/*.rs` - All macro source files
- `tests/**/*.rs` - All test files

---

## False Positives (Not Registries)

### 1. `src/metrics.rs` - Metrics Singleton

**Pattern:** `Lazy<LifeguardMetrics>`

```rust
pub static METRICS: Lazy<LifeguardMetrics> = Lazy::new(LifeguardMetrics::init);
```

**Analysis:** ✅ **Not a registry** - This is a legitimate singleton pattern for metrics collection. It's a single instance, not a lookup table. This is the correct use of `Lazy` for lazy initialization of a singleton.

**Status:** ✅ Keep as-is

---

### 2. `src/query/column.rs` - Type Mapping Function

**Pattern:** `from_rust_type()` with match statement

```rust
pub fn from_rust_type(rust_type: &str, ...) -> ColumnDefinition {
    let column_type = match inner_type {
        "i32" => Some("Integer".to_string()),
        "i64" => Some("BigInt".to_string()),
        // ...
    };
}
```

**Analysis:** ✅ **Not a registry** - This is a simple function with pattern matching. It maps string types to SQL types, but doesn't use a HashMap or TypeId lookup. This is fine and efficient.

**Status:** ✅ Keep as-is

---

## Design Document References

### Registry Patterns Mentioned (But Not Implemented)

The following design documents mention registry patterns as potential solutions, but these have been **rejected in favor of better approaches**:

#### 1. RelationMetadata Registry (Rejected)

**Location:** `lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md` §13  
**Status:** ❌ Rejected - Using `RelationDef` pattern instead

**Original Proposal:**
```rust
static RELATION_METADATA: Lazy<HashMap<(TypeId, TypeId), &'static str>> = Lazy::new(|| {
    // Populated by macro-generated code
});
```

**Replacement:** `RelationDef` struct pattern (see `DESIGN_RELATION_METADATA_AND_COMPOSITE_KEYS.md`)

**Why Rejected:**
- SeaORM uses `RelationDef` struct instead of registry
- No runtime lookup needed - metadata is in the struct
- Type-safe and compile-time verified
- Simpler and more maintainable

---

## Prevention Guidelines

To prevent registry anti-patterns from being introduced:

### ✅ Preferred Patterns

1. **Struct-Based Metadata** (Like `RelationDef`)
   ```rust
   pub struct RelationDef {
       pub from_col: Identity,
       pub to_col: Identity,
       // ... metadata stored directly
   }
   ```

2. **Trait-Associated Types**
   ```rust
   trait Related<R> {
       fn to() -> RelationDef;  // Returns struct, not lookup
   }
   ```

3. **Macro-Generated Code**
   - Generate structs with metadata
   - Generate trait implementations
   - Avoid generating registry initialization

4. **Singleton Pattern** (For legitimate use cases)
   ```rust
   static METRICS: Lazy<Metrics> = Lazy::new(Metrics::new);
   ```

### ❌ Avoid These Patterns

1. **TypeId-Based Lookups**
   ```rust
   // ❌ Anti-pattern
   static METADATA: Lazy<HashMap<TypeId, Metadata>> = Lazy::new(|| {
       // ...
   });
   ```

2. **Macro-Generated Registry Initialization**
   ```rust
   // ❌ Anti-pattern
   #[ctor::ctor]
   fn init_registry() {
       METADATA.insert(TypeId::of::<T>(), value);
   }
   ```

3. **Runtime Metadata Lookup**
   ```rust
   // ❌ Anti-pattern
   fn get_metadata<T>() -> Option<&Metadata> {
       METADATA.get(&TypeId::of::<T>())
   }
   ```

---

## Future Considerations

### When Registry Might Be Acceptable

Registry patterns are acceptable only when:
1. **Dynamic registration is required** - Types are not known at compile time
2. **Plugin system** - External code needs to register types
3. **Runtime discovery** - Types are discovered at runtime (e.g., from database schema)

**Current Status:** None of these cases apply to Lifeguard's current design.

### If Registry Is Needed

If a registry pattern becomes necessary in the future:

1. **Document the justification** - Why compile-time patterns won't work
2. **Use proper initialization** - Consider `ctor` crate or explicit initialization
3. **Add tests** - Ensure initialization order is correct
4. **Consider alternatives** - Re-evaluate if struct-based patterns could work
5. **Update this audit** - Document the registry and its justification

---

## Related Documents

- **Design Document:** `lifeguard-derive/DESIGN_RELATION_METADATA_AND_COMPOSITE_KEYS.md`
  - Documents the decision to use `RelationDef` instead of registry
  - Explains SeaORM's approach

- **Mapping Document:** `lifeguard-derive/SEAORM_LIFEGUARD_MAPPING.md`
  - Contains historical references to registry patterns (now rejected)

---

## Maintenance

This audit should be updated:
- When new registry patterns are proposed
- When registry patterns are implemented (with justification)
- When registry patterns are removed or replaced
- During code reviews - check for registry anti-patterns

**Review Frequency:** Quarterly or when major architectural changes are made

---

**Audit Status:** ✅ Clean - No registry anti-patterns found  
**Next Review:** 2025-04-27
