# Epic 02: ORM Core

## Overview

Build the core ORM functionality with `LifeModel` and `LifeRecord` derive macros, implementing basic CRUD operations and type-safe query builders.

## Goals

- Build `LifeModel` derive macro for immutable database row representation (replicates SeaORM's Model, Column, PrimaryKey, Entity)
- Build `LifeRecord` derive macro for mutable change-set objects (replicates SeaORM's ActiveModel)
- Implement basic CRUD operations (Create, Read, Update, Delete)
- Integrate SeaQuery for SQL building
- Create type-safe query builders (replicates SeaORM's QueryTrait, Select, Filter operations)
- Support batch operations (insert_many, update_many, delete_many)
- Support upsert operations (save method, on_conflict)
- Support pagination helpers (paginate, paginate_and_count)
- Support all SeaORM column attributes and query features

## Success Criteria

- `LifeModel` macro generates all necessary boilerplate (Model, Column enum, PrimaryKey enum, Entity type)
- `LifeRecord` macro generates change-set tracking (replicates ActiveModel)
- CRUD operations work with `may_postgres` connections
- All SeaORM query methods replicated: `find()`, `find_by_id()`, `find_one()`, `insert()`, `update()`, `delete()`, `save()`
- Batch operations supported: `insert_many()`, `update_many()`, `delete_many()`
- SeaQuery integration provides SQL building capabilities
- Type-safe queries compile and execute correctly
- All SeaORM column attributes supported: `primary_key`, `column_name`, `column_type`, `default_value`, `unique`, `indexed`, `nullable`, `auto_increment`
- All SeaORM filter operations supported: `eq()`, `ne()`, `gt()`, `gte()`, `lt()`, `lte()`, `like()`, `ilike()`, `in()`, `is_null()`, `is_not_null()`, `between()`, `contains()`
- Pagination helpers: `paginate()`, `paginate_and_count()`
- Examples demonstrate complete ORM usage matching SeaORM API

## Timeline

**Weeks 3-6**

## Dependencies

- Epic 01: Foundation (must be complete)
- SeaQuery crate (external dependency for SQL building)

## Technical Notes

- `LifeModel` should be immutable (safe to pass around) - replicates SeaORM's Model
- `LifeRecord` should track changes for inserts/updates - replicates SeaORM's ActiveModel
- Procedural macros should generate minimal boilerplate
- Query builders should be type-safe and compile-time validated
- Support for all PostgreSQL types (text, integer, boolean, timestamp, jsonb, arrays, custom types, etc.)
- **Complete SeaORM API parity**: All methods, attributes, and features should match SeaORM's API
- See `SEAORM_AUDIT.md` for complete feature list and coverage analysis

## Related Epics

- Epic 01: Foundation (prerequisite)
- Epic 03: Migrations (can start in parallel)

