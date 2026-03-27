# Story 05: Type-Safe Query Builders

## Description

Build type-safe query builders that provide compile-time validation and IDE autocomplete. These should feel natural and prevent common SQL errors.

## Acceptance Criteria

- [ ] Query builders are type-safe (compile-time validation)
- [ ] IDE autocomplete works for query methods
- [ ] Filter methods match LifeModel field types
- [ ] Query building is chainable and fluent
- [ ] Error messages are clear when queries are invalid
- [ ] Unit tests demonstrate type-safe query building

## Technical Details

- Query builder pattern: `LifeModel::find().filter(Field::eq(value)).all(pool)` (matches SeaORM API)
- Type-safe filters: `User::Email.eq("test@example.com")` (matches SeaORM's Column enum usage)
- Support all SeaORM filter operations:
  - Comparison: `eq()`, `ne()`, `gt()`, `gte()`, `lt()`, `lte()`
  - Pattern matching: `like()`, `ilike()` (case-insensitive)
  - Membership: `in()`, `is_not_in()`
  - Null checks: `is_null()`, `is_not_null()`
  - Range: `between(start, end)`
  - Containment: `contains(value)` (for arrays/JSONB)
  - Logical: `and()`, `or()` (chain conditions)
- Support ordering: `order_by()`, `order_by_desc()`, `order_by_asc()` (explicit)
- Support pagination: `limit()`, `offset()`
- Support aggregation: `count()`, `sum()`, `avg()`, `max()`, `min()`
- Support pagination helpers: `paginate(pool, page_size)`, `paginate_and_count(pool, page_size)`
- Support grouping: `group_by()`, `having()` (advanced queries)
- Query builder methods: `all()`, `one()`, `find_one()`, `find_by_id()`

## Dependencies

- Story 04: Integrate SeaQuery for SQL Building

## Notes

- Type safety is a key differentiator from raw SQL
- Look at SeaORM's query builder API for inspiration
- Consider adding query validation (warn about missing indexes, etc.)

