# Epic 03: Migrations

## Overview

Implement database migration system with `LifeMigration` trait, migration runner, and CLI tooling for managing schema changes.

## Goals

- Implement `LifeMigration` trait for defining migrations (replicates SeaORM's `MigrationTrait`)
- Build migration runner for executing migrations (replicates SeaORM's `Migrator`)
- Create CLI tooling (`lifeguard migrate`) - replicates `sea-orm-cli migrate`
- Support core PostgreSQL features in migrations
- Track migration history in database
- Support programmatic migrations (Rust code, not SQL files)
- Support data seeding in migrations
- Implement advanced operations: `refresh()`, `reset()`, enhanced `status()`
- Ensure atomic migrations with transaction support

## Success Criteria

- `LifeMigration` trait allows defining up/down migrations (replicates `MigrationTrait`)
- Migration runner provides: `up()`, `down()`, `refresh()`, `reset()`, `status()` (replicates `Migrator`)
- CLI tool provides all commands: `init`, `create`, `up`, `down`, `refresh`, `reset`, `status` (replicates `sea-orm-cli`)
- Migration history table tracks applied migrations with metadata
- Support for: CREATE TABLE, ALTER TABLE, CREATE INDEX, DROP statements
- Migrations are idempotent and reversible
- Programmatic migrations (Rust code) supported
- Data seeding in migrations supported
- Atomic migrations (transactions) ensure all-or-nothing execution
- Conditional operations (`has_column`, `has_table`, etc.) prevent errors

## Timeline

**Weeks 6-8**

## Dependencies

- Epic 01: Foundation (must be complete)
- Epic 02: ORM Core (helpful but not required)

## Technical Notes

- Migrations should be versioned and ordered
- Migration history stored in `_lifeguard_migrations` table
- CLI should support dry-run mode
- Migrations should support transactions (rollback on failure)
- Support for custom SQL in migrations

## Related Epics

- Epic 01: Foundation (prerequisite)
- Epic 04: v1 Release (depends on this epic)

