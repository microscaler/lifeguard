# SeaORM/SeaQuery → Lifeguard Mapping

## Overview

This document maps SeaORM (v2.0.0-rc.28) and SeaQuery (v0.32.7) components to their Lifeguard equivalents, identifying what exists, what's missing, and future state.

## Core Features

**JSON Support:** JSON is a **core feature** in Lifeguard and is always enabled. All JSON-related functionality is implemented as standard functionality, not as optional features. This includes:
- JSON column type support via `serde_json::Value`
- JSON value serialization/deserialization in queries
- JSON handling in ModelTrait get/set operations
- No feature flags required - JSON support is built-in

---

## 1. Core Traits & Types

| SeaORM/SeaQuery | Lifeguard | Status | Notes |
|----------------|-----------|--------|-------|
| `EntityTrait` | `LifeModelTrait` | ✅ Implemented | Similar API, provides `find()` method |
| `EntityName` | `LifeEntityName` | ✅ Implemented | Provides `table_name()` method |
| `ModelTrait` | ✅ Implemented | ✅ Complete | Model-level operations (get/set columns, get_primary_key_value) |
| `FromQueryResult` | `FromRow` | ✅ Implemented | Converts database rows to Model structs |
| `ActiveModelTrait` | ❌ Missing | 🔴 **Future** | Mutable model for inserts/updates (our `LifeRecord` is similar but different) |
| `ActiveModelBehavior` | ❌ Missing | 🟡 **Future** | Custom behavior hooks for ActiveModel |
| `ColumnTrait` | ✅ Implemented | ✅ Complete | Column-level operations (query builder methods ✅, metadata methods ✅ with default impls) |
| `PrimaryKeyTrait` | ❌ Missing | 🔴 **Future** | Primary key operations (auto_increment, ValueType) |
| `PrimaryKeyToColumn` | ❌ Missing | 🔴 **Future** | Mapping between PrimaryKey and Column |
| `PrimaryKeyArity` | ❌ Missing | 🔴 **Future** | Support for composite primary keys |
| `RelationTrait` | ❌ Missing | 🟡 **Future** | Entity relationships (belongs_to, has_one, has_many) |
| `Related` | ❌ Missing | 🟡 **Future** | Related entity queries |
| `Linked` | ❌ Missing | 🟡 **Future** | Multi-hop relationship queries |
| `PartialModelTrait` | ❌ Missing | 🟡 **Future** | Partial model queries (select subset of columns) |
| `TryIntoModel` | ❌ Missing | 🟡 **Future** | Conversion utilities |

---

## 2. Derive Macros

| SeaORM Macro | Lifeguard Macro | Status | Notes |
|-------------|----------------|--------|-------|
| `DeriveEntity` | `DeriveEntity` | ✅ Implemented | Generates Entity, EntityName, Iden, IdenStatic, LifeModelTrait. Used for nested expansion from LifeModel |
| `DeriveEntityModel` | `LifeModel` | ✅ Implemented | Combined macro (Entity + Model + Column + PrimaryKey + FromRow + ModelTrait) |
| `DeriveModel` | ❌ Not Needed | ✅ By Design | LifeModel generates Model struct + ModelTrait impl directly. No separate DeriveModel needed (unlike DeriveEntity which is used for nested expansion of unit struct) |
| `DeriveModelEx` | ❌ Missing | 🔴 **Future** | Complex model with relational fields |
| `DeriveActiveModel` | ❌ Missing | 🔴 **Future** | ActiveModel struct (our `LifeRecord` is different) |
| `DeriveActiveModelEx` | ❌ Missing | 🔴 **Future** | Complex ActiveModel with relational fields |
| `DeriveColumn` | ❌ Not Needed | ✅ By Design | LifeModel generates Column enum + Iden/IdenStatic impls directly |
| `DerivePrimaryKey` | ❌ Not Needed | ✅ By Design | LifeModel generates PrimaryKey enum directly |
| `DeriveIntoActiveModel` | ❌ Missing | 🔴 **Future** | Conversion from Model to ActiveModel |
| `DeriveActiveModelBehavior` | ❌ Missing | 🟡 **Future** | ActiveModelBehavior trait implementation |
| `DeriveActiveEnum` | ❌ Missing | 🟡 **Future** | Enum support for ActiveModel |
| `FromQueryResult` | `FromRow` | ✅ Implemented | Separate derive (matches SeaORM pattern) |
| `DeriveRelation` | ❌ Missing | 🟡 **Future** | Relation enum with RelationTrait |
| `DeriveRelatedEntity` | ❌ Missing | 🟡 **Future** | RelatedEntity enum |
| `DeriveMigrationName` | ❌ Missing | 🟡 **Future** | Migration name generation |
| `FromJsonQueryResult` | ❌ Missing | 🟡 **Future** | JSON query result deserialization (JSON column support is ✅ core feature) |
| `DerivePartialModel` | ❌ Missing | 🟡 **Future** | PartialModelTrait implementation |
| `DeriveValueType` | ❌ Missing | 🟡 **Future** | ValueType trait for wrapper types |
| `DeriveDisplay` | ❌ Missing | 🟡 **Future** | Display trait for ActiveEnum |
| `DeriveIden` | ❌ Missing | 🟡 **Future** | Iden trait helper |

**Lifeguard-Specific:**
- `LifeRecord` - ✅ Implemented (simplified version, generates Record struct with Option<T> fields)

### Architecture Pattern: Why `DeriveModel` is Not Needed

Lifeguard follows SeaORM's nested macro expansion pattern, but with a key difference:

**SeaORM Pattern:**
- `DeriveEntityModel` generates Entity struct + Model struct
- `DeriveEntity` (nested) generates trait implementations for Entity (unit struct)
- `DeriveModel` (nested) generates trait implementations for Model (data struct)

**Lifeguard Pattern:**
- `LifeModel` generates Entity struct + Model struct + all trait implementations
- `DeriveEntity` (nested) generates trait implementations for Entity (unit struct)
- `DeriveModel` is **not needed** because `LifeModel` generates Model + ModelTrait directly

**Why the difference?**
- `DeriveEntity` exists because Entity is a **unit struct** used in nested expansion (`#[derive(DeriveEntity)]` on Entity)
- Model is a **data struct with fields**, so `LifeModel` can generate both the struct and its trait implementations in the same expansion phase
- No use case exists for manually declaring a Model struct and only deriving traits (unlike Entity which is a unit struct)

This design simplifies the API while maintaining the same functionality.

---

## 3. Core Structures

| SeaORM/SeaQuery | Lifeguard | Status | Notes |
|----------------|-----------|--------|-------|
| `Entity` (unit struct) | `Entity` (unit struct) | ✅ Implemented | Generated by `LifeModel` |
| `Model` (struct) | `{Name}Model` (struct) | ✅ Implemented | Generated by `LifeModel` |
| `ActiveModel` (struct) | `{Name}Record` (struct) | ✅ Implemented | Generated by `LifeRecord` (different design) |
| `Column` (enum) | `Column` (enum) | ✅ Implemented | Generated by `LifeModel` |
| `PrimaryKey` (enum) | `PrimaryKey` (enum) | ✅ Implemented | Generated by `LifeModel` |
| `Relation` (enum) | ❌ Missing | 🟡 **Future** | Entity relationships |
| `ActiveValue` | ❌ Missing | 🔴 **Future** | Wrapper for ActiveModel field values |
| `ColumnDef` | ❌ Missing | 🔴 **Future** | Column definition with SQL attributes |
| `RelationDef` | ❌ Missing | 🟡 **Future** | Relation definition |
| `Select<E>` | `SelectQuery<E>` | ✅ Implemented | Query builder (different API) |
| `SelectModel<E>` | ❌ Missing | 🔴 **Future** | Typed select with Model return type |
| `Paginator` | `Paginator` | ✅ Implemented | Pagination support |
| `PaginatorWithCount` | `PaginatorWithCount` | ✅ Implemented | Pagination with total count |

---

## 4. Query Builder API

| SeaORM/SeaQuery | Lifeguard | Status | Notes |
|----------------|-----------|--------|-------|
| `Entity::find()` | `Entity::find()` | ✅ Implemented | Returns `SelectQuery<E>` |
| `Select<E>::filter()` | `SelectQuery<E>::filter()` | ✅ Implemented | WHERE clause builder |
| `Select<E>::order_by()` | `SelectQuery<E>::order_by()` | ✅ Implemented | ORDER BY clause |
| `Select<E>::limit()` | `SelectQuery<E>::limit()` | ✅ Implemented | LIMIT clause |
| `Select<E>::offset()` | `SelectQuery<E>::offset()` | ✅ Implemented | OFFSET clause |
| `Select<E>::group_by()` | ❌ Missing | 🔴 **Future** | GROUP BY clause |
| `Select<E>::having()` | ❌ Missing | 🔴 **Future** | HAVING clause |
| `Select<E>::join()` | ❌ Missing | 🟡 **Future** | JOIN operations |
| `Select<E>::left_join()` | ❌ Missing | 🟡 **Future** | LEFT JOIN |
| `Select<E>::right_join()` | ❌ Missing | 🟡 **Future** | RIGHT JOIN |
| `Select<E>::inner_join()` | ❌ Missing | 🟡 **Future** | INNER JOIN |
| `Select<E>::all()` | `SelectQuery<E>::all()` | ✅ Implemented | Execute and return Vec<Model> |
| `Select<E>::one()` | `SelectQuery<E>::one()` | ✅ Implemented | Execute and return Option<Model> |
| `Select<E>::paginate()` | `SelectQuery<E>::paginate()` | ✅ Implemented | Returns Paginator |
| `Select<E>::paginate_and_count()` | `SelectQuery<E>::paginate_and_count()` | ✅ Implemented | Returns PaginatorWithCount |
| `Select<E>::count()` | `SelectQuery<E>::count()` | ✅ Implemented | COUNT query |
| `Model::find_related<R>()` | ❌ Missing | 🟡 **Future** | Find related entities |
| `Model::find_linked<L>()` | ❌ Missing | 🟡 **Future** | Find linked entities |
| `Entity::insert()` | ❌ Missing | 🔴 **Future** | Insert ActiveModel |
| `Entity::update()` | ❌ Missing | 🔴 **Future** | Update ActiveModel |
| `Entity::delete()` | ❌ Missing | 🔴 **Future** | Delete by primary key |
| `Entity::delete_many()` | `Model::delete_many()` | ✅ Implemented | Batch delete (different API) |
| `Entity::insert_many()` | `Model::insert_many()` | ✅ Implemented | Batch insert (different API) |
| `Entity::update_many()` | `Model::update_many()` | ✅ Implemented | Batch update (different API) |

---

## 5. Column Operations

| SeaORM/SeaQuery | Lifeguard | Status | Notes |
|----------------|-----------|--------|-------|
| `Column::def()` | ✅ Implemented | ✅ Complete | Column definition with type, nullable, etc. (returns ColumnDefinition) |
| `Column::enum_type_name()` | ✅ Implemented | ✅ Complete | Enum type name for enum columns (default impl returns None, macro should override) |
| `Column::select_as()` | ✅ Implemented | ✅ Complete | Custom SELECT expression (default impl returns None, macro should override) |
| `Column::save_as()` | ✅ Implemented | ✅ Complete | Custom save expression (default impl returns None, macro should override) |
| `Column::eq()` | ✅ Implemented | ✅ Complete | Equality comparison (via ColumnTrait) |
| `Column::ne()` | ✅ Implemented | ✅ Complete | Inequality comparison |
| `Column::gt()` | ✅ Implemented | ✅ Complete | Greater than |
| `Column::gte()` | ✅ Implemented | ✅ Complete | Greater than or equal |
| `Column::lt()` | ✅ Implemented | ✅ Complete | Less than |
| `Column::lte()` | ✅ Implemented | ✅ Complete | Less than or equal |
| `Column::like()` | ✅ Implemented | ✅ Complete | LIKE pattern matching |
| `Column::is_in()` | ✅ Implemented | ✅ Complete | IN clause |
| `Column::is_not_in()` | ✅ Implemented | ✅ Complete | NOT IN clause |
| `Column::is_null()` | ✅ Implemented | ✅ Complete | IS NULL check |
| `Column::is_not_null()` | ✅ Implemented | ✅ Complete | IS NOT NULL check |
| `Column::between()` | ✅ Implemented | ✅ Complete | BETWEEN clause |

**Note:** All query builder methods are fully implemented. Metadata methods (`def()`, `enum_type_name()`, `select_as()`, `save_as()`) have default implementations that return empty/None values. The `LifeModel` macro should generate overrides for these methods based on field attributes to provide actual column metadata.

---

## 6. ActiveModel/Record Operations

| SeaORM/SeaQuery | Lifeguard | Status | Notes |
|----------------|-----------|--------|-------|
| `ActiveModel::insert()` | `Record::insert()` | ⚠️ Removed | Was in life_record.rs, removed in simplification |
| `ActiveModel::update()` | `Record::update()` | ⚠️ Removed | Was in life_record.rs, removed in simplification |
| `ActiveModel::save()` | ❌ Missing | 🔴 **Future** | Insert or update based on primary key |
| `ActiveModel::delete()` | ❌ Missing | 🔴 **Future** | Delete by primary key |
| `ActiveModel::reset()` | ❌ Missing | 🔴 **Future** | Reset all fields to default |
| `ActiveModel::set()` | `Record::set_*()` | ✅ Implemented | Setter methods (different API) |
| `ActiveModel::get()` | ❌ Missing | 🔴 **Future** | Get field value |
| `ActiveModel::take()` | ❌ Missing | 🔴 **Future** | Take field value (move) |
| `ActiveModel::into_active_value()` | ❌ Missing | 🔴 **Future** | Convert to ActiveValue |
| `ActiveModel::from_json()` | ❌ Missing | 🟡 **Future** | Deserialize from JSON (JSON column support is ✅ core feature) |
| `ActiveModel::to_json()` | ❌ Missing | 🟡 **Future** | Serialize to JSON (JSON column support is ✅ core feature) |
| `Model::into_active_model()` | `Model::to_record()` | ✅ Implemented | Convert Model to Record (different name) |
| `Record::from_model()` | ✅ Implemented | Create Record from Model |
| `Record::to_model()` | ✅ Implemented | Convert Record to Model |
| `Record::dirty_fields()` | ✅ Implemented | Get list of changed fields |
| `Record::is_dirty()` | ✅ Implemented | Check if any fields changed |

---

## 7. Value Types & Conversions

| SeaORM/SeaQuery | Lifeguard | Status | Notes |
|----------------|-----------|--------|-------|
| `Value` (enum) | `sea_query::Value` | ✅ Used | Direct use of SeaQuery's Value |
| `ValueType` | ❌ Missing | 🟡 **Future** | Trait for value type conversions |
| `TryGetable` | ❌ Missing | 🟡 **Future** | Trait for safe value extraction |
| `TryGetableMany` | ❌ Missing | 🟡 **Future** | Trait for extracting multiple values |
| `IntoValueTuple` | ❌ Missing | 🔴 **Future** | Conversion to ValueTuple (for composite keys) |
| `FromValueTuple` | ❌ Missing | 🔴 **Future** | Conversion from ValueTuple |
| `TryFromU64` | ❌ Missing | 🟡 **Future** | Conversion from u64 (for primary keys) |

---

## 8. Attributes & Configuration

| SeaORM Attribute | Lifeguard Attribute | Status | Notes |
|----------------|---------------------|--------|-------|
| `#[sea_orm(table_name = "...")]` | `#[table_name = "..."]` | ✅ Implemented | Table name |
| `#[sea_orm(schema_name = "...")]` | ❌ Missing | 🟡 **Future** | Schema name |
| `#[sea_orm(primary_key)]` | `#[primary_key]` | ✅ Implemented | Primary key field |
| `#[sea_orm(auto_increment = bool)]` | `#[auto_increment]` | ⚠️ Partial | Exists but not fully used |
| `#[sea_orm(column_name = "...")]` | `#[column_name = "..."]` | ✅ Implemented | Custom column name |
| `#[sea_orm(column_type = "...")]` | `#[column_type = "..."]` | ⚠️ Partial | Exists but not fully used |
| `#[sea_orm(nullable)]` | `#[nullable]` | ✅ Implemented | Nullable field |
| `#[sea_orm(default_value = ...)]` | `#[default_value = ...]` | ⚠️ Partial | Exists but not fully used |
| `#[sea_orm(default_expr = "...")]` | ❌ Missing | 🟡 **Future** | Default SQL expression |
| `#[sea_orm(unique)]` | `#[unique]` | ⚠️ Partial | Exists but not fully used |
| `#[sea_orm(indexed)]` | `#[indexed]` | ⚠️ Partial | Exists but not fully used |
| `#[sea_orm(ignore)]` | ❌ Missing | 🟡 **Future** | Ignore field |
| `#[sea_orm(enum_name = "...")]` | `#[enum_name = "..."]` | ⚠️ Partial | Exists but not fully used |
| `#[sea_orm(select_as = "...")]` | ❌ Missing | 🟡 **Future** | Custom SELECT expression |
| `#[sea_orm(save_as = "...")]` | ❌ Missing | 🟡 **Future** | Custom save expression |
| `#[sea_orm(renamed_from = "...")]` | ❌ Missing | 🟡 **Future** | Column renamed from |
| `#[sea_orm(comment = "...")]` | ❌ Missing | 🟡 **Future** | Column comment |

---

## 9. Future State Descriptions

### High Priority (Core Functionality)

#### ModelTrait
**Status:** ✅ Implemented  
**Current State:** Trait for Model-level operations:
- `get(column)` - Get column value as `Value` ✅
- `set(column, value)` - Set column value ✅
- `get_primary_key_value()` - Get primary key value(s) ✅
- `get_value_type(column)` - Get column's value type (🟡 Future)
- `find_related<R>()` - Find related entities (🟡 Future)
- `find_linked<L>()` - Find linked entities (🟡 Future)

#### ColumnTrait
**Status:** ✅ Implemented  
**Current State:** Trait for Column-level operations:
- Query builder methods: `eq()`, `ne()`, `gt()`, `gte()`, `lt()`, `lte()`, `like()`, `is_in()`, `is_not_in()`, `is_null()`, `is_not_null()`, `between()` ✅
- `def()` - Column definition (returns `ColumnDefinition` with metadata) ✅ (default impl, macro should override)
- `enum_type_name()` - Enum type name for enum columns ✅ (default impl returns None, macro should override)
- `select_as()` - Custom SELECT expression ✅ (default impl returns None, macro should override)
- `save_as()` - Custom save expression ✅ (default impl returns None, macro should override)

**Note:** Query builder methods are fully functional. Metadata methods have default implementations that return empty/None values. The `LifeModel` macro should generate column-specific overrides based on field attributes to provide actual metadata. This allows the trait to work immediately while macro generation can enhance it with real column metadata.

#### PrimaryKeyTrait
**Status:** 🔴 Missing  
**Future State:** Trait for PrimaryKey operations:
- `ValueType` - Associated type for primary key value type
- `auto_increment()` - Whether primary key is auto-increment
- Support for composite primary keys (via `PrimaryKeyArity`)

#### ActiveModel Operations
**Status:** 🔴 Missing  
**Future State:** Full ActiveModel API:
- `insert()`, `update()`, `save()`, `delete()` methods
- `get()`, `set()`, `take()` field access
- `reset()` to reset all fields
- `from_json()`, `to_json()` serialization
- Integration with `ActiveModelBehavior` for custom hooks

### Medium Priority (Relations & Advanced Features)

#### Relations
**Status:** 🟡 Future  
**Future State:** Entity relationship support:
- `RelationTrait` - Define relationships (belongs_to, has_one, has_many, has_many_through)
- `Related` - Related entity queries
- `Linked` - Multi-hop relationship queries
- `DeriveRelation` - Generate Relation enum
- `DeriveRelatedEntity` - Generate RelatedEntity enum

#### Partial Models
**Status:** 🟡 Future  
**Future State:** Support for partial model queries:
- `PartialModelTrait` - Trait for partial models
- `DerivePartialModel` - Generate partial model structs
- Select subset of columns from queries

#### Advanced Query Features
**Status:** 🟡 Future  
**Future State:**
- `group_by()`, `having()` - GROUP BY and HAVING clauses
- `join()`, `left_join()`, `right_join()`, `inner_join()` - JOIN operations
- Subqueries and CTEs
- Window functions

### Low Priority (Nice-to-Have)

#### Value Type System
**Status:** 🟡 Future  
**Future State:** Enhanced value type system:
- `ValueType` trait for custom value types
- `TryGetable` and `TryGetableMany` for safe value extraction
- `IntoValueTuple` and `FromValueTuple` for composite keys
- `TryFromU64` for primary key conversions

#### Migration Support
**Status:** 🟡 Future  
**Future State:**
- `DeriveMigrationName` - Generate migration names
- Integration with migration tools

#### JSON Support
**Status:** ✅ Core Feature (Always Enabled)  
**Current State:**
- ✅ JSON column type support via `serde_json::Value` - Fully implemented
- ✅ JSON value serialization in queries - Fully implemented
- ✅ JSON handling in ModelTrait get/set operations - Fully implemented
- ✅ No feature flags required - JSON is always available

**Future Enhancements:**
- `FromJsonQueryResult` - JSON query result deserialization (🟡 Future)
- `ActiveModel::from_json()`, `ActiveModel::to_json()` - ActiveModel JSON methods (🟡 Future)

**Note:** JSON support is a core feature and is always enabled. All JSON functionality works out of the box without any feature flags or configuration.

#### Enum Support
**Status:** 🟡 Future  
**Future State:**
- `DeriveActiveEnum` - Enum support for ActiveModel
- `DeriveDisplay` - Display trait for ActiveEnum
- Enum column type handling

---

## 10. Summary Statistics

| Category | SeaORM | Lifeguard | Coverage |
|----------|--------|-----------|----------|
| **Core Traits** | 15 | 4 | 27% |
| **Derive Macros** | 21 | 7 | 33% |
| **Core Structures** | 10 | 6 | 60% |
| **Query Builder Methods** | 20 | 10 | 50% |
| **Column Operations** | 15 | 15 | 100% |
| **ActiveModel/Record Operations** | 12 | 5 | 42% |
| **Value Types** | 6 | 1 | 17% |
| **Attributes** | 18 | 6 | 33% |
| **Overall** | 117 | 58 | **50%** |

---

## 11. Key Architectural Differences

### 1. **ActiveModel vs LifeRecord**
- **SeaORM:** `ActiveModel` is a mutable struct with `ActiveValue` wrappers, full CRUD operations
- **Lifeguard:** `LifeRecord` is a simplified struct with `Option<T>` fields, no built-in CRUD (removed in simplification)

### 2. **Model Naming**
- **SeaORM:** Model struct is always named `Model`
- **Lifeguard:** Model struct is named `{EntityName}Model` (e.g., `UserModel`)

### 3. **Query Builder**
- **SeaORM:** `Select<E>` with async methods
- **Lifeguard:** `SelectQuery<E>` with coroutine-based methods

### 4. **Column Operations**
- **SeaORM:** Type-safe column operations via `ColumnTrait` (e.g., `Column::Id.eq(1)`)
- **Lifeguard:** Uses `sea_query::Expr` directly (e.g., `Expr::col("id").eq(1)`)

### 5. **Relations**
- **SeaORM:** Full relationship system with `RelationTrait`, `Related`, `Linked`
- **Lifeguard:** No relationship support yet

---

## 12. Migration Path

### Phase 1: Core Traits (High Priority)
1. Implement `ModelTrait` with basic operations
2. Implement `ColumnTrait` with query builder methods
3. Implement `PrimaryKeyTrait` with auto-increment support
4. Add `ColumnDef` and column metadata

### Phase 2: ActiveModel Enhancement (High Priority)
1. Restore `Record::insert()` and `Record::update()` methods
2. Add `Record::save()` method (insert or update)
3. Add `Record::delete()` method
4. Add `ActiveValue` wrapper for field values

### Phase 3: Relations (Medium Priority)
1. Implement `RelationTrait`
2. Implement `Related` trait
3. Add `DeriveRelation` macro
4. Add relationship query methods

### Phase 4: Advanced Features (Low Priority)
1. Partial models
2. Advanced query features (JOINs, GROUP BY, etc.)
3. Value type system enhancements
4. Enum support (JSON is ✅ already implemented as core feature)

---

## Notes

- **Current Focus:** Core ORM functionality (Entity, Model, Record, Query Builder)
- **Design Philosophy:** Simpler API than SeaORM, optimized for coroutines
- **Compatibility:** Uses SeaQuery directly, ensuring SQL compatibility
- **JSON Support:** JSON is a **core feature** and is always enabled. All JSON functionality (column types, serialization, ModelTrait operations) works out of the box without feature flags.
- **Future:** Incremental feature addition based on user needs
