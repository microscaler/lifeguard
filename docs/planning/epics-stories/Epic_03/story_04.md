# Story 04: Support Core PostgreSQL Features in Migrations

## Description

Ensure migrations support core PostgreSQL features: CREATE TABLE, ALTER TABLE, CREATE INDEX, DROP statements, and common constraints.

## Acceptance Criteria

- [ ] CREATE TABLE with columns, types, constraints
- [ ] ALTER TABLE (add/drop columns, modify types)
- [ ] CREATE INDEX (single, composite, unique, partial)
- [ ] DROP TABLE, DROP INDEX
- [ ] Foreign key constraints
- [ ] Check constraints
- [ ] Default values
- [ ] Unit tests demonstrate all supported features

## Technical Details

- Support PostgreSQL types: `text`, `varchar(n)`, `integer`, `bigint`, `boolean`, `timestamp`, `date`, `jsonb`, `uuid`, etc.
- Constraints: `PRIMARY KEY`, `FOREIGN KEY`, `UNIQUE`, `NOT NULL`, `CHECK`
- Indexes: `CREATE INDEX`, `CREATE UNIQUE INDEX`, `CREATE INDEX ... WHERE`
- Use SeaQuery for SQL building (if applicable) or raw SQL

## Dependencies

- Story 01: Implement LifeMigration Trait

## Notes

- Start with common features, expand later
- Consider adding helper methods for common patterns
- Document PostgreSQL-specific features

