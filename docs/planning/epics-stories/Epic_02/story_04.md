# Story 04: Integrate SeaQuery for SQL Building

## Description

Integrate SeaQuery as the SQL query builder for Lifeguard. SeaQuery provides type-safe SQL building that's compatible with coroutines (no async runtime).

## Acceptance Criteria

- [ ] SeaQuery added to `Cargo.toml` dependencies
- [ ] LifeModel queries use SeaQuery builders
- [ ] Type-safe query building works (compile-time validation)
- [ ] Generated SQL is correct and parameterized
- [ ] Query builders support: SELECT, INSERT, UPDATE, DELETE
- [ ] Unit tests demonstrate various query patterns

## Technical Details

- Add `sea-query = "x.x.x"` to `Cargo.toml`
- Create `LifeQuery` facade over SeaQuery (if needed)
- Map SeaQuery `QueryBuilder` to SQL strings
- Support SeaQuery features:
  - `select()`, `from()`, `where()`, `order_by()`, `limit()`, `offset()`
  - `insert()`, `values()`, `on_conflict()`
  - `update()`, `set()`
  - `delete()`
- Parameter binding for SQL injection prevention

## Dependencies

- Story 03: Implement Basic CRUD Operations

## Notes

- SeaQuery is sync (no async), perfect for coroutines
- This is the "borrow" part of "beg, borrow, steal"
- Consider creating a `LifeQuery` wrapper for Lifeguard-specific features

