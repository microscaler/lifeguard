# Story 07: Raw SQL Helpers

## Description

Implement raw SQL execution helpers that replicate SeaORM's `find_by_statement()` and `execute_unprepared()` methods. These allow executing custom SQL when query builders aren't sufficient.

## Acceptance Criteria

- [ ] `Entity::find_by_statement(statement)` - Execute raw SQL and return entities
- [ ] `Entity::execute_unprepared(sql)` - Execute unprepared SQL
- [ ] Raw SQL works with LifeExecutor trait
- [ ] Parameter binding supported in raw SQL
- [ ] Results can be mapped to LifeModel
- [ ] Unit tests demonstrate raw SQL usage

## Technical Details

- `find_by_statement()`: Execute prepared statement, map results to LifeModel
- `execute_unprepared()`: Execute raw SQL string (use with caution)
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

