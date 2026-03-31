# SeaORM/SeaQuery → Lifeguard Mapping

## Overview

This document maps SeaORM (v2.0.0-rc.28) and SeaQuery (v0.32.7) components to their Lifeguard equivalents, identifying what exists, what's missing, and future state.

### PRD parity snapshot (schema, validators, scopes, F(), session)

Cross-reference: [PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md](../PRD_SCHEMA_VALIDATORS_SESSION_AND_SCOPES.md). These rows summarize **v0** shipped behavior vs SeaORM-style **vision**; [COMPARISON.md](../../../COMPARISON.md) tracks the same features in table form.

| Capability | Primary API / location | Status | Notes |
|------------|------------------------|--------|-------|
| **Schema inference (DB → Rust)** | `lifeguard-migrate infer-schema`, `compare-schema` (tables + column names for shared tables vs merged SQL), `schema_infer::emit_inferred_rust`, `schema_migration_compare`, `generated_migration_diff::column_map_from_merged_baseline`, `tests/golden/*.expected.rs` | 🟡 **Partial** | PRD §5.7; deterministic emitter golden tests; conservative type mapping; composite PKs emit multiple `#[primary_key]`; `compare-schema` reconciles column **names** (not SQL type/column-def equality); **Deferred:** watch mode / richer CI golden workflows (PRD §5.7a) |
| **Validators** | `run_validators`, `run_validators_with_strategy`, `ValidationStrategy` (FailFast / Aggregate), `ActiveModelBehavior::validate_fields` / `validate_model` / `validation_strategy`, `ActiveModelError::Validation`, `ValidateOp` (Insert / Update / Delete), `#[validate(custom = path)]` on fields, `lifeguard::predicates` | 🟡 **Partial** | PRD §6.7. **Shipped:** field → model order; fail-fast default; aggregate collects field+model errors; delete path runs validators; derive `custom` (`fn(&Value) -> Result<(), String>`). **Predicates:** `string_utf8_chars_max`, `string_utf8_chars_in_range`, `blob_or_string_byte_len_max`, `i64_in_range`, `f64_in_range` on `Value`. **Gap vs SeaORM:** no full built-in attribute matrix (length/range as derive attrs on every type); compose via `validate_fields` + predicates or custom fns. |
| **Scopes** | `SelectQuery::scope`, `scope_or`, `scope_any`, `IntoScope`, `#[scope]` / **`#[scope_bundle]`** on `impl Entity` (`lifeguard::scope`, `lifeguard::scope_bundle`), `src/query/scope.rs` | 🟡 **Partial** | PRD §7.7; AND + OR composition; **`scope_bundle`** ANDs named `#[scope]` helpers into one `scope_*() -> Condition`; soft-delete interaction; **`find_related` does not inherit parent `scope` predicates** — chain `.scope`/`.filter` on the query `find_related` returns; see [DESIGN_FIND_RELATED_SCOPES.md](../DESIGN_FIND_RELATED_SCOPES.md), rustdoc on `query::scope` / `FindRelated`, **`tests/db_integration/related_trait.rs`** (`test_find_related_chains_scope_on_related_query`), **`lifeguard-derive/tests/test_minimal.rs`** (`scope_bundle_and_chains`) |
| **F() expressions** | `ColumnTrait::f_add` / `f_sub` / `f_mul` / `f_div`; `LifeRecord::set_*_expr` + `__update_exprs` on derived `update()`; `Expr::expr` + `ExprTrait` / `order_by_expr` for `WHERE` / `ORDER BY` | 🟡 **Partial** | PRD §8.7; Postgres integration tests in `column_f_update.rs` / `column_f_where.rs`. **PostgreSQL numeric typing:** expressions use SeaQuery `SimpleExpr` arithmetic; the server applies **binary promotion** (e.g. `integer` ± `numeric` → `numeric`). Lifeguard does **not** auto-cast operands—if you need a specific result or storage type (e.g. force `bigint`), align column and RHS types in the query builder or use `Expr::cust` / raw SQL for explicit `::type` casts. See `ColumnTrait::f_add` rustdoc and PRD §8.7. |
| **Session / UoW** | `ModelIdentityMap`, `fingerprint_pk_values`, `mark_dirty` / `mark_dirty_key` / `flush_dirty` / `flush_dirty_with_map_key`, `register_pending_insert` / `promote_pending_to_loaded` / `is_pending_insert_key`, `Session`, `LifeRecord::identity_map_key`, `src/session/` | 🟡 **Partial** | PRD §9.7; identity + dirty flush + insert-only pending keys; `attach_session` auto-dirty when PK set — not full SeaORM session semantics |

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
| `RelationTrait` | ✅ Implemented | ✅ **Complete** | Entity relationships (belongs_to, has_one, has_many, has_many_through) - Trait implemented with join support and automatic join condition generation |
| `Related` | ✅ Implemented | ✅ Complete | Related entity queries - Trait implemented, DeriveRelation macro generates implementations, returns RelationDef for composite key support |
| `FindRelated` | ✅ Implemented | ✅ Complete | Extension trait for finding related entities from model instances - Fixed trait bounds, works correctly with Models |
| `Linked` | ✅ Implemented | ✅ **Complete** | Multi-hop relationship queries - Linked<I, T> trait and FindLinked extension trait implemented |
| `PartialModelTrait` | ✅ Implemented | ✅ **Complete** | Partial model queries (select subset of columns) - Trait implemented, column selection working, DerivePartialModel macro implemented |
| `TryIntoModel` | ✅ Implemented | ✅ Complete | Conversion utilities - Trait for converting types into Model instances. Includes DeriveTryIntoModel macro for auto-generating implementations. Supports field mapping, custom conversions, and missing field handling via Default::default() |

---

## 2. Derive Macros

| SeaORM Macro | Lifeguard Macro | Status | Notes |
|-------------|----------------|--------|-------|
| `DeriveEntity` | `DeriveEntity` | ✅ Implemented | Generates Entity, EntityName, Iden, IdenStatic, LifeModelTrait. Used for nested expansion from LifeModel |
| `DeriveEntityModel` | `LifeModel` | ✅ Implemented | Combined macro (Entity + Model + Column + PrimaryKey + FromRow + ModelTrait) |
| `FromQueryResult` | `FromRow` | ✅ Implemented | Separate derive (matches SeaORM pattern) |
| `DeriveRelation` | ✅ Implemented | ✅ **Complete** | Relation enum with Related trait implementations - Full implementation with composite key support, default column inference, compile-time error checking, duplicate impl deduplication, and `def()` method generation for Relation enum (allows `Relation::Posts.def()` pattern matching SeaORM) |
| `DeriveRelatedEntity` | ✅ Implemented | ✅ **Complete** | RelatedEntity enum - Generated automatically by DeriveRelation macro |
| `DerivePartialModel` | ✅ Implemented | ✅ **Complete** | PartialModelTrait and FromRow implementation - Generates selected_columns() and FromRow from struct fields with column_name attribute support |
| `DeriveLinked` | ✅ Implemented | ✅ **Complete** | Linked enum with Linked trait implementations - Generates `Linked<I, T>` trait implementations from enum variants, reducing boilerplate for multi-hop relationship queries. Supports 2-hop, 3-hop, arbitrary-length paths, self-referential chains, and module-qualified paths. **Competitive advantage:** SeaORM doesn't have this feature |
| `DeriveModel` | ❌ Not Needed | ✅ By Design | LifeModel generates Model struct + ModelTrait impl directly. No separate DeriveModel needed (unlike DeriveEntity which is used for nested expansion of unit struct) |
| `DeriveModelEx` | ❌ Missing | 🔴 **Future** | Complex model with relational fields - **Not needed for migrations** |
| `DeriveActiveModel` | ❌ Missing | 🔴 **Future** | ActiveModel struct (our `LifeRecord` is different) - **Not needed for migrations** |
| `DeriveActiveModelEx` | ❌ Missing | 🔴 **Future** | Complex ActiveModel with relational fields - **Not needed for migrations** |
| `DeriveColumn` | ❌ Not Needed | ✅ By Design | LifeModel generates Column enum + Iden/IdenStatic impls directly |
| `DerivePrimaryKey` | ❌ Not Needed | ✅ By Design | LifeModel generates PrimaryKey enum directly |
| `DeriveIntoActiveModel` | ❌ Missing | 🔴 **Future** | Conversion from Model to ActiveModel - **Not needed for migrations** |
| `DeriveActiveModelBehavior` | ✅ Implemented | ✅ Complete | ActiveModelBehavior trait implementation (default impl generated for all Records) |
| `DeriveActiveEnum` | ❌ Missing | 🟡 **Future** | Enum support for ActiveModel - **Not needed for migrations** |
| `DeriveMigrationName` | ✅ `lifeguard::migration::DeriveMigrationName` + `MigrationName` | ✅ **Implemented** | Unit struct → snake_case `MIGRATION_NAME` + `MigrationName`; pair with manual `Migration` |
| `FromJsonQueryResult` | ❌ Missing | 🟡 **Future** | JSON query result deserialization (JSON column support is ✅ core feature) |
| `DeriveValueType` | ❌ Missing | 🟡 **Future** | ValueType trait for wrapper types - **Not needed for migrations** |
| `DeriveDisplay` | ❌ Missing | 🟡 **Future** | Display trait for ActiveEnum - **Not needed for migrations** |
| `DeriveIden` | ❌ Missing | 🟡 **Future** | Iden trait helper - **Not needed (LifeModel already generates Iden/IdenStatic)** |

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
| `Relation` (enum) | ✅ Implemented | ✅ **Complete** | Entity relationships - Generated by `DeriveRelation` macro with `def()` method for each variant (allows `Relation::Posts.def()` pattern matching SeaORM) |
| `RelationDef` | ✅ Implemented | ✅ Complete | Relation definition - Fully implemented struct with composite key support |
| `ActiveValue` | ✅ Implemented | ✅ Complete | Wrapper for ActiveModel field values (Set, NotSet, Unset variants) |
| `ColumnDef` | ✅ Enhanced | ✅ Complete | Column definition with SQL attributes (via ColumnDefinition::to_column_def()) |
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
| `Model::find_linked<L>()` | `FindLinked::find_linked()` | ✅ Implemented | Find linked entities (via FindLinked trait extension, with DeriveLinked macro for code generation) |
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
| `ValueType` | ✅ Implemented | ✅ **Complete** | Trait for value type conversions - Full implementation with null_value() support for Option<T> |
| `TryGetable` | ✅ Implemented | ✅ **Complete** | Trait for safe value extraction with error handling - Full implementation with ValueExtractionError |
| `TryGetableMany` | ✅ Implemented | ✅ **Complete** | Trait for extracting multiple values - Full implementation for collections |
| `IntoValueTuple` | ✅ Implemented | ✅ **Complete** | Conversion to ValueTuple (for composite keys) - Supports tuples 2-6 and Vec<Value> for 6+ |
| `FromValueTuple` | ✅ Implemented | ✅ **Complete** | Conversion from ValueTuple - Supports tuples 2-6 and Vec<Value> for 6+ |
| `TryFromU64` | ✅ Implemented | ✅ **Complete** | Conversion from u64 (for primary keys) - Full implementation with overflow handling for all integer types |

---

## 8. Attributes & Configuration

| SeaORM Attribute | Lifeguard Attribute | Status | Notes |
|----------------|---------------------|--------|-------|
| `#[sea_orm(table_name = "...")]` | `#[table_name = "..."]` | ✅ Implemented | Table name |
| `#[sea_orm(primary_key)]` | `#[primary_key]` | ✅ Implemented | Primary key field |
| `#[sea_orm(auto_increment = bool)]` | `#[auto_increment]` | ✅ Complete | Auto-increment field - LifeModel macro generates ColumnTrait::def() with auto_increment metadata |
| `#[sea_orm(column_name = "...")]` | `#[column_name = "..."]` | ✅ Implemented | Custom column name |
| `#[sea_orm(column_type = "...")]` | `#[column_type = "..."]` | ✅ Complete | Custom column type - LifeModel macro generates ColumnTrait::def() with column_type metadata |
| `#[sea_orm(nullable)]` | `#[nullable]` | ✅ Implemented | Nullable field |
| `#[sea_orm(default_value = ...)]` | `#[default_value = ...]` | ✅ Complete | Default value - LifeModel macro generates ColumnTrait::def() with default_value metadata |
| `#[sea_orm(unique)]` | `#[unique]` | ✅ Complete | Unique constraint - LifeModel macro generates ColumnTrait::def() with unique metadata |
| `#[sea_orm(indexed)]` | `#[indexed]` | ✅ Complete | Indexed column - LifeModel macro generates ColumnTrait::def() with indexed metadata |
| `#[sea_orm(enum_name = "...")]` | `#[enum_name = "..."]` | ✅ Complete | Enum type name - LifeModel macro generates ColumnTrait::enum_type_name() implementation |
| `#[sea_orm(default_expr = "...")]` | `#[default_expr = "..."]` | ✅ Implemented | Default SQL expression - LifeModel macro generates ColumnTrait::def() with default_expr metadata, includes apply_default_expr() helper for migrations |
| `#[sea_orm(schema_name = "...")]` | `#[schema_name = "..."]` | ✅ Implemented | Schema name - LifeModel macro generates schema_name() method on Entity, query builders use schema-qualified table names |
| `#[sea_orm(ignore)]` | `#[skip]` | ✅ Implemented | Ignore field - Fields with `#[skip]` are excluded from Column enum and database operations but remain in Model struct |
| `#[sea_orm(select_as = "...")]` | `#[select_as = "..."]` | ✅ Implemented | Custom SELECT expression - Metadata stored in ColumnDefinition, ready for query builder integration |
| `#[sea_orm(save_as = "...")]` | `#[save_as = "..."]` | ✅ Implemented | Custom save expression - Metadata stored in ColumnDefinition, ready for CRUD operations integration |
| `#[sea_orm(renamed_from = "...")]` | `#[renamed_from = "..."]` | ✅ Implemented | Column renamed from - LifeModel macro generates ColumnTrait::def() with renamed_from metadata for migration workflows |
| `#[sea_orm(comment = "...")]` | `#[comment = "..."]` | ✅ Implemented | Column comment - Metadata stored in ColumnDefinition for documentation and schema introspection |
| `#[sea_orm(foreign_key = "...")]` | ❌ Missing | 🔴 **Migration Blocker** | Foreign key constraint - **CRITICAL for entity-driven migrations** - Need to support `foreign_key = "table(column) ON DELETE action"` |
| `#[sea_orm(check = "...")]` | ❌ Missing | 🔴 **Migration Blocker** | CHECK constraint - **CRITICAL for entity-driven migrations** - Need to support table-level and column-level CHECK constraints |
| `#[sea_orm(composite_unique = [...])]` | ❌ Missing | 🔴 **Migration Blocker** | Composite unique constraint - **CRITICAL for entity-driven migrations** - Need to support multi-column unique constraints |
| `#[sea_orm(table_comment = "...")]` | ❌ Missing | 🟡 **Migration Enhancement** | Table comment (COMMENT ON TABLE) - **Nice-to-have for migrations** - Metadata for documentation |
| `#[sea_orm(index = "...")]` | ❌ Missing | 🔴 **Migration Blocker** | Custom index definition - **CRITICAL for entity-driven migrations** - Need to support composite indexes, partial indexes, unique indexes |

---

## 9. Future State Descriptions

### High Priority (Core Functionality)

#### ModelTrait
**Status:** ✅ Implemented  
**Current State:** Trait for Model-level operations:
- `get(column)` - Get column value as `Value` ✅
- `set(column, value)` - Set column value ✅
- `get_primary_key_value()` - Get primary key value(s) ✅
- `get_value_type(column)` - Get column's value type ✅ **Complete** - Returns Rust type string (e.g., `"i32"`, `"String"`, `"Option<i32>"`) for runtime type introspection
- `find_related<R>()` - ✅ Implemented (via FindRelated trait extension) - Fixed trait bounds, works correctly with Models
- `find_linked<I, T>()` - Find linked entities ✅ (Implemented via FindLinked trait extension)

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
**Status:** ✅ Complete  
**Current State:**
- `RelationTrait` - ✅ Implemented with functional query building (belongs_to, has_one, has_many, has_many_through methods accept foreign keys and join conditions)
- `join_condition()` helper function - ✅ Implemented (creates join conditions from table/column names)
- All relationship methods build actual queries with LEFT JOIN clauses
**Current State:**
- `Related` - ✅ Implemented (trait for defining relationships)
- `FindRelated` - ✅ Implemented (extension trait providing `find_related()` method on models) - Fixed impossible trait bound, fully functional
- `DeriveRelation` - ✅ Implemented (macro generates Related trait implementations from Relation enum, with duplicate impl deduplication to prevent trait coherence violations when multiple relations target the same entity)
- `RelationMetadata` - ✅ Implemented (trait for storing relationship metadata, generated by DeriveRelation when from/to columns are provided)
- `Identity` - ✅ Implemented (enum for single and composite column references: Unary, Binary, Ternary, Many)
- `RelationDef` - ✅ Implemented (struct containing all relationship metadata including Identity for composite keys)
- `get_primary_key_identity()` - ✅ Implemented (ModelTrait method returning Identity for single/composite keys)
- `get_primary_key_values()` - ✅ Implemented (ModelTrait method returning Vec<Value> for all primary key values)
**Implementation Status:**
- ✅ Single key relationships fully supported
- ✅ Composite key relationships fully supported (Binary, Ternary, Many variants)
- ✅ `find_related()` uses `RelationDef` and `build_where_condition()` for both single and composite keys
- ✅ `DeriveRelation` macro generates `RelationDef` with proper `Identity` construction
- ✅ `DeriveRelation` macro deduplicates Related and From impls when multiple relations target the same entity (prevents trait coherence violations)
- ✅ `LifeModel` macro generates `get_primary_key_identity()` and `get_primary_key_values()` for all key types
**Future State:**
- Enhanced error messages for invalid column references in DeriveRelation macro ✅ (Completed - comprehensive validation added)
- Support for has_many_through relationships ✅ (Completed - DeriveRelation macro supports has_many_through with through attribute)
- Automatic join condition generation from foreign key metadata ✅ (Completed - RelationDef::join_on_expr() and convenience methods)
- `Linked` - Multi-hop relationship queries ✅ (Completed - Linked<I, T> trait and FindLinked extension trait)
- `DeriveLinked` - Generate Linked trait implementations ✅ (Completed - DeriveLinked macro generates `Linked<I, T>` impls from enum variants, reducing boilerplate by 80%+)
- `DeriveRelatedEntity` - Generate RelatedEntity enum ✅ (Completed - automatically generated by DeriveRelation macro)
- Eager loading support ✅ (Completed - load_related() function with selectinload strategy, FK extraction, and grouping)
- Lazy loading support ✅ (Completed - LazyLoader struct with on-demand query execution)
- `Relation::def()` method for Relation enum ✅ **Completed** - Generate `impl Relation` with `def()` method that returns `RelationDef` for each variant (matches SeaORM pattern: `Relation::Posts.def()`)

#### Partial Models
**Status:** ✅ Complete  
**Current State:**
- `PartialModelTrait` - ✅ Implemented (trait for partial models with `selected_columns()` method returning `Vec<&'static str>`)
- `PartialModelBuilder` - ✅ Implemented (trait for building partial model queries)
- `SelectPartialQuery` - ✅ Implemented (query builder for partial models)
- `select_partial()` method - ✅ Implemented (on `SelectQuery<E>`) - Uses column names directly with SeaQuery
- `DerivePartialModel` - ✅ Implemented (macro generates PartialModelTrait and FromRow implementations)
**Known Limitations:**
- `select_partial()` replaces the entire query, which means WHERE/ORDER BY/etc. clauses from before `select_partial()` are lost. Users should call `select_partial()` early in the query chain, before adding filters/ordering.
  - **Root Cause:** sea-query's `SelectStatement` doesn't expose clause getters or column replacement methods
  - **Tracking:** See `SEAQUERY_IMPROVEMENTS_AUDIT.md` for details and potential contributions
- Column order must match between `selected_columns()` and `FromRow` implementation (enforced by macro)
**Future Enhancements:**
- Preserve existing query clauses (WHERE, ORDER BY, etc.) when calling `select_partial()`
  - **Blocked by:** sea-query API limitations (see `SEAQUERY_IMPROVEMENTS_AUDIT.md`)

#### Advanced Query Features
**Status:** ✅ **Complete**  
**Current State:**
- `group_by()`, `having()` - ✅ Implemented (GROUP BY and HAVING clauses)
- `join()`, `left_join()`, `right_join()`, `inner_join()` - ✅ Implemented (JOIN operations)
- `with()` - ✅ Implemented (CTEs using WITH clauses, returns `WithQuery`)
- `subquery_column()` - ✅ Implemented (Subqueries as SELECT columns)
- `window_function_cust()` - ✅ Implemented (Window functions using `Expr::cust()` for SQL expressions)

### Low Priority (Nice-to-Have)

#### Value Type System
**Status:** 🟡 Future  
**Future State:** Enhanced value type system:
- `ValueType` trait for custom value types
- `TryGetable` and `TryGetableMany` for safe value extraction
- `IntoValueTuple` and `FromValueTuple` for composite keys
- `TryFromU64` for primary key conversions

#### Migration Support
**Status:** ✅ **READY FOR IMPLEMENTATION**  
**Current State:**
- ✅ All core migration infrastructure is implemented
- ✅ Phase 1 attributes (`default_expr`, `renamed_from`, `schema_name`) are complete
- ✅ `ColumnDefinition::to_column_def()` - Converts to SeaQuery ColumnDef for migrations
- ✅ `ColumnDefinition::apply_default_expr()` - Applies default SQL expressions
- ✅ `ColumnDefinition::comment_sql()` - Generates COMMENT ON COLUMN SQL
- ✅ Type mapping for all common column types
- ✅ Entity metadata (`table_name()`, `schema_name()`, `all_columns()`)

**Note:** The missing derive macros listed above are **NOT prerequisites** for migrations. See `MIGRATION_PREREQUISITES_DISCOVERY.md` for detailed analysis.

**Future State:**
- `DeriveMigrationName` — **shipped:** `lifeguard::migration::DeriveMigrationName` + `MigrationName` trait (`MIGRATION_NAME` constant)
- Migration CLI tool - Integration with migration tools

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
| **Core Traits** | 15 | 10 | 67% (Enhanced: PrimaryKeyArity with granular variants, PartialModelTrait, Related, Linked, FindLinked implemented) |
| **Derive Macros** | 21 | 9 | 43% (Added: DeriveRelatedEntity) |
| **Core Structures** | 10 | 6 | 60% |
| **Query Builder Methods** | 20 | 19 | 95% |
| **Column Operations** | 15 | 15 | 100% |
| **ActiveModel/Record Operations** | 12 | 7 | 58% |
| **Value Types** | 6 | 2 | 33% |
| **Attributes** | 18 | 6 | 33% |
| **Overall** | 117 | 78 | **67%** |

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
- **Lifeguard:** ✅ Complete relationship system with `RelationTrait`, `Related`, `FindRelated`, `Linked`, `FindLinked`, eager loading, lazy loading, and `DeriveRelatedEntity`

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

---

## 13. Implementation Notes

### RelationMetadata Trait Bound Limitation

**Issue:** The `RelationMetadata` trait is generated by `DeriveRelation` macro when `from`/`to` columns are specified, but it cannot be used in `find_related()` due to Rust's trait bound system.

**Root Cause:**
- `find_related()` is defined in the `FindRelated` trait with a specific signature: `fn find_related<R>() -> SelectQuery<R> where R: LifeModelTrait, Self::Entity: Related<R>` - Fixed to use correct relationship direction (Self::Entity -> R) and removed impossible LifeModelTrait bound on Self
- To use `RelationMetadata`, we would need to add `R: RelationMetadata<Self::Entity>` to the trait bound
- However, this would make `RelationMetadata` a **required** trait bound, breaking all existing code that doesn't implement it
- Rust doesn't support "optional" trait bounds - you can't conditionally use a trait method based on whether it's implemented

**Potential Solutions:**
1. **Default Trait Implementation Pattern**: Make `RelationMetadata` always return `None` by default, and only override it when metadata is available. However, we still can't call it without the trait bound.

2. **Associated Constants**: Use associated constants instead of trait methods to store metadata. This avoids trait bounds but requires different syntax:
   ```rust
   trait RelationMetadata<R> {
       const FOREIGN_KEY_COLUMN: Option<&'static str> = None;
   }
   ```
   This allows accessing `R::FOREIGN_KEY_COLUMN` without trait bounds, but constants can't be overridden per implementation.

3. **Type-Level Metadata**: Use const generics or type-level programming to encode metadata at compile time. Complex and may not be worth it.

4. **Separate Trait for Metadata**: Create a separate trait that's only required when metadata is needed, but this still requires trait bounds.

5. **Runtime Metadata Lookup**: Store metadata in a static HashMap or similar structure, keyed by `TypeId`. Requires `std::any::TypeId` and has runtime overhead:
   ```rust
   static RELATION_METADATA: Lazy<HashMap<(TypeId, TypeId), &'static str>> = Lazy::new(|| {
       // Populated by macro-generated code
   });
   ```

**Recommended Approach:** Use **associated constants with a default implementation pattern** or a **static metadata registry**. The registry approach is more flexible and doesn't require trait bounds.

### Composite Primary Key Support

**Issue:** `find_related()` currently only supports single-column primary keys. Composite primary keys require matching multiple foreign key columns.

**Root Cause:**
- `get_primary_key_value()` only returns a single `Value`, not a tuple or collection
- We can't enumerate `PrimaryKey` enum variants at runtime (Rust doesn't support enum variant iteration)
- We need to know which foreign key columns correspond to which primary key columns
- Even with `PrimaryKeyArityTrait`, we can't get individual primary key values without knowing the variants

**Potential Solutions:**
1. **Relationship Metadata**: Use `RelationMetadata` to specify all foreign key columns for composite keys. Still blocked by trait bound limitation above.

2. **PrimaryKeyTrait Enhancement**: Add a method to get all primary key values as a collection:
   ```rust
   trait PrimaryKeyTrait {
       fn get_all_values(&self) -> Vec<Value>; // For composite keys
   }
   ```
   Requires changes to `ModelTrait` and macro generation to support this.

3. **Type-Level Metadata**: Encode composite key structure at compile time using const generics or associated types. Very complex.

4. **Helper Trait**: Create a `CompositeKeyMetadata` trait that provides foreign key column names for each primary key column:
   ```rust
   trait CompositeKeyMetadata<R> {
       fn foreign_key_columns() -> Vec<&'static str>;
   }
   ```
   Still requires trait bounds.

5. **Macro-Generated Helper Functions**: Generate helper functions alongside the entity that return all primary key values:
   ```rust
   // Generated by macro
   impl UserModel {
       fn get_all_primary_key_values(&self) -> Vec<Value> {
           vec![self.id.into(), self.tenant_id.into()]
       }
   }
   ```
   This requires changes to the macro but avoids trait bound issues.

**Implemented Solution:** Enhanced `ModelTrait` with `get_primary_key_identity()` and `get_primary_key_values()` methods, and `RelationDef` pattern (replacing static registry) to handle composite keys:
1. ✅ `ModelTrait` enhanced with `get_primary_key_identity()` returning `Identity` enum
2. ✅ `ModelTrait` enhanced with `get_primary_key_values()` returning `Vec<Value>`
3. ✅ `LifeModel` macro generates both methods for single and composite keys
4. ✅ `RelationDef` struct contains `Identity` for both `from_col` and `to_col`
5. ✅ `build_where_condition()` uses `get_primary_key_values()` to build WHERE clauses
6. ✅ `DeriveRelation` macro generates `RelationDef` with proper `Identity` construction
7. ✅ Comprehensive test coverage for all key types and edge cases

**Design Document:** See [DESIGN_RELATION_METADATA_AND_COMPOSITE_KEYS.md](./DESIGN_RELATION_METADATA_AND_COMPOSITE_KEYS.md) for detailed implementation architecture, design decisions, and step-by-step guide.

---

## 14. Implementation Priority Plan: Value Types & Attributes

### Executive Summary

**Highest Impact: Attributes & Configuration (Section 222-229)**  
**Lower Priority: Value Types & Conversions (Section 199-205)**

The **Attributes & Configuration** section has **significantly higher impact** on delivering README promises, particularly for:
1. **Migrations** (promised but partially implemented)
2. **PostgreSQL schema support** (critical for production)
3. **Column management** (needed for real-world applications)

The **Value Types & Conversions** section provides infrastructure improvements but doesn't block core functionality since composite keys already work.

---

### Priority Analysis

#### Section 1: Value Types & Conversions (199-205) ✅ **COMPLETE**

| Feature | Priority | Impact on README Promises | Current Status |
|---------|----------|---------------------------|----------------|
| `IntoValueTuple` | 🟡 **Low** | Minimal - Composite keys already work | ✅ **Complete** |
| `FromValueTuple` | 🟡 **Low** | Minimal - Composite keys already work | ✅ **Complete** |
| `ValueType` | 🟡 **Medium** | Developer experience improvement | ✅ **Complete** |
| `TryGetable` | 🟡 **Medium** | Better error handling, not blocking | ✅ **Complete** |
| `TryGetableMany` | 🟡 **Low** | Convenience feature | ✅ **Complete** |
| `TryFromU64` | 🟡 **Low** | Minor convenience for primary keys | ✅ **Complete** |

**Overall Assessment:** 
- ✅ **All value type infrastructure features fully implemented** (Phase 4 complete)
- ✅ **Composite keys fully supported** via `get_primary_key_identity()` and `get_primary_key_values()`
- ✅ **Type-safe conversions** available for all supported Rust types
- ✅ **Comprehensive error handling** with `ValueExtractionError`
- **Impact:** Medium - Significantly improves developer experience and type safety

#### Section 2: Attributes & Configuration (222-229)

| Feature | Priority | Impact on README Promises | Current Status |
|---------|----------|---------------------------|----------------|
| `default_expr` | 🔴 **CRITICAL** | **Migrations** (promised) | ✅ **Complete** - Parsed, stored, `apply_default_expr()` method available for migrations |
| `schema_name` | 🔴 **CRITICAL** | **PostgreSQL Features** (promised) | ✅ **Complete** - Parsed, used in entity generation, query builders use schema-qualified names |
| `renamed_from` | 🔴 **CRITICAL** | **Migrations** (promised) | ✅ **Complete** - Parsed, stored in ColumnDefinition for migration workflows |
| `ignore` / `skip` | 🟠 **HIGH** | **ORM Features** (promised) | ✅ **Complete** - Fields excluded from Column enum and database operations |
| `select_as` | 🟠 **HIGH** | **Query Builder** (promised) | ✅ **Complete** - Integrated into SELECT query building, uses custom expressions when specified |
| `save_as` | 🟠 **HIGH** | **CRUD Operations** (promised) | ✅ **Complete** - Integrated into INSERT and UPDATE operations, uses custom expressions when specified |
| `comment` | 🟡 **MEDIUM** | **Developer Experience** (promised) | ✅ **Complete** - `comment_sql()` helper method generates COMMENT ON COLUMN SQL for migrations |

**Overall Assessment:**
- ✅ **Core Migration Attributes Complete** - `default_expr`, `schema_name`, `renamed_from` fully implemented
- ✅ **Field Exclusion Complete** - `ignore`/`skip` fully functional
- ✅ **Query/CRUD Integration Complete** - `select_as` and `save_as` fully integrated into query building and CRUD operations
- ✅ **Documentation Support Complete** - `comment` integrated with `comment_sql()` helper for migration builders
- **Impact:** **HIGH** - All attribute features complete and fully functional

---

### Recommended Implementation Order

#### Phase 1: Critical Migration Attributes (Week 1-2)

**Priority: 🔴 CRITICAL - Blocks Migrations Promise**

1. **`default_expr`** - Default SQL expression
   - **Why:** Essential for migrations with SQL expressions (`NOW()`, `uuid_generate_v4()`, `gen_random_uuid()`)
   - **Impact:** Enables promised "programmatic migrations" with real-world use cases
   - **Complexity:** Medium (requires SQL expression parsing/storage)
   - **Dependencies:** None

2. **`renamed_from`** - Column renamed from
   - **Why:** Critical for migration workflows (column renames are common)
   - **Impact:** Enables promised "advanced migration operations"
   - **Complexity:** Low (metadata storage)
   - **Dependencies:** None

3. **`schema_name`** - Schema name
   - **Why:** Critical for PostgreSQL production deployments (multi-tenant, organization)
   - **Impact:** Enables promised "PostgreSQL Features" (schema support)
   - **Complexity:** Low (metadata storage, table name generation)
   - **Dependencies:** None

**Deliverable:** Complete migration support for promised features

#### Phase 2: Advanced ORM Attributes (Week 3-4)

**Priority: 🟠 HIGH - Enables Advanced Features**

4. **`ignore`** - Ignore field
   - **Why:** Needed for computed columns, virtual fields, fields not in database
   - **Impact:** Enables promised "ORM Features" completeness
   - **Complexity:** Low (macro filtering)
   - **Dependencies:** None

5. **`select_as`** - Custom SELECT expression
   - **Why:** Needed for computed columns, virtual columns, custom SELECT expressions
   - **Impact:** Enables promised "Query Builder" advanced features
   - **Complexity:** Medium (SQL expression handling in query builder)
   - **Dependencies:** None

6. **`save_as`** - Custom save expression
   - **Why:** Needed for computed columns on write, custom save logic
   - **Impact:** Enables promised "CRUD Operations" completeness
   - **Complexity:** Medium (SQL expression handling in insert/update)
   - **Dependencies:** None

**Deliverable:** Advanced ORM features enabled

#### Phase 3: Developer Experience (Week 5)

**Priority: 🟡 MEDIUM - Nice-to-Have**

7. **`comment`** - Column comment
   - **Why:** Documentation, schema introspection
   - **Impact:** Improves developer experience
   - **Complexity:** Low (metadata storage)
   - **Dependencies:** None

**Deliverable:** Better documentation support

#### Phase 4: Value Type Infrastructure (Week 6-8) ✅ **COMPLETE**

**Priority: 🟡 LOW - Optimization**

8. **`ValueType`** - Trait for value type conversions ✅ **COMPLETE**
   - **Why:** Better type safety, developer experience
   - **Impact:** Improves type system, not blocking
   - **Complexity:** Medium (trait design, macro integration)
   - **Dependencies:** None
   - **Status:** ✅ Fully implemented with `null_value()` support for Option<T>

9. **`TryGetable`** - Trait for safe value extraction ✅ **COMPLETE**
   - **Why:** Better error handling
   - **Impact:** Improves error messages, not blocking
   - **Complexity:** Medium (trait design, error types)
   - **Dependencies:** ValueType (optional)
   - **Status:** ✅ Fully implemented with ValueExtractionError

10. **`TryGetableMany`** - Trait for extracting multiple values ✅ **COMPLETE**
    - **Why:** Convenience for batch operations
    - **Impact:** Minor convenience, not blocking
    - **Complexity:** Low (extends TryGetable)
    - **Dependencies:** TryGetable
    - **Status:** ✅ Fully implemented for collections

11. **`TryFromU64`** - Conversion from u64 ✅ **COMPLETE**
    - **Why:** Convenience for primary keys
    - **Impact:** Minor convenience, not blocking
    - **Complexity:** Low (trait implementation)
    - **Dependencies:** None
    - **Status:** ✅ Fully implemented with overflow handling for all integer types

12. **`IntoValueTuple` / `FromValueTuple`** - Composite key conversions ✅ **COMPLETE**
    - **Why:** Optimization for composite keys (already work without these)
    - **Impact:** Performance optimization, not blocking
    - **Complexity:** Medium (trait design, tuple handling)
    - **Dependencies:** None
    - **Status:** ✅ Fully implemented for tuples 2-6 and Vec<Value> for 6+

**Deliverable:** ✅ Enhanced type system and developer experience - **COMPLETE**

---

### Impact Summary

#### Attributes & Configuration (222-229)
- **Blocks:** Migrations (promised), PostgreSQL schema support (promised)
- **Enables:** Advanced ORM features (promised)
- **Impact Score:** 🔴 **9/10** (Critical)

#### Value Types & Conversions (199-205) ✅ **COMPLETE**
- **Blocks:** Nothing (composite keys already work)
- **Enables:** Better developer experience, optimizations
- **Impact Score:** 🟡 **3/10** (Low)
- **Status:** ✅ All value type traits implemented and tested (ValueType, TryGetable, TryGetableMany, IntoValueTuple, FromValueTuple, TryFromU64)

---

### Recommendation

**Implement Attributes & Configuration FIRST** (Phases 1-3, Weeks 1-5)

**Rationale:**
1. ✅ **Directly blocks promised features** (Migrations, PostgreSQL schema support)
2. ✅ **High user impact** - Needed for production deployments
3. ✅ **Enables real-world use cases** - Multi-tenant apps, computed columns, migrations
4. ✅ **Lower complexity** - Mostly metadata storage and macro changes
5. ✅ **Clear deliverables** - Each phase delivers tangible value

**Defer Value Types & Conversions** (Phase 4, Weeks 6-8)

**Rationale:**
1. ✅ **Not blocking** - Composite keys already work
2. ✅ **Optimization focus** - Improves developer experience but doesn't enable new features
3. ✅ **Lower priority** - Can be added incrementally without breaking changes
4. ✅ **Better to ship core features first** - Migrations and schema support are more critical

---

### Success Metrics

#### Phase 1 Success (Critical Migration Attributes)
- ✅ Can create migrations with SQL default expressions (`NOW()`, `uuid_generate_v4()`)
- ✅ Can rename columns during migrations
- ✅ Can use PostgreSQL schemas (multi-tenant support)
- ✅ README "Migrations" status: 🟡 Partial → ✅ Implemented

#### Phase 2 Success (Advanced ORM Attributes)
- ✅ Can ignore fields not in database
- ✅ Can use computed columns in SELECT queries
- ✅ Can use custom save expressions
- ✅ README "ORM Features" status: 🟡 67% → 🟡 75%+

#### Phase 3 Success (Developer Experience)
- ✅ Column comments stored and accessible
- ✅ Better schema introspection support

#### Phase 4 Success (Value Type Infrastructure)
- ✅ Better type safety with ValueType trait
- ✅ Improved error messages with TryGetable
- ✅ Performance optimizations for composite keys

---

### Implementation Notes

#### For Attributes (Phases 1-3)

**Macro Changes Required:**
- `lifeguard-derive/src/macros/life_model.rs` - Parse new attributes
- `lifeguard-derive/src/attributes.rs` - Store attribute metadata
- `src/query/column/definition.rs` - Use attributes in ColumnDefinition
- `src/query/column/column_trait.rs` - Implement select_as() and save_as()

**Testing Required:**
- Migration tests with `default_expr`
- Schema name tests
- Column rename tests
- Computed column tests
- Ignore field tests

#### For Value Types (Phase 4)

**Trait Design Required:**
- `src/value/types.rs` - ValueType trait
- `src/value/try_getable.rs` - TryGetable trait
- `src/value/tuple.rs` - IntoValueTuple/FromValueTuple traits
- Macro integration for auto-implementations

**Testing Required:**
- Type conversion tests
- Error handling tests
- Composite key optimization tests

---

### Conclusion

**Attributes & Configuration (222-229) should be prioritized** because they:
1. Block promised features (Migrations, PostgreSQL schema support)
2. Enable real-world production use cases
3. Have clear, measurable deliverables
4. Are needed for the "Migrations" promise in README

**Value Types & Conversions (199-205) can be deferred** because they:
1. Don't block any promised features
2. Are optimizations, not requirements
3. Composite keys already work without them
4. Can be added incrementally later

**Recommended Timeline:**
- **Weeks 1-5:** Attributes & Configuration (Phases 1-3)
- **Weeks 6-8:** Value Types & Conversions (Phase 4) - if time permits

This prioritization maximizes impact on README promises while delivering value incrementally.

---

## 15. ORM Equivalents: SQL Views & Stored Procedures

### Overview

This section explains how SQL Views and Stored Procedures map to ORM patterns, and how Lifeguard can support them.

---

### SQL Views → ORM Equivalents

#### What are SQL Views?

SQL Views are **virtual tables** based on the result of a SQL query. They:
- Don't store data (except materialized views)
- Provide a query interface (SELECT only, typically)
- Can simplify complex queries
- Can provide security/abstraction layers

#### ORM Equivalents

| SQL Concept | ORM Equivalent | Lifeguard Status | Implementation Approach |
|-------------|----------------|-------------------|------------------------|
| **Regular View** | **Read-only Model** | 🟡 **Future** | Model backed by SELECT query, no write operations |
| **Materialized View** | **Cached Query Model** | 🟡 **Future** | Model backed by materialized view table, refresh support |
| **View with JOINs** | **Query-based Model** | ✅ **Partial** | Use query builder with joins, map to struct |
| **View with Aggregations** | **Projection/Partial Model** | ✅ **Implemented** | `DerivePartialModel` for selected columns |
| **View as Security Layer** | **Scoped Queries** | 🟡 **Partial** | `SelectQuery::scope` / `IntoScope` + `#[scope]` (`lifeguard::scope`) + entity helpers returning `IntoCondition` (`src/query/scope.rs`) |

#### Implementation Patterns

**Pattern 1: Read-only Model (Regular View)**
```rust
// SQL: CREATE VIEW user_stats AS SELECT user_id, COUNT(*) as post_count FROM posts GROUP BY user_id;

#[derive(LifeModel)]
#[table_name = "user_stats"]  // Points to view, not table
#[read_only]  // Future attribute - prevents insert/update/delete
pub struct UserStats {
    pub user_id: i32,
    pub post_count: i64,
}

// Usage - works like normal model, but only SELECT
let stats = UserStats::find().all(&executor)?;
```

**Pattern 2: Query-based Model (Complex View)**
```rust
// SQL: CREATE VIEW active_users AS SELECT u.* FROM users u WHERE u.is_active = true;

#[derive(LifeModel)]
#[table_name = "active_users"]
pub struct ActiveUser {
    // Same fields as User model
}

// OR use query builder directly
let active_users = User::find()
    .filter(User::IsActive.eq(true))
    .all(&executor)?;
```

**Pattern 3: Materialized View**
```rust
// SQL: CREATE MATERIALIZED VIEW user_summary AS SELECT ...;

#[derive(LifeModel)]
#[table_name = "user_summary"]
#[materialized_view]  // Future attribute
pub struct UserSummary {
    // Fields
}

// Refresh materialized view
UserSummary::refresh_materialized_view(&executor)?;

// Then query normally
let summaries = UserSummary::find().all(&executor)?;
```

**Pattern 4: Partial Model (View-like Projection)**
```rust
// SQL: SELECT id, name, email FROM users;  (not all columns)

#[derive(DerivePartialModel)]
pub struct UserBasic {
    pub id: i32,
    pub name: String,
    pub email: String,
}

// Query with partial model
let users = User::find()
    .select_partial::<UserBasic>()
    .all(&executor)?;
```

#### Current Lifeguard Support

✅ **What Works Now:**
- Query builder with joins (can simulate views)
- Partial models (`DerivePartialModel`) for column selection
- Raw SQL queries (can query views directly)
- Type-safe models (can map view results to structs)

🟡 **What's Missing:**
- `#[read_only]` attribute to prevent writes to views
- `#[materialized_view]` attribute for refresh support
- Automatic view detection (schema introspection)
- View-specific query optimizations

---

### Stored Procedures → ORM Equivalents

#### What are Stored Procedures?

Stored Procedures are **pre-compiled SQL code** stored in the database. They:
- Encapsulate business logic in the database
- Can accept parameters
- Can return result sets or single values
- Can perform complex operations (transactions, loops, etc.)

#### ORM Equivalents

| SQL Concept | ORM Equivalent | Lifeguard Status | Implementation Approach |
|-------------|----------------|-------------------|------------------------|
| **Stored Procedure** | **Raw SQL Execution** | ✅ **Implemented** | `execute_statement()` or `find_by_statement()` |
| **Function (returns value)** | **Query Value Helper** | ✅ **Implemented** | `query_value()` for single values |
| **Function (returns table)** | **Query with Model Mapping** | ✅ **Implemented** | `find_all_by_statement()` + `FromRow` |
| **Procedure with Business Logic** | **Repository Pattern** | 🟡 **Future** | Model Managers (promised but not implemented) |
| **Database Function** | **Custom Query Methods** | 🟡 **Future** | Model Managers or extension traits |

#### Implementation Patterns

**Pattern 1: Raw SQL Execution (Current)**
```rust
// SQL: CREATE FUNCTION get_user_stats(user_id INT) RETURNS TABLE(...) AS $$

use lifeguard::{execute_statement, find_all_by_statement, LifeExecutor};

// Call stored procedure/function
let rows = find_all_by_statement(
    &executor,
    "SELECT * FROM get_user_stats($1)",
    &[&42i64]
)?;

// Map to model
let stats: Vec<UserStats> = rows.iter()
    .map(|row| UserStats::from_row(row))
    .collect();
```

**Pattern 2: Query Value (Single Return)**
```rust
// SQL: CREATE FUNCTION count_active_users() RETURNS INT AS $$

use lifeguard::{query_value, LifeExecutor};

let count: i64 = query_value(
    &executor,
    "SELECT count_active_users()",
    &[]
)?;
```

**Pattern 3: Repository Pattern (Future)**
```rust
// Model Manager with stored procedure wrapper
impl User {
    pub fn get_stats(&self, executor: &dyn LifeExecutor) -> Result<UserStats, LifeError> {
        find_by_statement(
            executor,
            "SELECT * FROM get_user_stats($1)",
            &[&self.id]
        )?
        .try_into()
    }
    
    pub fn refresh_cache(executor: &dyn LifeExecutor) -> Result<(), LifeError> {
        execute_statement(executor, "CALL refresh_user_cache()", &[])?;
        Ok(())
    }
}
```

**Pattern 4: Extension Trait (Future)**
```rust
// Custom trait for database functions
trait UserFunctions {
    fn get_stats(&self, executor: &dyn LifeExecutor) -> Result<UserStats, LifeError>;
    fn calculate_score(&self, executor: &dyn LifeExecutor) -> Result<f64, LifeError>;
}

impl UserFunctions for User {
    fn get_stats(&self, executor: &dyn LifeExecutor) -> Result<UserStats, LifeError> {
        // Implementation using raw SQL
    }
}
```

#### Current Lifeguard Support

✅ **What Works Now:**
- Raw SQL execution (`execute_statement()`, `execute_unprepared()`)
- Parameterized queries (`find_by_statement()`, `find_all_by_statement()`)
- Single value queries (`query_value()`)
- Result mapping (can use `FromRow` to map results to models)

🟡 **What's Missing:**
- Model Managers (promised but not implemented)
- Type-safe stored procedure wrappers
- Automatic parameter binding for procedures
- Procedure result set type inference

---

### Can ORMs Deliver This Functionality?

**Yes, absolutely!** ORMs can deliver both Views and Stored Procedures support:

#### Views Support

**✅ Fully Deliverable:**
1. **Read-only Models** - Models that map to views, prevent writes
2. **Query-based Models** - Models backed by SELECT queries
3. **Materialized Views** - Models with refresh capabilities
4. **View Queries** - Query builder can target views

**Implementation Complexity:**
- **Low-Medium** - Mostly metadata and attribute handling
- Requires `#[read_only]` attribute support
- Requires materialized view refresh methods
- Can leverage existing query builder

#### Stored Procedures Support

**✅ Fully Deliverable:**
1. **Raw SQL Execution** - ✅ Already implemented
2. **Parameter Binding** - ✅ Already implemented
3. **Result Mapping** - ✅ Already implemented (via `FromRow`)
4. **Type-safe Wrappers** - 🟡 Future (Model Managers)

**Implementation Complexity:**
- **Low** - Raw SQL already works
- **Medium** - For type-safe wrappers (Model Managers)
- Can be enhanced with convenience methods

---

### Recommended Implementation for Lifeguard

#### Phase 1: View Support (Medium Priority)

**Features:**
1. `#[read_only]` attribute - Prevents insert/update/delete on view-backed models
2. `#[materialized_view]` attribute - Marks materialized views
3. `refresh_materialized_view()` method - Refreshes materialized views
4. View detection in schema introspection (future)

**Impact:** Enables promised "Views, materialized views" in README

**Complexity:** Medium (attribute parsing, write prevention, refresh method)

#### Phase 2: Stored Procedure Enhancements (Low Priority)

**Features:**
1. Model Managers - Custom query methods (already promised)
2. Type-safe procedure wrappers - Convenience methods
3. Procedure parameter helpers - Easier parameter binding

**Impact:** Improves developer experience for stored procedures

**Complexity:** Low-Medium (Model Managers are already planned)

---

### Conclusion

**Views:**
- ✅ **Can be delivered** via read-only models and query-based models
- 🟡 **Partially supported** - Query builder works, need read-only attribute
- 📋 **Recommended:** Implement `#[read_only]` and `#[materialized_view]` attributes

**Stored Procedures:**
- ✅ **Already supported** - Raw SQL execution works perfectly
- 🟡 **Can be enhanced** - Model Managers will provide type-safe wrappers
- 📋 **Recommended:** Implement Model Managers (already promised) for better DX

**Both features are fully deliverable via ORM patterns**, and Lifeguard already has the foundation (raw SQL, query builder, type-safe models) to support them.

---

## 12. Migration Generation Requirements (Entity-Driven Migrations)

### Missing Features for Entity-Driven Migration Generation

When building entities to generate SQL migrations, the following features are **critical blockers**:

#### 🔴 Critical Blockers (Must Have)

1. **Foreign Key Constraints**
   - **Status:** ❌ Missing
   - **Required For:** All entities with relationships (chart_of_accounts.parent_id, accounts.chart_of_account_id, etc.)
   - **SQL Example:** `FOREIGN KEY (parent_id) REFERENCES chart_of_accounts(id) ON DELETE SET NULL`
   - **Proposed Attribute:** `#[foreign_key = "table(column) ON DELETE action"]`
   - **Actions Needed:**
     - Add `foreign_key` attribute parsing to LifeModel macro
     - Store foreign key metadata in ColumnDefinition
     - Generate FOREIGN KEY constraints in migration SQL generation
     - Support ON DELETE actions: RESTRICT, CASCADE, SET NULL, NO ACTION

2. **CHECK Constraints**
   - **Status:** ❌ Missing
   - **Required For:** Business logic validation (journal_entries.total_debit = total_credit, journal_entry_lines debit OR credit, etc.)
   - **SQL Example:** `CONSTRAINT check_balanced_entry CHECK (total_debit = total_credit)`
   - **Proposed Attribute:** `#[check = "expression"]` (table-level) or `#[check = "expression"]` (column-level)
   - **Actions Needed:**
     - Add `check` attribute parsing to LifeModel macro
     - Store CHECK constraint metadata (table-level vs column-level)
     - Generate CHECK constraints in migration SQL generation
     - Support complex expressions (multiple columns, operators, functions)

3. **Composite Unique Constraints**
   - **Status:** ❌ Missing
   - **Required For:** Multi-column unique constraints (account_balances: account_id, fiscal_period_id, balance_date, currency_code, company_id)
   - **SQL Example:** `UNIQUE(account_id, fiscal_period_id, balance_date, currency_code, company_id)`
   - **Proposed Attribute:** `#[composite_unique = ["column1", "column2", ...]]` (table-level)
   - **Actions Needed:**
     - Add `composite_unique` attribute parsing to LifeModel macro
     - Store composite unique metadata
     - Generate UNIQUE constraints in migration SQL generation

4. **Index Definitions**
   - **Status:** ❌ Missing (partial - `#[indexed]` exists but only for single columns)
   - **Required For:** Composite indexes, partial indexes, unique indexes
   - **SQL Example:** `CREATE INDEX idx_journal_entries_source ON journal_entries(source_type, source_id)`
   - **SQL Example (Partial):** `CREATE INDEX idx_invoices_customer_id ON invoices(customer_id) WHERE customer_id IS NOT NULL`
   - **Proposed Attribute:** `#[index = "name(columns) WHERE condition"]` (table-level)
   - **Actions Needed:**
     - Add `index` attribute parsing to LifeModel macro
     - Store index metadata (name, columns, unique, partial WHERE clause)
     - Generate CREATE INDEX statements in migration SQL generation
     - Support composite indexes, partial indexes, unique indexes

#### 🟡 Nice-to-Have (Enhancements)

5. **Table Comments**
   - **Status:** ❌ Missing
   - **Required For:** Documentation (COMMENT ON TABLE)
   - **SQL Example:** `COMMENT ON TABLE chart_of_accounts IS 'Hierarchical chart of accounts structure'`
   - **Proposed Attribute:** `#[table_comment = "..."]` (table-level)
   - **Actions Needed:**
     - Add `table_comment` attribute parsing to LifeModel macro
     - Store table comment metadata
     - Generate COMMENT ON TABLE statements in migration SQL generation

### Current Entity Examples (Dog Fooding)

Entities created in `examples/entities/` demonstrate these gaps:

1. **chart_of_accounts.rs** - Missing: foreign key (self-reference), indexes, table comment
2. **account.rs** - Missing: foreign key (to chart_of_accounts), indexes, table comment
3. **journal_entry.rs** - Missing: CHECK constraint (total_debit = total_credit), composite index, table comment

### Implementation Priority

**Phase 1 (Critical for Migration Generation):**
1. Foreign key constraints
2. CHECK constraints
3. Composite unique constraints
4. Index definitions

**Phase 2 (Enhancements):**
5. Table comments

### Migration SQL Generation Requirements

The migration generator needs to:
1. **Read entity definitions** - Parse `#[derive(LifeModel)]` structs
2. **Extract metadata** - Collect all column attributes, foreign keys, constraints, indexes
3. **Compare with previous state** - Diff current entities vs previous snapshot
4. **Generate SQL** - Create CREATE TABLE, ALTER TABLE, CREATE INDEX, etc. statements
5. **Handle relationships** - Generate foreign keys in correct order (dependencies)

### Related Documentation

- `../migrations/README.md` - Migration strategy and entity-driven generation process
- `../migrations/original/` - Reference SQL migrations to match
- `../docs/MIGRATION_PROCESS_DIAGRAMS.md` - Migration process architecture
