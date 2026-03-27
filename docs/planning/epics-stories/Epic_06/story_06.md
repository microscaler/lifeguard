# Story 06: Schema-First Design

## Description

Implement schema-first design where a schema file (YAML/TOML) defines models, and code is generated from the schema. This matches Prisma's approach and provides a single source of truth.

## Acceptance Criteria

- [ ] Schema file format (YAML or TOML)
- [ ] Schema defines: tables, columns, types, relationships, indexes
- [ ] Code generation from schema → `#[derive(LifeModel)]` code
- [ ] Schema validation (check for errors)
- [ ] Schema diff detection (changes between versions)
- [ ] Unit tests demonstrate schema-first workflow

## Technical Details

- Schema format (YAML example):
  ```yaml
  models:
    User:
      table: users
      fields:
        id:
          type: i64
          primary_key: true
        email:
          type: String
          unique: true
        created_at:
          type: DateTime<Utc>
          default: now
      relations:
        posts:
          type: has_many
          model: Post
  ```
- Code generation:
  - Parse schema file
  - Generate `#[derive(LifeModel)]` structs
  - Generate relations
  - Generate migrations (if schema changed)
- Schema validation:
  - Check for duplicate names
  - Validate types
  - Validate relationships
  - Check for circular dependencies

## Dependencies

- Story 04: Schema Introspection Tools
- Story 05: Code Generation from Database

## Notes

- Schema-first is popular in modern ORMs (Prisma, TypeORM)
- Provides single source of truth
- Enables better tooling and validation

