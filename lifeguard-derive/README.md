# lifeguard-derive

Procedural macros for Lifeguard ORM - a coroutine-native PostgreSQL ORM for Rust.

## Overview

`lifeguard-derive` provides derive macros that generate boilerplate code for database entities, models, records, and relationships. These macros follow SeaORM's architecture patterns but are designed for Lifeguard's coroutine-based runtime.

## Features

### Core Derive Macros

- **`LifeModel`** - Generates immutable database row representations
- **`LifeRecord`** - Generates mutable change-set objects for updates
- **`DeriveEntity`** - Generates entity unit structs and trait implementations
- **`FromRow`** - Generates `FromRow` trait implementations for converting database rows
- **`DeriveRelation`** - Generates `Related` trait implementations for entity relationships
- **`DeriveLinked`** - Generates `Linked` trait implementations for multi-hop relationships

## Usage

### Basic Entity Definition

```rust
use lifeguard_derive::LifeModel;

#[derive(LifeModel)]
#[lifeguard(table_name = "users")]
pub struct User {
    #[lifeguard(primary_key, auto_increment)]
    pub id: i32,
    
    pub name: String,
    
    #[lifeguard(column_name = "email_address", unique, indexed)]
    pub email: String,
    
    #[lifeguard(nullable)]
    pub bio: Option<String>,
}
```

This generates:
- `Entity` unit struct
- `Model` struct (immutable row representation)
- `Column` enum (all columns)
- `PrimaryKey` enum (primary key columns)
- `FromRow` implementation
- `LifeModelTrait` implementation

### Mutable Records for Updates

```rust
use lifeguard_derive::LifeRecord;

#[derive(LifeRecord)]
#[lifeguard(table_name = "users")]
pub struct UserRecord {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub bio: Option<String>,
}
```

This generates:
- `Record` struct with `Option<T>` fields
- `from_model()` method (create from `LifeModel` for updates)
- `to_model()` method (convert to `LifeModel`, `None` fields use defaults)
- `dirty_fields()` method (returns list of changed fields)
- `is_dirty()` method (checks if any fields changed)
- Setter methods for each field

### Entity Relationships

```rust
use lifeguard_derive::DeriveRelation;

#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(has_many = "super::posts::Entity")]
    Posts,
    
    #[lifeguard(belongs_to = "super::users::Entity")]
    User,
}
```

This generates `Related` trait implementations for each relationship variant.

### Multi-Hop Relationships

```rust
use lifeguard_derive::DeriveLinked;

#[derive(DeriveLinked)]
pub enum LinkedRelation {
    #[lifeguard(linked = "PostEntity -> CommentEntity")]
    Comments,
}
```

This generates `Linked` trait implementations for multi-hop relationship queries.

## Attributes

### Table-Level Attributes

- `table_name` - Database table name
- `schema_name` - Database schema name (default: `public`)
- `table_comment` - Table comment/documentation

### Column Attributes

- `column_name` - Database column name (default: snake_case of field name)
- `column_type` - Explicit column type override
- `primary_key` - Marks column as primary key
- `auto_increment` - Marks column as auto-incrementing
- `nullable` - Allows NULL values
- `unique` - Creates unique constraint
- `indexed` - Creates index
- `default_value` - Default value for column
- `default_expr` - Default expression (SQL)
- `renamed_from` - Previous column name (for migrations)
- `select_as` - Custom SELECT expression
- `save_as` - Custom save expression
- `comment` - Column comment/documentation
- `skip` - Skip field in model/record
- `skip_from_row` - Skip field in `FromRow` implementation

### Relationship Attributes

- `has_many` - One-to-many relationship
- `belongs_to` - Many-to-one relationship
- `has_one` - One-to-one relationship
- `linked` - Multi-hop relationship path (for `DeriveLinked`)

## Type Support

The derive macros support a wide range of Rust types:

- **Integers**: `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`
- **Floats**: `f32`, `f64`
- **Strings**: `String`, `&str` (with proper handling)
- **Booleans**: `bool`
- **Dates/Times**: `chrono::NaiveDateTime`, `chrono::DateTime`, `chrono::NaiveDate`
- **UUIDs**: `uuid::Uuid`
- **Decimals**: `rust_decimal::Decimal`
- **JSON**: `serde_json::Value`
- **Optionals**: `Option<T>` for nullable columns
- **Vectors**: `Vec<T>` for array columns

## Testing

The crate includes comprehensive test coverage:

- Unit tests for each macro
- Integration tests with real database connections
- Compile-fail tests using `trybuild`
- Edge case coverage for various attribute combinations

Run tests with:

```bash
cargo test
```

## Documentation

For detailed usage examples and attribute reference, see:

- `tests/test_minimal.rs` - Basic usage examples
- `DERIVE_RELATION_USAGE.md` - Relationship macro guide
- `DERIVE_LINKED_USAGE.md` - Multi-hop relationship guide
- `SEAORM_LIFEGUARD_MAPPING.md` - Migration guide from SeaORM

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
