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
| `ActiveModelTrait` | ✅ Implemented | ✅ Complete | Mutable model operations (get, set, take, reset ✅; insert/update/save/delete ✅) |
| `ActiveModelBehavior` | ✅ Implemented | ✅ Complete | Custom behavior hooks for ActiveModel (8 lifecycle hooks) |
| `ColumnTrait` | ✅ Implemented | ✅ Complete | Column-level operations (query builder methods ✅, metadata methods ✅ with default impls) |
| `PrimaryKeyTrait` | ✅ Implemented | ✅ Complete | Primary key operations (ValueType ✅, auto_increment() ✅) |
| `PrimaryKeyToColumn` | ✅ Implemented | ✅ Complete | Mapping between PrimaryKey and Column (to_column() ✅) |
| `PrimaryKeyArity` | ✅ Implemented | ✅ Enhanced | Support for composite primary keys with granular variants (Single, Tuple2-Tuple5, Tuple6Plus) - Lifeguard enhancement beyond SeaORM |
| `RelationTrait` | ✅ Implemented | 🟡 **Partial** | Entity relationships (belongs_to, has_one, has_many, has_many_through) - Trait implemented with join support, automatic join condition generation pending |
| `Related` | ✅ Implemented | 🟡 **Partial** | Related entity queries - Trait implemented, DeriveRelation macro generates implementations |
| `Linked` | ❌ Missing | 🟡 **Future** | Multi-hop relationship queries |
| `PartialModelTrait` | ✅ Implemented | 🟡 **Partial** | Partial model queries (select subset of columns) - Trait implemented, but column selection uses SELECT * fallback, DerivePartialModel macro missing |
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
| `DeriveActiveModelBehavior` | ✅ Implemented | ✅ Complete | ActiveModelBehavior trait implementation (default impl generated for all Records) |
| `DeriveActiveEnum` | ❌ Missing | 🟡 **Future** | Enum support for ActiveModel |
| `FromQueryResult` | `FromRow` | ✅ Implemented | Separate derive (matches SeaORM pattern) |
| `DeriveRelation` | ✅ Implemented | 🟡 **Partial** | Relation enum with Related trait implementations - Basic implementation complete, relationship metadata support pending |
| `DeriveRelatedEntity` | ❌ Missing | 🟡 **Future** | RelatedEntity enum |
| `DeriveMigrationName` | ❌ Missing | 🟡 **Future** | Migration name generation |
| `FromJsonQueryResult` | ❌ Missing | 🟡 **Future** | JSON query result deserialization (JSON column support is ✅ core feature) |
| `DerivePartialModel` | ❌ Missing | 🟡 **Future** | PartialModelTrait implementation (trait exists, macro needed for auto-generation) |
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
| `ActiveValue` | ✅ Implemented | ✅ Complete | Wrapper for ActiveModel field values (Set, NotSet, Unset variants) |
| `ColumnDef` | ✅ Enhanced | ✅ Complete | Column definition with SQL attributes (via ColumnDefinition::to_column_def()) |
| `RelationDef` | ❌ Missing | 🟡 **Future** | Relation definition |
| `Select<E>` | `SelectQuery<E>` | ✅ Implemented | Query builder (different API) |
| `SelectModel<E>` | ✅ Implemented | ✅ Complete | Typed select with Model return type (SelectModel<E, M>) |
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
| `Select<E>::group_by()` | `SelectQuery<E>::group_by()` | ✅ Implemented | GROUP BY clause |
| `Select<E>::having()` | `SelectQuery<E>::having()` | ✅ Implemented | HAVING clause |
| `Select<E>::join()` | `SelectQuery<E>::join()` | ✅ Implemented | JOIN operations (INNER JOIN) |
| `Select<E>::left_join()` | `SelectQuery<E>::left_join()` | ✅ Implemented | LEFT JOIN |
| `Select<E>::right_join()` | `SelectQuery<E>::right_join()` | ✅ Implemented | RIGHT JOIN |
| `Select<E>::inner_join()` | `SelectQuery<E>::inner_join()` | ✅ Implemented | INNER JOIN (alias for join()) |
| `Select<E>::all()` | `SelectQuery<E>::all()` | ✅ Implemented | Execute and return Vec<Model> |
| `Select<E>::one()` | `SelectQuery<E>::one()` | ✅ Implemented | Execute and return Option<Model> |
| `Select<E>::paginate()` | `SelectQuery<E>::paginate()` | ✅ Implemented | Returns Paginator |
| `Select<E>::paginate_and_count()` | `SelectQuery<E>::paginate_and_count()` | ✅ Implemented | Returns PaginatorWithCount |
| `Select<E>::count()` | `SelectQuery<E>::count()` | ✅ Implemented | COUNT query |
| `Model::find_related<R>()` | `FindRelated::find_related()` | ✅ Implemented | Find related entities (via FindRelated trait extension) |
| `Model::find_linked<L>()` | ❌ Missing | 🟡 **Future** | Find linked entities |
| `Entity::insert()` | ✅ Implemented | ✅ Complete | Insert ActiveModel (static convenience method) |
| `Entity::update()` | ✅ Implemented | ✅ Complete | Update ActiveModel (static convenience method) |
| `Entity::delete()` | ✅ Implemented | ✅ Complete | Delete by primary key (static convenience method) |
| `Entity::delete_many()` | `Model::delete_many()` | ✅ Implemented | Batch delete (different API) |
| `Entity::insert_many()` | `Model::insert_many()` | ✅ Implemented | Batch insert (different API) |
| `Entity::update_many()` | `Model::update_many()` | ✅ Implemented | Batch update (different API) |

---

## 5. Column Operations

| SeaORM/SeaQuery | Lifeguard | Status | Notes |
|----------------|-----------|--------|-------|
| `Column::def()` | ✅ Implemented | ✅ Complete | Column definition with type, nullable, etc. (returns ColumnDefinition, default impl) |
| `ColumnDefinition::to_column_def()` | ✅ Enhanced | ✅ Complete | Convert to SeaQuery ColumnDef for migrations (full type mapping) |
| `ColumnDefinition::from_rust_type()` | ✅ Implemented | ✅ Complete | Create ColumnDefinition from Rust type string |
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
| `ActiveModel::insert()` | `ActiveModelTrait::insert()` | ✅ Implemented | INSERT operation with auto-increment PK handling |
| `ActiveModel::update()` | `ActiveModelTrait::update()` | ✅ Implemented | UPDATE operation with WHERE clause for primary keys |
| `ActiveModel::save()` | `ActiveModelTrait::save()` | ✅ Implemented | Routes to insert or update based on PK presence |
| `ActiveModel::delete()` | `ActiveModelTrait::delete()` | ✅ Implemented | DELETE operation with WHERE clause for primary keys |
| `ActiveModel::reset()` | `ActiveModelTrait::reset()` | ✅ Implemented | Reset all fields to None |
| `ActiveModel::set()` | `ActiveModelTrait::set()` | ✅ Implemented | Set field value from Value (type conversion implemented) |
| `ActiveModel::get()` | `ActiveModelTrait::get()` | ✅ Implemented | Get field value as Option<Value> (optimized, no to_model() needed) |
| `ActiveModel::take()` | `ActiveModelTrait::take()` | ✅ Implemented | Take field value (move) (optimized, no to_model() needed) |
| `ActiveModel::into_active_value()` | ✅ Implemented | ✅ Complete | Convert to ActiveValue (default implementation in trait) |
| `ActiveModel::from_json()` | `ActiveModelTrait::from_json()` | ✅ Implemented | Deserialize from JSON (uses Model Deserialize, then from_model()) |
| `ActiveModel::to_json()` | `ActiveModelTrait::to_json()` | ✅ Implemented | Serialize to JSON (iterates over set fields using get(), converts Value to JSON - no to_model() needed) |
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
| `ActiveValue` | `lifeguard::ActiveValue` | ✅ Implemented | Wrapper for ActiveModel field values (Set, NotSet, Unset) |
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
- `find_related<R>()` - ✅ Implemented (via FindRelated trait extension)
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
**Status:** ✅ Implemented  
**Current State:** Trait for PrimaryKey operations:
- `ValueType` - Associated type for primary key value type ✅ (handles Option<T> correctly)
- `auto_increment()` - Whether primary key is auto-increment ✅ (tracks each primary key's auto_increment attribute per variant)
- Support for composite primary keys (via `PrimaryKeyArity`) - ✅ Complete

#### PrimaryKeyToColumn
**Status:** ✅ Implemented  
**Current State:** Trait for mapping PrimaryKey to Column:
- `to_column()` - Convert PrimaryKey variant to Column variant ✅

#### PrimaryKeyArity
**Status:** ✅ Implemented (Enhanced beyond SeaORM)  
**Current State:** Support for composite primary keys with granular arity variants:
- `PrimaryKeyArity` enum - `Single` for single-column, `Tuple2`-`Tuple5` for specific sizes, `Tuple6Plus` for 6+ columns ✅
- `PrimaryKeyArityTrait` - `arity()` method returns the arity of the primary key ✅
- Macro automatically generates implementation based on number of primary key variants ✅
- **Lifeguard Enhancement:** Granular arity variants (`Tuple2`, `Tuple3`, `Tuple4`, `Tuple5`, `Tuple6Plus`) provide better type safety than SeaORM's simple `Single`/`Tuple` distinction ✅

#### ActiveModel Operations
**Status:** ✅ Complete  
**Current State:** All core ActiveModel API methods implemented:
- `get()` - Get field value as `Option<Value>` ✅ (optimized - direct type conversion, no to_model() needed)
- `set()` - Set field value from `Value` ✅ (type conversion implemented for all supported types)
- `take()` - Take (move) field value ✅ (optimized - direct type conversion, no to_model() needed)
- `reset()` - Reset all fields to None ✅
- `insert()` - INSERT operation ✅ (skips auto-increment PKs, uses SeaQuery)
- `update()` - UPDATE operation ✅ (requires PK, updates only dirty fields)
- `save()` - Insert or update based on PK presence ✅ (routes to insert/update)
- `delete()` - DELETE operation ✅ (requires PK)
- `from_json()`, `to_json()` serialization ✅ (Implemented - from_json() uses Model Deserialize, to_json() iterates set fields directly)
- Integration with `ActiveModelBehavior` for custom hooks ✅ (Implemented - 8 lifecycle hooks with default implementations)

**Note:** All CRUD operations use SeaQuery for SQL generation and proper parameter binding. `get()` and `take()` have been optimized to avoid the `to_model()` requirement, using direct type conversion from `Option<T>` to `Value`.

### Medium Priority (Relations & Advanced Features)

#### Relations
**Status:** 🟡 Partial  
**Current State:**
- `RelationTrait` - ✅ Implemented with functional query building (belongs_to, has_one, has_many, has_many_through methods accept foreign keys and join conditions)
- `join_condition()` helper function - ✅ Implemented (creates join conditions from table/column names)
- All relationship methods build actual queries with LEFT JOIN clauses
**Current State:**
- `Related` - ✅ Implemented (trait for defining relationships)
- `FindRelated` - ✅ Implemented (extension trait providing `find_related()` method on models)
- `DeriveRelation` - ✅ Implemented (macro generates Related trait implementations from Relation enum)
- `RelationMetadata` - ✅ Implemented (trait for storing relationship metadata, generated by DeriveRelation when from/to columns are provided)
**Known Limitations:**
- `RelationMetadata` is generated but not yet used in `find_related()` due to Rust trait bound limitations (see Implementation Notes below)
- Composite primary key support in `find_related()` requires runtime trait checking or alternative approach
**Future State:**
- Runtime use of RelationMetadata in find_related() (requires solution to trait bound limitation)
- Composite primary key support in find_related() using relationship metadata
- Automatic join condition generation from foreign key metadata
- `Linked` - Multi-hop relationship queries
- `DeriveRelatedEntity` - Generate RelatedEntity enum
- Eager loading support
- Lazy loading support

#### Partial Models
**Status:** 🟡 Partial  
**Current State:**
- `PartialModelTrait` - ✅ Implemented (trait for partial models with `selected_columns()` method)
- `PartialModelBuilder` - ✅ Implemented (trait for building partial model queries)
- `SelectPartialQuery` - ✅ Implemented (query builder for partial models)
- `select_partial()` method - ✅ Implemented (on `SelectQuery<E>`)
**Known Limitations:**
- Column selection currently uses `SELECT *` as fallback (proper Expr-to-column conversion pending)
- Column order must match between `selected_columns()` and `FromRow` implementation
**Future State:**
- `DerivePartialModel` - Generate partial model structs automatically
- Proper column selection implementation (extract column names from Expr or change API)

#### Advanced Query Features
**Status:** 🟢 Partial  
**Current State:**
- `group_by()`, `having()` - ✅ Implemented (GROUP BY and HAVING clauses)
- `join()`, `left_join()`, `right_join()`, `inner_join()` - ✅ Implemented (JOIN operations)
**Future State:**
- Subqueries and CTEs (🟡 Future)
- Window functions (🟡 Future)

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
| **Core Traits** | 15 | 9 | 60% (Enhanced: PrimaryKeyArity with granular variants, PartialModelTrait and Related implemented) |
| **Derive Macros** | 21 | 8 | 38% |
| **Core Structures** | 10 | 6 | 60% |
| **Query Builder Methods** | 20 | 19 | 95% |
| **Column Operations** | 15 | 15 | 100% |
| **ActiveModel/Record Operations** | 12 | 7 | 58% |
| **Value Types** | 6 | 2 | 33% |
| **Attributes** | 18 | 6 | 33% |
| **Overall** | 117 | 74 | **63%** |

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
- **Lifeguard Enhancements:** 
  - **PrimaryKeyArity Granularity:** Lifeguard provides granular arity variants (`Tuple2`, `Tuple3`, `Tuple4`, `Tuple5`, `Tuple6Plus`) for better type safety, going beyond SeaORM's simple `Single`/`Tuple` distinction. This enables compile-time verification of composite key sizes and more specific handling.
  - **ValueType Tuple Support:** Full tuple `ValueType` support for composite primary keys (e.g., `(i32, String)`) with proper `Option<T>` unwrapping.
- **Future:** Incremental feature addition based on user needs
