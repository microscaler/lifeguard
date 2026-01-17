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

## 4. Detailed Implementation Plan

### 4.1 Phase 1: Identity Enum Implementation

**Goal:** Create the `Identity` enum to represent single and composite column references.

**File:** `src/relation/identity.rs` (new file)

**Implementation Steps:**

1. **Define Identity Enum:**
```rust
use sea_query::{DynIden, Iden, IntoIden};
use std::borrow::Cow;

/// Represents a column identifier that can be single or composite
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Identity {
    /// Single column identifier
    Unary(DynIden),
    /// Two column identifiers (composite key)
    Binary(DynIden, DynIden),
    /// Three column identifiers (composite key)
    Ternary(DynIden, DynIden, DynIden),
    /// Four or more column identifiers (composite key)
    Many(Vec<DynIden>),
}
```

2. **Implement Helper Methods:**
```rust
impl Identity {
    /// Get the arity (number of columns) for this identity
    pub fn arity(&self) -> usize {
        match self {
            Self::Unary(_) => 1,
            Self::Binary(_, _) => 2,
            Self::Ternary(_, _, _) => 3,
            Self::Many(vec) => vec.len(),
        }
    }

    /// Iterate over column identifiers
    pub fn iter(&self) -> BorrowedIdentityIter<'_> {
        BorrowedIdentityIter { identity: self, index: 0 }
    }

    /// Check if this identity contains a specific column
    pub fn contains(&self, col: &DynIden) -> bool {
        self.iter().any(|c| c == col)
    }
}
```

3. **Implement Iterator:**
```rust
pub struct BorrowedIdentityIter<'a> {
    identity: &'a Identity,
    index: usize,
}

impl<'a> Iterator for BorrowedIdentityIter<'a> {
    type Item = &'a DynIden;

    fn next(&mut self) -> Option<Self::Item> {
        // Implementation for iterating over columns
        // ... (similar to SeaORM's implementation)
    }
}
```

4. **Add Conversion from Column Enum:**
```rust
/// Trait for converting column enums to Identity
pub trait IntoIdentity {
    fn into_identity(self) -> Identity;
}

// Macro will generate implementations for Column enums
// Example: impl IntoIdentity for Column { ... }
```

**Testing:**
- Unit tests for `arity()`, `iter()`, `contains()`
- Test conversion from Column enum variants
- Test composite key creation (Binary, Ternary, Many)

**Dependencies:** `sea-query` (for `DynIden`)

---

### 4.2 Phase 2: RelationDef Struct Implementation

**Goal:** Create the `RelationDef` struct to store relationship metadata.

**File:** `src/relation/def.rs` (new file)

**Implementation Steps:**

1. **Define RelationType Enum:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationType {
    /// One-to-one relationship
    HasOne,
    /// One-to-many relationship
    HasMany,
    /// Many-to-one relationship (belongs_to)
    BelongsTo,
}
```

2. **Define RelationDef Struct:**
```rust
use crate::relation::identity::Identity;
use sea_query::{Condition, ConditionType, DynIden, TableRef};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct RelationDef {
    /// Type of relationship
    pub rel_type: RelationType,
    /// Source table reference
    pub from_tbl: TableRef,
    /// Target table reference
    pub to_tbl: TableRef,
    /// Foreign key column(s) in source table
    pub from_col: Identity,
    /// Primary key column(s) in target table
    pub to_col: Identity,
    /// Whether this entity owns the relationship
    pub is_owner: bool,
    /// Skip foreign key constraint generation
    pub skip_fk: bool,
    /// Optional custom join condition
    pub on_condition: Option<Arc<dyn Fn(DynIden, DynIden) -> Condition + Send + Sync>>,
    /// Condition type (All/Any)
    pub condition_type: ConditionType,
}
```

3. **Implement From<RelationDef> for Condition (for JOINs):**
```rust
impl From<RelationDef> for Condition {
    fn from(mut rel: RelationDef) -> Condition {
        let from_tbl = rel.from_tbl.clone();
        let to_tbl = rel.to_tbl.clone();
        
        let mut condition = match rel.condition_type {
            ConditionType::All => Condition::all(),
            ConditionType::Any => Condition::any(),
        };

        // Build join condition: from_table.from_col = to_table.to_col
        condition = condition.add(join_tbl_on_condition(
            from_tbl,
            to_tbl,
            rel.from_col,
            rel.to_col,
        ));

        // Add custom condition if provided
        if let Some(f) = rel.on_condition.take() {
            condition = condition.add(f(from_tbl.clone(), to_tbl.clone()));
        }

        condition
    }
}
```

4. **Implement Helper Function for WHERE Clauses:**
```rust
/// Build WHERE condition from RelationDef and model primary key values
pub fn build_where_condition<M>(
    rel_def: &RelationDef,
    model: &M,
) -> Condition
where
    M: ModelTrait + LifeModelTrait,
{
    let mut condition = Condition::all();
    
    // Get primary key values from model
    let pk_identity = model.get_primary_key_identity();
    let pk_values = extract_primary_key_values(model, &pk_identity);
    
    // Match foreign key columns to primary key values
    for (fk_col, pk_val) in rel_def.from_col.iter().zip(pk_values.iter()) {
        let expr = Expr::col((rel_def.to_tbl.clone(), fk_col.clone()))
            .eq(Expr::val(pk_val.clone()));
        condition = condition.add(expr);
    }
    
    condition
}

/// Extract primary key values from model based on Identity
fn extract_primary_key_values<M>(model: &M, pk_identity: &Identity) -> Vec<Value>
where
    M: ModelTrait,
{
    // Implementation to extract values from model based on Identity columns
    // This requires ModelTrait to have a method like get_value_by_column()
    // ... (to be implemented)
}
```

5. **Implement join_tbl_on_condition Helper:**
```rust
/// Build join condition from Identity pairs
pub fn join_tbl_on_condition(
    from_tbl: TableRef,
    to_tbl: TableRef,
    from_col: Identity,
    to_col: Identity,
) -> Condition {
    let mut condition = Condition::all();
    
    // Ensure arities match
    assert_eq!(
        from_col.arity(),
        to_col.arity(),
        "Foreign key and primary key must have matching arity"
    );
    
    // Build equality conditions for each column pair
    for (fk_col, pk_col) in from_col.iter().zip(to_col.iter()) {
        let expr = Expr::col((from_tbl.clone(), fk_col.clone()))
            .equals(Expr::col((to_tbl.clone(), pk_col.clone())));
        condition = condition.add(expr);
    }
    
    condition
}
```

**Testing:**
- Test `From<RelationDef> for Condition` conversion
- Test `build_where_condition()` with single and composite keys
- Test `join_tbl_on_condition()` with various Identity combinations
- Test error handling for mismatched arities

**Dependencies:** `sea-query`, `crate::relation::identity`, `crate::model::ModelTrait`

---

### 4.3 Phase 3: Update Related Trait

**Goal:** Change `Related::to()` to return `RelationDef` instead of `SelectQuery`.

**File:** `src/relation.rs`

**Implementation Steps:**

1. **Update Related Trait Signature:**
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

2. **Update find_related() Implementation:**
```rust
impl<M> FindRelated for M
where
    M: ModelTrait + LifeModelTrait,
    M::Entity: LifeEntityName,
{
    fn find_related<R>(&self) -> SelectQuery<R>
    where
        R: LifeModelTrait + Related<Self::Entity>,
    {
        let mut query = SelectQuery::new();
        let rel_def = R::to();
        
        // Build WHERE condition from RelationDef and model
        let condition = build_where_condition(&rel_def, self);
        query = query.filter(condition);
        
        query
    }
}
```

**Breaking Changes:**
- `Related::to()` now returns `RelationDef` instead of `SelectQuery<Self>`
- All existing `Related` implementations must be updated
- This is a **breaking change** but necessary for proper design

**Migration Path:**
- Update all existing `Related` implementations to return `RelationDef`
- Update any code that calls `Related::to()` directly
- Provide migration guide in documentation

**Testing:**
- Test `find_related()` with single key relationships
- Test `find_related()` with composite key relationships
- Test backward compatibility (if possible)

---

### 4.4 Phase 4: Enhance ModelTrait

**Goal:** Add `get_primary_key_identity()` method to `ModelTrait`.

**File:** `src/model.rs`

**Implementation Steps:**

1. **Add Method to ModelTrait:**
```rust
pub trait ModelTrait {
    /// Get the primary key value (backward compatible)
    fn get_primary_key_value(&self) -> Value;
    
    /// Get the primary key as Identity (supports composite keys)
    fn get_primary_key_identity(&self) -> Identity;
    
    /// Get primary key values as Vec<Value> (helper for WHERE clauses)
    fn get_primary_key_values(&self) -> Vec<Value> {
        // Default implementation extracts values from Identity
        // Macro will override this for efficiency
        let identity = self.get_primary_key_identity();
        extract_values_from_identity(self, &identity)
    }
}
```

2. **Add Helper Function:**
```rust
/// Extract values from model based on Identity columns
fn extract_values_from_identity<M>(model: &M, identity: &Identity) -> Vec<Value>
where
    M: ModelTrait,
{
    // This requires ModelTrait to have get_value_by_column() or similar
    // Implementation depends on how we access model fields
    // ... (to be implemented)
}
```

**Testing:**
- Test `get_primary_key_identity()` for single keys
- Test `get_primary_key_identity()` for composite keys
- Test `get_primary_key_values()` extraction

---

### 4.5 Phase 5: Update LifeModel Macro

**Goal:** Generate `get_primary_key_identity()` implementation in `LifeModel` macro.

**File:** `lifeguard-derive/src/macros/life_model.rs`

**Implementation Steps:**

1. **Generate Identity for Single Primary Key:**
```rust
// In the ModelTrait implementation generation
let pk_identity_impl = if primary_key_variant_idents.len() == 1 {
    let pk_col = &primary_key_variant_idents[0];
    quote! {
        fn get_primary_key_identity(&self) -> Identity {
            Identity::Unary(Column::#pk_col.into_iden())
        }
    }
} else {
    // Handle composite keys
    // ...
};
```

2. **Generate Identity for Composite Primary Keys:**
```rust
let pk_identity_impl = match primary_key_variant_idents.len() {
    1 => {
        // Single key - Unary
        quote! { Identity::Unary(Column::#col1.into_iden()) }
    }
    2 => {
        // Two keys - Binary
        quote! { Identity::Binary(Column::#col1.into_iden(), Column::#col2.into_iden()) }
    }
    3 => {
        // Three keys - Ternary
        quote! { Identity::Ternary(Column::#col1.into_iden(), Column::#col2.into_iden(), Column::#col3.into_iden()) }
    }
    n => {
        // Four or more keys - Many
        let cols: Vec<_> = primary_key_variant_idents.iter().map(|col| {
            quote! { Column::#col.into_iden() }
        }).collect();
        quote! { Identity::Many(vec![#(#cols),*]) }
    }
};
```

3. **Generate get_primary_key_values() Implementation:**
```rust
let pk_values_impl = {
    let pk_value_exprs: Vec<_> = primary_key_variant_idents.iter().map(|col| {
        // Get field name from column
        let field_name = convert_column_to_field_name(col);
        quote! { self.#field_name.into() }
    }).collect();
    
    quote! {
        fn get_primary_key_values(&self) -> Vec<Value> {
            vec![#(#pk_value_exprs),*]
        }
    }
};
```

**Testing:**
- Test macro generates correct `Identity` for single keys
- Test macro generates correct `Identity` for composite keys (2, 3, 4+)
- Test macro generates correct `get_primary_key_values()` implementation

---

### 4.6 Phase 6: Update DeriveRelation Macro

**Goal:** Generate `RelationDef` construction in `Related::to()` implementations.

**File:** `lifeguard-derive/src/macros/relation.rs`

**Implementation Steps:**

1. **Parse from/to Attributes:**
```rust
// In process_relation_variant()
let (from_col, to_col) = if let Some(from_attr) = from_attr {
    let from_col = extract_column_identity(&from_attr)?;
    let to_col = if let Some(to_attr) = to_attr {
        extract_column_identity(&to_attr)?
    } else {
        // Default: infer from target entity's primary key
        infer_primary_key_identity(&target_entity_path)?
    };
    (from_col, to_col)
} else {
    // Default: infer both from relationship type
    infer_default_columns(&variant, &target_entity_path)?
};
```

2. **Generate RelationDef Construction:**
```rust
let relation_def = quote! {
    RelationDef {
        rel_type: RelationType::#relation_type_variant,
        from_tbl: TableRef::Table(Entity::table_name().into()),
        to_tbl: TableRef::Table(#target_entity_path::Entity::table_name().into()),
        from_col: #from_col,
        to_col: #to_col,
        is_owner: true,
        skip_fk: false,
        on_condition: None,
        condition_type: ConditionType::All,
    }
};
```

3. **Generate Related Implementation:**
```rust
let related_impl = quote! {
    impl Related<#target_entity_path::Entity> for Entity {
        fn to() -> RelationDef {
            #relation_def
        }
    }
};
```

**Testing:**
- Test macro generates `RelationDef` for `has_many` relationships
- Test macro generates `RelationDef` for `belongs_to` relationships
- Test macro handles `from`/`to` attributes correctly
- Test macro handles composite foreign keys
- Test macro infers default columns when not specified

---

### 4.7 Phase 7: Integration and Testing

**Goal:** Integrate all components and add comprehensive tests.

**Implementation Steps:**

1. **Update Module Structure:**
```rust
// src/relation/mod.rs
pub mod identity;
pub mod def;
pub use identity::Identity;
pub use def::{RelationDef, RelationType, build_where_condition, join_tbl_on_condition};
```

2. **Add Integration Tests:**
- **File:** `tests/integration/relation_def.rs`
  - Single key relationships
  - Composite key relationships (2, 3, 4+ columns)
  - Custom foreign key column names
  - Default naming fallback

3. **Add Macro Tests:**
- **File:** `lifeguard-derive/tests/test_derive_relation.rs`
  - Test `RelationDef` generation
  - Test `Identity` generation for composite keys
  - Test attribute parsing

4. **Update Documentation:**
- Update `DERIVE_RELATION_USAGE.md` with RelationDef examples
- Update `SEAORM_LIFEGUARD_MAPPING.md` implementation notes
- Add migration guide for breaking changes

**Testing Checklist:**
- [ ] Single key relationships work
- [ ] Composite key relationships work (2, 3, 4+ columns)
- [ ] Custom foreign key columns work
- [ ] Default naming fallback works
- [ ] Error handling for mismatched arities
- [ ] Performance benchmarks
- [ ] Backward compatibility (where possible)

---

### 4.8 Implementation Order Summary

**Recommended Implementation Order:**

1. **Phase 1:** Identity Enum (`src/relation/identity.rs`)
   - Foundation for everything else
   - Can be tested independently

2. **Phase 2:** RelationDef Struct (`src/relation/def.rs`)
   - Depends on Identity
   - Core metadata structure

3. **Phase 3:** Update Related Trait (`src/relation.rs`)
   - Breaking change - do early to identify migration issues
   - Depends on RelationDef

4. **Phase 4:** Enhance ModelTrait (`src/model.rs`)
   - Depends on Identity
   - Required for find_related()

5. **Phase 5:** Update LifeModel Macro (`lifeguard-derive/src/macros/life_model.rs`)
   - Depends on ModelTrait changes
   - Generates Identity implementations

6. **Phase 6:** Update DeriveRelation Macro (`lifeguard-derive/src/macros/relation.rs`)
   - Depends on RelationDef
   - Generates RelationDef in Related::to()

7. **Phase 7:** Integration and Testing
   - Final integration
   - Comprehensive testing
   - Documentation updates

---

## 5. API Changes

### 5.1 Breaking Changes

**⚠️ BREAKING CHANGE:** `Related::to()` return type changes from `SelectQuery<Self>` to `RelationDef`.

**Impact:**
- All existing `Related` trait implementations must be updated
- Code that calls `Related::to()` directly will break
- `find_related()` API remains the same (no breaking change for end users)

**Migration Path:**
1. Update all `Related` implementations to return `RelationDef` instead of `SelectQuery`
2. Use `DeriveRelation` macro to auto-generate implementations
3. For manual implementations, construct `RelationDef` with relationship metadata

**Additive Changes:**
- `get_primary_key_identity()` is a new method on `ModelTrait`
- `get_primary_key_values()` is a new helper method on `ModelTrait`
- All new functionality is additive

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

### Phase 1: Identity Enum Implementation
- [ ] Create `src/relation/identity.rs` module
- [ ] Define `Identity` enum (Unary, Binary, Ternary, Many)
- [ ] Implement helper methods (`arity()`, `iter()`, `contains()`)
- [ ] Implement iterator for `Identity`
- [ ] Add `IntoIdentity` trait for Column enum conversion
- [ ] Add unit tests for Identity functionality
- [ ] Document public API

### Phase 2: RelationDef Struct Implementation
- [ ] Create `src/relation/def.rs` module
- [ ] Define `RelationType` enum (HasOne, HasMany, BelongsTo)
- [ ] Define `RelationDef` struct with all fields
- [ ] Implement `From<RelationDef> for Condition` (for JOINs)
- [ ] Implement `build_where_condition()` helper function
- [ ] Implement `join_tbl_on_condition()` helper function
- [ ] Add unit tests for RelationDef conversion
- [ ] Add tests for WHERE condition building
- [ ] Document public API

### Phase 3: Update Related Trait (Breaking Change)
- [ ] Change `Related::to()` return type from `SelectQuery<Self>` to `RelationDef`
- [ ] Update `find_related()` to use `RelationDef` and `build_where_condition()`
- [ ] Update all existing `Related` implementations (if any)
- [ ] Add migration guide for breaking changes
- [ ] Add integration tests for `find_related()` with RelationDef
- [ ] Verify backward compatibility where possible

### Phase 4: Enhance ModelTrait
- [ ] Add `get_primary_key_identity()` method to `ModelTrait`
- [ ] Add `get_primary_key_values()` helper method
- [ ] Implement `extract_values_from_identity()` helper function
- [ ] Add unit tests for new ModelTrait methods
- [ ] Update ModelTrait documentation

### Phase 5: Update LifeModel Macro
- [ ] Generate `get_primary_key_identity()` implementation
- [ ] Handle single primary keys (Unary)
- [ ] Handle composite primary keys (Binary, Ternary, Many)
- [ ] Generate `get_primary_key_values()` implementation
- [ ] Add macro tests for Identity generation
- [ ] Test with various primary key configurations

### Phase 6: Update DeriveRelation Macro
- [ ] Parse `from`/`to` attributes to build `Identity`
- [ ] Generate `RelationDef` construction in `Related::to()`
- [ ] Handle single foreign keys
- [ ] Handle composite foreign keys
- [ ] Infer default columns when not specified
- [ ] Add macro tests for RelationDef generation
- [ ] Test with various relationship types

### Phase 7: Integration and Testing
- [ ] Update module structure (`src/relation/mod.rs`)
- [ ] Add integration tests for single key relationships
- [ ] Add integration tests for composite key relationships
- [ ] Add error handling tests (mismatched arities, etc.)
- [ ] Performance benchmarking
- [ ] Update `DERIVE_RELATION_USAGE.md` with examples
- [ ] Update `SEAORM_LIFEGUARD_MAPPING.md` implementation notes
- [ ] Create migration guide for breaking changes
- [ ] Final documentation review

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

1. **Breaking Change Strategy:** How should we handle the breaking change to `Related::to()`? Should we:
   - Release as a major version bump?
   - Provide a migration path with deprecated methods?
   - Support both old and new APIs temporarily?

2. **Composite Key Syntax:** Should we support array syntax `from = [...]` or require separate attributes?
   - Array syntax: `from = ["Column::UserId", "Column::TenantId"]`
   - Separate attributes: `from_col1 = "Column::UserId", from_col2 = "Column::TenantId"`
   - Recommendation: Start with array syntax for simplicity

3. **Error Handling:** How should we handle mismatched key sizes (e.g., 2 primary keys but 1 foreign key)?
   - Option A: Panic at runtime with clear error message
   - Option B: Compile-time check in macro (preferred)
   - Option C: Return Result and let user handle
   - Recommendation: Compile-time check in macro when possible, runtime panic as fallback

4. **ModelTrait Value Extraction:** How should we extract values from model based on `Identity`?
   - Option A: Add `get_value_by_column(column: Column) -> Value` to ModelTrait
   - Option B: Use reflection (not available in stable Rust)
   - Option C: Macro generates helper function for each model
   - Recommendation: Option C - macro generates efficient value extraction

5. **TableRef Implementation:** Do we need to implement custom `TableRef` or can we use SeaQuery's?
   - Check if SeaQuery's `TableRef` is sufficient
   - May need wrapper for table name handling

6. **Performance Optimization:** Should we cache `RelationDef` instances or generate them each time?
   - `Related::to()` is called each time `find_related()` is called
   - Consider caching if performance becomes an issue
   - Initial implementation: generate each time (simpler)

---

**Document Status:** 🟡 Awaiting Review  
**Last Updated:** 2025-01-27  
**Next Steps:** Review design, approve approach, begin Phase 1 implementation
