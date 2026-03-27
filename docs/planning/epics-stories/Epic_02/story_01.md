# Story 01: Build LifeModel Derive Macro

## Description

Create a procedural macro `#[derive(LifeModel)]` that generates all necessary boilerplate for immutable database row representation. This macro should handle field mapping, type conversion, and basic query methods.

## Acceptance Criteria

- [ ] `#[derive(LifeModel)]` macro compiles and generates code
- [ ] Macro handles basic PostgreSQL types (text, integer, boolean, timestamp, etc.)
- [ ] Generated struct implements necessary traits for database operations
- [ ] Field mapping works (Rust field names to PostgreSQL column names)
- [ ] Type conversion handles PostgreSQL types to Rust types
- [ ] Unit tests demonstrate macro usage with various field types

## Technical Details

- Use `proc_macro` and `syn`/`quote` crates
- Macro should generate (replicating SeaORM's `DeriveEntityModel`):
  - `Model` struct (immutable row representation)
  - `Column` enum (all columns: `Column::Name`, `Column::Email`, etc.)
  - `PrimaryKey` enum (primary key columns)
  - `Entity` type (entity itself)
  - `FromRow` implementation for deserializing database rows
  - Field getters (immutable access)
  - Table name and column metadata
  - Primary key identification
- Support all SeaORM column attributes:
  - `#[table_name = "table_name"]` - Table name
  - `#[primary_key]` - Primary key field
  - `#[column_name = "custom_name"]` - Custom column name
  - `#[column_type = "Text"]` - Column type specification
  - `#[default_value = "value"]` - Default value
  - `#[unique]` - Unique constraint
  - `#[indexed]` - Indexed column
  - `#[nullable]` - Nullable field
  - `#[auto_increment]` - Auto-increment
  - `#[enum_name = "EnumName"]` - Enum type
- Handle snake_case to camelCase conversion
- Support composite primary keys

## Dependencies

- Epic 01: Foundation (must be complete)

## Notes

- This is a complex macro - start simple, iterate
- Look at SeaORM's `DeriveEntity` macro for inspiration
- Test with various PostgreSQL types early

