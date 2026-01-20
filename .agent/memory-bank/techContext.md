# Technical Context

## Technology Stack
- **Language**: Rust (edition 2021)
- **Runtime**: may coroutine runtime
- **Database**: PostgreSQL via may_postgres
- **Query Builder**: sea-query (v1.0.0-rc.29)
- **Macros**: Procedural macros (proc-macro2, syn, quote)

## Architecture
- **Derive Macros**: LifeModel, DeriveEntity
- **Core Traits**: ModelTrait, LifeModelTrait, LifeEntityName
- **Error Handling**: ModelError enum with detailed error messages

## Key Components
- `src/model.rs`: ModelTrait implementation
- `src/query.rs`: Query builder and LifeModelTrait
- `lifeguard-derive/`: Procedural macro implementations
- `lifeguard-derive/src/macros/life_model.rs`: Main macro generation

## Procedural Macro Architecture

### Nested Macro Expansion Pattern
- **LifeModel** (parent macro) generates: Entity, Model, Column, PrimaryKey structs/enums
- **DeriveEntity** (nested macro) generates: LifeModelTrait, LifeEntityName, Iden, IdenStatic implementations
- Uses `#[derive(DeriveEntity)]` attribute on Entity struct for nested expansion
- Column enum name passed via `#[column = "ColumnName"]` attribute or default pattern

### Code Generation Patterns
1. **Entity Generation**: Empty struct with `#[derive(DeriveEntity)]` attribute
2. **Model Generation**: Struct with fields matching input struct
3. **Column Enum**: Enum with variants matching field names (PascalCase)
4. **PrimaryKey Enum**: Enum with variants for primary key fields
5. **FromRow Implementation**: Generated for Model struct using may_postgres row access
6. **ModelTrait Implementation**: Generated with match arms for each column

### Type Handling in Macros
- **Primitive Types**: Direct mapping to sea_query::Value variants
  - `i32` → `Value::Int`
  - `i64` → `Value::BigInt`
  - `String` → `Value::String`
  - `bool` → `Value::Bool`
- **Option Types**: Extracted via `extract_option_inner_type()` helper
  - `Option<T>` → inner type `T` with `None` handling
- **JSON Types**: `serde_json::Value` → `Value::Json(Some(Box::new(v)))`
- **Unknown Types**: Fallback to `Value::String(None)` with warning comments

### Column Enum Requirements
- Must implement `sea_query::Iden` for dynamic identifiers
- Must implement `sea_query::IdenStatic` for static identifiers (required by LifeModelTrait)
- Generated with `Copy, Clone, Debug, PartialEq, Eq, Hash` derives
- Variants match field names in PascalCase

### Primary Key Tracking
- First field with `#[primary_key]` attribute is tracked
- Stores: type, auto_increment flag, column mapping
- Generates `get_primary_key_value()` implementation
- Returns `Value::String(None)` if no primary key found (with warning)

### Error Handling in Generated Code
- `ModelTrait::set()` returns `Result<(), ModelError>`
- Type mismatches return `ModelError::InvalidValueType` with details
- Null values for non-Option fields return error
- Unknown types fallback with comments (may hide bugs)

## Testing
- Test framework: cargo test
- Test location: `lifeguard-derive/tests/test_minimal.rs`
- Current: 25 tests passing
- **Test Structure**: Use separate modules (`mod option_tests`, `mod json_tests`) to avoid name conflicts
- **Manual Models**: For complex types (JSON), create manual Model/Column/Entity implementations for testing
