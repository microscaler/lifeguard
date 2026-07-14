# Story 07: Raw SQL Helpers

## Description

Historical story. Parameterized statement/query helpers remain, but the
`execute_unprepared()` convenience was removed in 2026-07 because it encouraged
application schema changes and bypass of the entity/migration process.

## Acceptance Criteria

- [ ] `Entity::find_by_statement(statement)` - Execute raw SQL and return entities
- [x] Do not expose `Entity::execute_unprepared(sql)`
- [ ] Raw SQL works with LifeExecutor trait
- [ ] Parameter binding supported in raw SQL
- [ ] Results can be mapped to LifeModel
- [ ] Unit tests demonstrate raw SQL usage

## Technical Details

- `find_by_statement()`: Execute prepared statement, map results to LifeModel
- Unprepared application execution is deliberately unsupported.
- Support parameter binding: `$1`, `$2`, etc.
- Map query results to LifeModel using `FromRow` implementation
- Use LifeExecutor for execution

## Dependencies

- Story 03: Implement LifeExecutor Trait
- Epic 02: ORM Core (LifeModel must exist)

## Notes

- Raw SQL is useful for complex queries
- Should match SeaORM's raw SQL API
- Document security considerations (SQL injection prevention)
