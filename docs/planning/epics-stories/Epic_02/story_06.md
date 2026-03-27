# Story 06: Batch Operations

## Description

Implement batch operations for efficient bulk inserts, updates, and deletes. These replicate SeaORM's `insert_many()`, `update_many()`, and `delete_many()` methods.

## Acceptance Criteria

- [ ] `Entity::insert_many(models, pool)` - Batch insert multiple records
- [ ] `Entity::update_many(filter, values, pool)` - Batch update matching records
- [ ] `Entity::delete_many(filter, pool)` - Batch delete matching records
- [ ] Batch operations use efficient SQL (single query with multiple values)
- [ ] Batch operations return affected row counts
- [ ] Unit tests demonstrate all batch operations

## Technical Details

- `insert_many()`: `INSERT INTO table (columns) VALUES (values1), (values2), ... RETURNING *`
- `update_many()`: `UPDATE table SET column = value WHERE filter`
- `delete_many()`: `DELETE FROM table WHERE filter`
- Use SeaQuery for SQL building
- Support transactions for atomic batch operations
- Consider chunking for very large batches (1000+ records)

## Dependencies

- Story 03: Implement Basic CRUD Operations
- Story 04: Integrate SeaQuery for SQL Building

## Notes

- Batch operations are critical for performance
- Should match SeaORM's batch operation API
- Consider adding progress callbacks for large batches

