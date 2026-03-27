# Story 07: Upsert Support (Save and On Conflict)

## Description

Implement upsert operations that insert or update records based on conflict resolution. This replicates SeaORM's `save()` method and `on_conflict()` handling.

## Acceptance Criteria

- [ ] `LifeRecord::save(pool)` - Insert or update (upsert)
- [ ] `on_conflict()` support for conflict resolution
- [ ] Support PostgreSQL `ON CONFLICT` clause
- [ ] Conflict resolution strategies: `do_nothing`, `do_update`, `do_update_set`
- [ ] Unit tests demonstrate upsert operations

## Technical Details

- `save()`: Check if record exists, insert if not, update if exists
- `on_conflict()`: `INSERT ... ON CONFLICT (key) DO UPDATE SET ...`
- Conflict resolution:
  - `do_nothing`: Ignore conflicts
  - `do_update`: Update on conflict
  - `do_update_set`: Update specific columns on conflict
- Use SeaQuery's `on_conflict()` builder
- Support composite unique constraints

## Dependencies

- Story 02: Build LifeRecord Derive Macro
- Story 03: Implement Basic CRUD Operations
- Story 04: Integrate SeaQuery for SQL Building

## Notes

- Upsert is essential for idempotent operations
- Should match SeaORM's `save()` and `on_conflict()` API
- Consider adding `upsert_many()` for batch upserts

