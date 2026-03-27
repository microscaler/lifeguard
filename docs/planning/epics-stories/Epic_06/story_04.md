# Story 04: Schema Introspection Tools (Diesel table! Macro Equivalent)

## Description

Build tools to introspect PostgreSQL schema and generate Rust types. This replicates Diesel's `table!` macro capability, allowing code generation from existing databases.

## Acceptance Criteria

- [ ] Schema introspection reads PostgreSQL catalog
- [ ] Generates LifeModel definitions from existing tables (replicates `table!` macro)
- [ ] Handles: columns, types, constraints, indexes, foreign keys
- [ ] Generates Column enums (matches Diesel's column types)
- [ ] Generates PrimaryKey enums
- [ ] CLI command: `lifeguard introspect` (replicates `diesel print-schema`)
- [ ] Can generate schema file (YAML/TOML) for schema-first workflow
- [ ] Incremental updates (don't overwrite custom code)
- [ ] Unit tests demonstrate schema introspection

## Technical Details

- Query PostgreSQL system catalogs:
  - `pg_class` (tables)
  - `pg_attribute` (columns)
  - `pg_type` (types)
  - `pg_constraint` (constraints, foreign keys)
  - `pg_index` (indexes)
- Generate Rust code (Diesel `table!` equivalent):
  ```rust
  // Generated from database
  #[derive(LifeModel)]
  #[table = "users"]
  pub struct User {
      #[primary_key]
      pub id: i64,
      pub email: String,
      pub created_at: DateTime<Utc>,
  }
  
  // Also generates Column enum (like Diesel)
  pub enum UserColumn {
      Id,
      Email,
      CreatedAt,
  }
  ```
- CLI commands:
  - `lifeguard introspect` - Generate LifeModel code
  - `lifeguard introspect --schema-file` - Generate schema file
  - `lifeguard introspect --output-dir models/` - Output directory
- Match Diesel's `diesel print-schema` functionality
- Support custom type mappings (PostgreSQL → Rust types)

## Dependencies

- Epic 02: ORM Core (LifeModel must exist)
- Epic 03: Migrations (CLI tooling)

## Notes

- Essential for working with existing databases
- Replicates Diesel's `table!` macro capability
- Consider incremental updates (don't overwrite custom code)
- Support custom type mappings
- Can feed into schema-first workflow (Story 06)

