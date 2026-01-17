# Design Document: RelationMetadata and Composite Primary Key Support

## Overview

This document outlines the design and implementation plan for:
1. **RelationMetadata Usage**: Enabling `find_related()` to use relationship metadata without trait bounds
2. **Composite Primary Key Support**: Full support for composite primary keys in `find_related()`

**Status:** 🟡 Design Phase - Awaiting Review  
**Related Sections:** [SEAORM_LIFEGUARD_MAPPING.md §13 - Implementation Notes](./SEAORM_LIFEGUARD_MAPPING.md#13-implementation-notes)

---

## 1. Current State Analysis

### 1.1 RelationMetadata Trait

**Current Implementation:**
- ✅ `RelationMetadata` trait defined in `src/relation.rs`
- ✅ `DeriveRelation` macro generates `RelationMetadata` implementations when `from`/`to` columns are provided
- ❌ Not used in `find_related()` due to trait bound limitations

**Location:** `src/relation.rs:242-260`

**Generated Code Example:**
```rust
impl RelationMetadata<Entity> for super::users::Entity {
    fn foreign_key_column() -> Option<&'static str> {
        Some("user_id")
    }
}
```

### 1.2 Composite Primary Keys

**Current Implementation:**
- ✅ `PrimaryKeyArityTrait` exists and works
- ✅ Macro generates composite key support in `PrimaryKeyTrait::ValueType` (tuples)
- ❌ `ModelTrait::get_primary_key_value()` only returns first key value
- ❌ `find_related()` only supports single-column primary keys

**Location:** 
- `src/query/primary_key.rs:177-208` (PrimaryKeyArityTrait)
- `src/model.rs:104` (get_primary_key_value)
- `src/relation.rs:378-415` (find_related)

### 1.3 Derive Macros

**Current Capabilities:**
- ✅ `LifeModel` macro generates Model, Column, PrimaryKey enums
- ✅ `DeriveRelation` macro generates Related and RelationMetadata implementations
- ✅ Can parse attributes and generate code based on metadata

**Location:** `lifeguard-derive/src/macros/`

---

## 2. Problem Statement

### 2.1 RelationMetadata Usage Problem

**Issue:** Cannot use `RelationMetadata` in `find_related()` without making it a required trait bound.

**Root Cause:**
- Rust trait system requires explicit trait bounds to call trait methods
- Adding `R: RelationMetadata<Self::Entity>` would break existing code
- No way to conditionally use a trait method

**Impact:**
- Relationship metadata is generated but unused
- Users must manually specify foreign key columns or rely on defaults
- Custom foreign key column names are not supported

### 2.2 Composite Primary Key Problem

**Issue:** `find_related()` cannot handle composite primary keys (multiple columns).

**Root Cause:**
1. `get_primary_key_value()` returns single `Value`, not collection
2. Cannot enumerate `PrimaryKey` enum variants at runtime
3. No way to map multiple primary key columns to foreign key columns

**Impact:**
- Entities with composite primary keys cannot use `find_related()`
- Must manually construct queries for composite key relationships
- Limits ORM functionality for complex schemas

---

## 3. Proposed Solution: RelationDef Pattern (SeaORM Approach)

### 3.1 Architecture Overview

**Key Insight from SeaORM:** SeaORM doesn't use a registry. Instead, they use a `RelationDef` struct that contains all relationship metadata. The `Related::to()` method returns this struct, which can then be converted to a SeaQuery `Condition`.

**SeaORM's Approach:**
1. **`RelationDef` struct**: Contains all metadata (from_col, to_col, from_tbl, to_tbl, etc.)
2. **`Related::to()` returns `RelationDef`**: Not a query, but a metadata struct
3. **`RelationDef` implements `From<RelationDef> for Condition`**: Can be converted to SeaQuery Condition
4. **`Identity` enum**: Handles both single and composite keys (Unary, Binary, Ternary, Many variants)
5. **`join_tbl_on_condition` function**: Takes `Identity` and builds join conditions for SeaQuery

**Why This is Better:**
- ✅ No trait bounds required - `RelationDef` is just a struct
- ✅ No runtime registry lookup - metadata is in the struct
- ✅ Supports composite keys natively via `Identity` enum
- ✅ Type-safe - all metadata is known at compile time
- ✅ Can be used directly with SeaQuery

**Key Components for Lifeguard:**
1. **`RelationDef` struct**: Similar to SeaORM's, stores relationship metadata
2. **`Identity` enum**: For single and composite column references (Unary, Binary, Ternary, Many)
3. **`Related::to()` returns `RelationDef`**: Instead of `SelectQuery` (breaking change, but better design)
4. **Conversion to Condition**: `RelationDef` → SeaQuery `Condition` for WHERE clauses
5. **Enhanced ModelTrait**: Support for getting all primary key values as `Identity`
6. **`join_tbl_on_condition` function**: Helper to build join conditions from `Identity` pairs

**SeaORM's `RelationDef` Structure:**
```rust
pub struct RelationDef {
    pub rel_type: RelationType,      // HasOne, HasMany
    pub from_tbl: TableRef,          // Source table
    pub to_tbl: TableRef,            // Target table
    pub from_col: Identity,          // Foreign key column(s) - supports composite!
    pub to_col: Identity,            // Primary key column(s) - supports composite!
    pub is_owner: bool,
    pub skip_fk: bool,
    pub on_delete: Option<ForeignKeyAction>,
    pub on_update: Option<ForeignKeyAction>,
    pub on_condition: Option<Arc<dyn Fn(DynIden, DynIden) -> Condition>>,
    pub fk_name: Option<String>,
    pub condition_type: ConditionType,
}
```

**SeaORM's `Identity` Enum (Handles Composite Keys):**
```rust
pub enum Identity {
    Unary(DynIden),                    // Single column
    Binary(DynIden, DynIden),          // 2 columns
    Ternary(DynIden, DynIden, DynIden), // 3 columns
    Many(Vec<DynIden>),                // 4+ columns
}
```

**How SeaORM Uses It:**
1. Macro generates `RelationDef` with all metadata in `Related::to()`
2. `RelationDef` implements `From<RelationDef> for Condition`
3. `join_tbl_on_condition()` takes `Identity` pairs and builds join conditions
4. Works for both single and composite keys automatically

### 3.2 Solution 1: RelationDef Pattern (Following SeaORM)

#### 3.2.1 Design

**Key Change:** `Related::to()` should return `RelationDef` instead of `SelectQuery<R>`.

**RelationDef Structure:**
```rust
pub struct RelationDef {
    pub rel_type: RelationType,      // HasOne, HasMany, BelongsTo
    pub from_tbl: TableRef,          // Source table
    pub to_tbl: TableRef,            // Target table
    pub from_col: Identity,          // Foreign key column(s) - supports composite!
    pub to_col: Identity,            // Primary key column(s) - supports composite!
    pub is_owner: bool,
    pub skip_fk: bool,
    pub on_delete: Option<ForeignKeyAction>,
    pub on_update: Option<ForeignKeyAction>,
    pub on_condition: Option<Arc<dyn Fn(DynIden, DynIden) -> Condition>>,
    pub fk_name: Option<String>,
    pub condition_type: ConditionType,
}
```

**Identity Enum (for composite keys):**
```rust
pub enum Identity {
    Unary(DynIden),                    // Single column
    Binary(DynIden, DynIden),          // 2 columns
    Ternary(DynIden, DynIden, DynIden), // 3 columns
    Many(Vec<DynIden>),                // 4+ columns
}
```

**Updated Related Trait:**
```rust
pub trait Related<R>
where
    Self: LifeModelTrait,
    R: LifeModelTrait,
{
    /// Returns RelationDef with all relationship metadata
    fn to() -> RelationDef;
}
```

**Generated Code Example:**
```rust
// Generated by DeriveRelation macro
impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        RelationDef {
            rel_type: RelationType::BelongsTo,
            from_tbl: TableRef::Table(Entity::table_name().into()),
            to_tbl: TableRef::Table(super::users::Entity::table_name().into()),
            from_col: Identity::Unary(Column::UserId.into_iden()),
            to_col: Identity::Unary(super::users::Column::Id.into_iden()),
            is_owner: true,
            skip_fk: false,
            on_delete: None,
            on_update: None,
            on_condition: None,
            fk_name: None,
            condition_type: ConditionType::All,
        }
    }
}
```

**Usage in find_related():**
```rust
fn find_related<R>(&self) -> SelectQuery<R>
where
    R: LifeModelTrait + Related<Self::Entity>,
{
    let mut query = SelectQuery::new();
    let rel_def = R::to();
    
    // Get primary key as Identity
    let pk_identity = self.get_primary_key_identity();
    
    // Build WHERE condition from RelationDef
    let condition = rel_def.to_condition(pk_identity);
    query = query.filter(condition);
    
    query
}
```

#### 3.2.2 Implementation Plan

**Step 1: Create Identity Enum**
- **File:** `src/relation/identity.rs` (new)
- **Contents:**
  - `Identity` enum (Unary, Binary, Ternary, Many)
  - Helper methods (arity, iter, contains)
  - Conversion from Column enum variants

**Step 2: Create RelationDef Struct**
- **File:** `src/relation/def.rs` (new)
- **Contents:**
  - `RelationDef` struct definition
  - `RelationType` enum
  - `From<RelationDef> for Condition` implementation
  - `to_condition()` method for WHERE clauses

**Step 3: Update Related Trait**
- **File:** `src/relation.rs`
- **Changes:**
  - Change `Related::to()` return type from `SelectQuery<R>` to `RelationDef`
  - This is a **breaking change** but necessary for proper design

**Step 4: Update DeriveRelation Macro**
- **File:** `lifeguard-derive/src/macros/relation.rs`
- **Changes:**
  - Generate `RelationDef` construction instead of `SelectQuery`
  - Handle `from`/`to` attributes to build `Identity` for composite keys
  - Support both single and composite foreign keys

**Step 5: Update find_related()**
- **File:** `src/relation.rs`
- **Changes:**
  - Use `R::to()` to get `RelationDef`
  - Convert `RelationDef` to `Condition` for WHERE clause
  - Works automatically for both single and composite keys

**Step 6: Add Helper Functions**
- **File:** `src/relation/def.rs`
- **Contents:**
  - `join_tbl_on_condition()` - Build join conditions from Identity pairs
  - `to_condition()` - Convert RelationDef to WHERE condition

**Step 7: Add Tests**
- **File:** `tests/integration/relation_def.rs` (new)
- **Tests:**
  - Single key relationships
  - Composite key relationships
  - Custom foreign key column names
  - Default fallback behavior

#### 3.2.3 Dependencies

**New Dependencies:**
- None (uses existing SeaQuery types)

**Existing Dependencies:**
- `lifeguard-derive` (for macro generation)
- `sea-query` (for Condition, DynIden, etc.)

### 3.3 Solution 2: Composite Primary Key Support

#### 3.3.1 Design

**Key Insight:** With `RelationDef` and `Identity`, composite key support is **automatic**. The `Identity` enum handles both single and composite keys, and `RelationDef` stores the column mappings.

**Enhanced ModelTrait:**
```rust
trait ModelTrait {
    // Existing method (single key) - kept for backward compatibility
    fn get_primary_key_value(&self) -> Value;
    
    // New method: Get primary key as Identity (supports composite keys)
    fn get_primary_key_identity(&self) -> Identity;
}
```

**Macro-Generated Implementation:**
```rust
// For single key
impl ModelTrait for UserModel {
    fn get_primary_key_value(&self) -> Value {
        self.id.into()
    }
    
    fn get_primary_key_identity(&self) -> Identity {
        Identity::Unary(Column::Id.into_iden())
    }
}

// For composite key (id, tenant_id)
impl ModelTrait for TenantUserModel {
    fn get_primary_key_value(&self) -> Value {
        self.id.into()  // Returns first key (backward compatible)
    }
    
    fn get_primary_key_identity(&self) -> Identity {
        Identity::Binary(
            Column::Id.into_iden(),
            Column::TenantId.into_iden()
        )
    }
}
```

**Usage in find_related() (with RelationDef):**
```rust
fn find_related<R>(&self) -> SelectQuery<R>
where
    R: LifeModelTrait + Related<Self::Entity>,
{
    let mut query = SelectQuery::new();
    let rel_def = R::to();
    
    // Get primary key as Identity (works for both single and composite)
    let pk_identity = self.get_primary_key_identity();
    
    // Build WHERE condition: related_table.from_col = pk_values
    // RelationDef already has from_col (foreign key) and to_col (primary key)
    // We just need to match from_col to pk_identity
    let condition = build_where_condition(&rel_def, &pk_identity);
    query = query.filter(condition);
    
    query
}

fn build_where_condition(rel_def: &RelationDef, pk_identity: &Identity) -> Condition {
    // Match each column in from_col to corresponding value in pk_identity
    // This works automatically for both single and composite keys!
    let mut condition = Condition::all();
    
    for (fk_col, pk_val) in rel_def.from_col.iter().zip(pk_identity.iter()) {
        let expr = Expr::col((rel_def.to_tbl, fk_col.clone()))
            .eq(Expr::val(pk_val));  // pk_val comes from model
        condition = condition.add(expr);
    }
    
    condition
}
```

**Note:** The actual implementation needs to extract values from the model, not just column names. This requires a helper to convert `Identity` (column references) to actual `Value`s from the model.

#### 3.3.2 Implementation Plan

**Step 1: Create Identity Enum**
- **File:** `src/relation/identity.rs` (new)
- **Contents:**
  - `Identity` enum (Unary, Binary, Ternary, Many)
  - Helper methods (arity, iter, contains)
  - Conversion from Column enum variants to `DynIden`

**Step 2: Enhance ModelTrait**
- **File:** `src/model.rs`
- **Changes:**
  - Add `get_primary_key_identity()` method returning `Identity`
  - Keep `get_primary_key_value()` for backward compatibility
  - Add helper method to get primary key values as `Vec<Value>` from `Identity`

**Step 3: Update LifeModel Macro**
- **File:** `lifeguard-derive/src/macros/life_model.rs`
- **Changes:**
  - Generate `get_primary_key_identity()` implementation
  - For single keys: return `Identity::Unary(Column::Id.into_iden())`
  - For composite keys: return `Identity::Binary/Ternary/Many(...)`
  - Use existing primary key tracking logic

**Step 4: Create RelationDef Struct**
- **File:** `src/relation/def.rs` (new)
- **Contents:**
  - `RelationDef` struct with all metadata fields
  - `RelationType` enum (HasOne, HasMany, BelongsTo)
  - `From<RelationDef> for Condition` for JOINs
  - `to_where_condition()` method for WHERE clauses (needs model values)

**Step 5: Update DeriveRelation Macro**
- **File:** `lifeguard-derive/src/macros/relation.rs`
- **Changes:**
  - Generate `RelationDef` construction in `Related::to()`
  - Handle `from`/`to` attributes to build `Identity` for composite keys
  - Support both single and composite foreign keys
  - Convert Column enum variants to `Identity`

**Step 6: Update find_related()**
- **File:** `src/relation.rs`
- **Changes:**
  - Use `R::to()` to get `RelationDef`
  - Get primary key as `Identity` from model
  - Build WHERE condition matching `from_col` to primary key values
  - Works automatically for both single and composite keys

**Step 7: Add Helper Functions**
- **File:** `src/relation/def.rs`
- **Contents:**
  - `join_tbl_on_condition()` - Build join conditions from Identity pairs
  - `to_where_condition()` - Convert RelationDef + model values to WHERE condition

**Step 8: Add Tests**
- **File:** `tests/integration/composite_key_relations.rs` (new)
- **Tests:**
  - Single key relationships (backward compatibility)
  - Composite key relationships (2, 3, 4+ columns)
  - Multiple foreign key columns
  - Edge cases (mismatched key sizes, etc.)

#### 3.3.3 Dependencies

**New Dependencies:**
- None (uses existing SeaQuery types)

**Existing Dependencies:**
- `lifeguard-derive` (for macro generation)
- `sea-query` (for Condition, DynIden, etc.)

---

## 4. Implementation Details

### 4.1 Metadata Registry Module Structure

**File:** `src/relation/metadata.rs`

```rust
use std::any::TypeId;
use std::collections::HashMap;
use once_cell::sync::Lazy;

// Single foreign key column registry
type ForeignKeyRegistry = HashMap<(TypeId, TypeId), &'static str>;

// Composite foreign key columns registry
type CompositeForeignKeyRegistry = HashMap<(TypeId, TypeId), Vec<&'static str>>;

// Global registries (populated by macro-generated code)
static FOREIGN_KEY_METADATA: Lazy<ForeignKeyRegistry> = Lazy::new(|| {
    let mut map = HashMap::new();
    // Macro will generate registration calls here
    map
});

static COMPOSITE_FOREIGN_KEY_METADATA: Lazy<CompositeForeignKeyRegistry> = Lazy::new(|| {
    let mut map = HashMap::new();
    // Macro will generate registration calls here
    map
});

// Public API
pub fn get_foreign_key_column<R, E>() -> Option<&'static str>
where
    R: 'static,
    E: 'static,
{
    FOREIGN_KEY_METADATA.get(&(TypeId::of::<R>(), TypeId::of::<E>())).copied()
}

pub fn get_composite_foreign_key_columns<R, E>() -> Option<Vec<&'static str>>
where
    R: 'static,
    E: 'static,
{
    COMPOSITE_FOREIGN_KEY_METADATA.get(&(TypeId::of::<R>(), TypeId::of::<E>())).cloned()
}

// Registration functions (called by macro-generated code)
pub fn register_foreign_key<R, E>(column: &'static str)
where
    R: 'static,
    E: 'static,
{
    FOREIGN_KEY_METADATA.insert((TypeId::of::<R>(), TypeId::of::<E>()), column);
}

pub fn register_composite_foreign_keys<R, E>(columns: Vec<&'static str>)
where
    R: 'static,
    E: 'static,
{
    COMPOSITE_FOREIGN_KEY_METADATA.insert((TypeId::of::<R>(), TypeId::of::<E>()), columns);
}
```

**Note:** The registration functions need to be called at initialization time. We'll need to use a different approach - see Section 4.2.

### 4.2 Registration Strategy

**Problem:** Static `Lazy` collections can't be mutated after initialization.

**Solution Options:**

**Option A: Initialize-once Pattern**
```rust
static FOREIGN_KEY_METADATA: Lazy<Mutex<ForeignKeyRegistry>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

pub fn register_foreign_key<R, E>(column: &'static str) {
    FOREIGN_KEY_METADATA.lock().unwrap().insert(...);
}
```
**Pros:** Simple, works  
**Cons:** Runtime mutex overhead, not thread-safe for concurrent registration

**Option B: Macro-Generated Initialization Function**
```rust
// Generated by macro
#[ctor::ctor]
fn init_relation_metadata() {
    register_foreign_key::<PostEntity, UserEntity>("user_id");
}

// Registry is populated before main()
static FOREIGN_KEY_METADATA: Lazy<ForeignKeyRegistry> = Lazy::new(|| {
    let mut map = HashMap::new();
    // Macro generates this initialization
    init_relation_metadata();
    map
});
```
**Pros:** No runtime overhead, thread-safe  
**Cons:** Requires `ctor` crate, initialization order issues

**Option C: Build-time Registration (Recommended)**
```rust
// Macro generates a function that returns the map
#[macro_export]
macro_rules! register_relation_metadata {
    () => {{
        let mut map = HashMap::new();
        map.insert((TypeId::of::<PostEntity>(), TypeId::of::<UserEntity>()), "user_id");
        map
    }};
}

// User's code (generated by macro)
static FOREIGN_KEY_METADATA: Lazy<ForeignKeyRegistry> = Lazy::new(|| {
    register_relation_metadata!()
});
```
**Pros:** No runtime overhead, compile-time initialization  
**Cons:** Requires macro to generate code in user's crate

**Recommended:** Option C - Generate static initialization in user's code via macro.

### 4.3 Macro Generation Changes

#### 4.3.1 DeriveRelation Macro Updates

**Current:** Generates trait implementations  
**New:** Generates trait implementations + registry initialization

**Generated Code Pattern:**
```rust
// Existing: Related trait impl
impl Related<super::users::Entity> for Entity {
    fn to() -> SelectQuery<super::users::Entity> {
        SelectQuery::new()
    }
}

// New: Registry initialization (if from/to provided)
#[doc(hidden)]
#[allow(non_upper_case_globals)]
static _RELATION_METADATA_INIT: once_cell::sync::Lazy<()> = once_cell::sync::Lazy::new(|| {
    lifeguard::relation::metadata::register_foreign_key::<super::users::Entity, Entity>("user_id");
});
```

**Location:** `lifeguard-derive/src/macros/relation.rs:130-150`

#### 4.3.2 LifeModel Macro Updates

**Current:** Generates `get_primary_key_value()`  
**New:** Also generates `get_all_primary_key_values()`

**Generated Code Pattern:**
```rust
// Single key
impl ModelTrait for UserModel {
    fn get_primary_key_value(&self) -> Value {
        self.id.into()
    }
    
    fn get_all_primary_key_values(&self) -> Vec<Value> {
        vec![self.id.into()]
    }
}

// Composite key
impl ModelTrait for TenantUserModel {
    fn get_primary_key_value(&self) -> Value {
        self.id.into()  // First key (backward compatible)
    }
    
    fn get_all_primary_key_values(&self) -> Vec<Value> {
        vec![self.id.into(), self.tenant_id.into()]
    }
}
```

**Location:** `lifeguard-derive/src/macros/life_model.rs:1045-1075`

### 4.4 find_related() Updates

**Current Implementation:**
```rust
fn find_related<R>(&self) -> SelectQuery<R> {
    let mut query = R::to();
    let pk_value = self.get_primary_key_value();
    // ... single key logic
}
```

**New Implementation:**
```rust
fn find_related<R>(&self) -> SelectQuery<R> {
    let mut query = R::to();
    let pk_values = self.get_all_primary_key_values();
    let arity = <Self::Entity as PrimaryKeyArityTrait>::arity();
    
    let related_table = R::default().table_name();
    
    match arity {
        PrimaryKeyArity::Single => {
            // Single key: existing logic with registry lookup
            let fk_column = metadata::get_foreign_key_column::<R, Self::Entity>()
                .unwrap_or_else(|| format!("{}_id", current_table));
            query = query.filter(Expr::column(format!("{}.{}", related_table, fk_column)).eq(pk_values[0]));
        }
        _ => {
            // Composite key: multiple WHERE conditions
            let fk_columns = metadata::get_composite_foreign_key_columns::<R, Self::Entity>()
                .unwrap_or_else(|| {
                    // Default: generate column names from primary key
                    generate_default_fk_columns(current_table, arity)
                });
            
            for (pk_val, fk_col) in pk_values.iter().zip(fk_columns.iter()) {
                query = query.filter(Expr::column(format!("{}.{}", related_table, fk_col)).eq(pk_val));
            }
        }
    }
    
    query
}
```

**Location:** `src/relation.rs:378-415`

---

## 5. API Changes

### 5.1 Breaking Changes

**None** - All changes are additive:
- `get_all_primary_key_values()` is a new method with default implementation
- Registry lookup is internal implementation detail
- Existing code continues to work

### 5.2 New Public APIs

**New Module:** `lifeguard::relation::metadata`
- `get_foreign_key_column<R, E>() -> Option<&'static str>`
- `get_composite_foreign_key_columns<R, E>() -> Option<Vec<&'static str>>`

**Enhanced Trait:** `ModelTrait`
- `get_all_primary_key_values(&self) -> Vec<Value>` (new method)

### 5.3 DeriveRelation Macro Changes

**New Attribute Support:**
```rust
#[lifeguard(
    belongs_to = "super::users::Entity",
    from = "Column::UserId",           // Single foreign key
    to = "super::users::Column::Id"
)]
User,

// For composite keys (future):
#[lifeguard(
    belongs_to = "super::tenants::Entity",
    from = ["Column::UserId", "Column::TenantId"],  // Multiple foreign keys
    to = ["super::tenants::Column::Id", "super::tenants::Column::TenantId"]
)]
Tenant,
```

---

## 6. Testing Strategy

### 6.1 Unit Tests

**File:** `src/relation/metadata.rs` (tests module)
- Registry registration and lookup
- TypeId-based key matching
- Default fallback behavior

### 6.2 Integration Tests

**File:** `tests/integration/relation_metadata.rs`
- Custom foreign key column names
- Single key relationships
- Default naming fallback

**File:** `tests/integration/composite_key_relations.rs`
- Composite primary key relationships
- Multiple foreign key columns
- Single key backward compatibility

### 6.3 Macro Tests

**File:** `lifeguard-derive/tests/test_derive_relation.rs`
- Registry initialization code generation
- Foreign key column name extraction
- Composite key metadata generation

---

## 7. Migration Path

### 7.1 For Existing Code

**No changes required** - all enhancements are backward compatible:
- Existing `find_related()` calls continue to work
- Default foreign key naming still works
- Single primary keys unchanged

### 7.2 For New Code

**Optional:** Use `from`/`to` attributes in `DeriveRelation` to specify custom foreign key columns:
```rust
#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",  // Custom foreign key
        to = "super::users::Column::Id"
    )]
    User,
}
```

---

## 8. Dependencies

### 8.1 New Dependencies

**Required:**
- `once_cell` (already in use) or `lazy_static` - for static initialization
- `std::any::TypeId` - standard library, no new dependency

**Optional (for initialization):**
- `ctor` - if using constructor-based initialization (Option B)

### 8.2 Existing Dependencies

- `lifeguard-derive` - for macro generation
- `sea-query` - for query building
- All existing dependencies remain

---

## 9. Implementation Checklist

### Phase 1: Metadata Registry Infrastructure
- [ ] Create `src/relation/metadata.rs` module
- [ ] Define registry types and lookup functions
- [ ] Add unit tests for registry
- [ ] Document public API

### Phase 2: RelationMetadata Registry Integration
- [ ] Update `DeriveRelation` macro to generate registry initialization
- [ ] Update `find_related()` to use registry lookup
- [ ] Add integration tests for custom foreign key columns
- [ ] Verify backward compatibility

### Phase 3: Composite Primary Key Support
- [ ] Add `get_all_primary_key_values()` to `ModelTrait`
- [ ] Update `LifeModel` macro to generate composite key support
- [ ] Create composite key metadata registry
- [ ] Update `DeriveRelation` to support composite key metadata
- [ ] Update `find_related()` to handle composite keys
- [ ] Add comprehensive tests

### Phase 4: Documentation and Polish
- [ ] Update `DERIVE_RELATION_USAGE.md` with examples
- [ ] Update `SEAORM_LIFEGUARD_MAPPING.md` implementation notes
- [ ] Add examples to documentation
- [ ] Performance testing and optimization

---

## 10. Risks and Mitigations

### 10.1 TypeId Collisions

**Risk:** Different types with same name could collide  
**Mitigation:** Use full module path in TypeId (already handled by Rust)

### 10.2 Initialization Order

**Risk:** Registry accessed before initialization  
**Mitigation:** Use `Lazy` static initialization (guaranteed before first access)

### 10.3 Performance

**Risk:** HashMap lookup overhead  
**Mitigation:** 
- TypeId comparison is fast (pointer comparison)
- Lookup is O(1) average case
- Only called once per `find_related()` call
- Benchmark and optimize if needed

### 10.4 Backward Compatibility

**Risk:** Breaking existing code  
**Mitigation:** 
- All changes are additive
- Default behavior preserved
- Comprehensive testing before release

---

## 11. Success Criteria

### 11.1 RelationMetadata Usage
- ✅ `find_related()` uses custom foreign key columns when specified
- ✅ Default naming still works when metadata not provided
- ✅ No trait bound requirements
- ✅ Zero breaking changes

### 11.2 Composite Primary Key Support
- ✅ `find_related()` works with composite primary keys
- ✅ Multiple WHERE conditions generated correctly
- ✅ Single key support unchanged
- ✅ Comprehensive test coverage

---

## 12. References

- **Implementation Notes:** [SEAORM_LIFEGUARD_MAPPING.md §13](./SEAORM_LIFEGUARD_MAPPING.md#13-implementation-notes)
- **Related Trait:** [SEAORM_LIFEGUARD_MAPPING.md §1 - Related](./SEAORM_LIFEGUARD_MAPPING.md#1-core-traits--types)
- **DeriveRelation Macro:** [SEAORM_LIFEGUARD_MAPPING.md §2 - DeriveRelation](./SEAORM_LIFEGUARD_MAPPING.md#2-derive-macros)
- **Usage Guide:** [DERIVE_RELATION_USAGE.md](./DERIVE_RELATION_USAGE.md)

---

## 13. Open Questions

1. **Initialization Strategy:** Which approach (A, B, or C) should we use for registry initialization?
2. **Composite Key Syntax:** Should we support array syntax `from = [...]` or require separate attributes?
3. **Error Handling:** How should we handle mismatched key sizes (e.g., 2 primary keys but 1 foreign key)?
4. **Performance:** Should we add caching or other optimizations for registry lookups?

---

**Document Status:** 🟡 Awaiting Review  
**Last Updated:** 2025-01-27  
**Next Steps:** Review design, approve approach, begin Phase 1 implementation
