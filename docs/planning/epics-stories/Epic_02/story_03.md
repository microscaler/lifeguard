# Story 03: Implement Basic CRUD Operations

## Description

Implement Create, Read, Update, Delete operations for LifeModel and LifeRecord. These should work with the LifeExecutor trait and use SeaQuery for SQL building.

## Acceptance Criteria

- [ ] `LifeModel::find_by_id(pool, id)` - Read by primary key
- [ ] `LifeModel::find()` - Query builder for reads
- [ ] `LifeRecord::insert(pool)` - Create new record
- [ ] `LifeRecord::update(pool)` - Update existing record
- [ ] `LifeModel::delete(pool)` - Delete record
- [ ] All operations use LifeExecutor trait
- [ ] All operations use SeaQuery for SQL building
- [ ] Unit tests cover all CRUD operations

## Technical Details

- `find_by_id`: `SELECT * FROM table WHERE id = $1`
- `find()`: Returns query builder (chainable filters)
- `insert()`: `INSERT INTO table (columns) VALUES (values) RETURNING *`
- `update()`: `UPDATE table SET column = value WHERE id = $1 RETURNING *`
- `delete()`: `DELETE FROM table WHERE id = $1`
- Use SeaQuery's `QueryBuilder` for SQL construction
- Map SeaQuery SQL to LifeExecutor calls

## Dependencies

- Story 01: Build LifeModel Derive Macro
- Story 02: Build LifeRecord Derive Macro
- Epic 01: Foundation (LifeExecutor trait)

## Notes

- Start with simple cases, add complexity later
- Error handling should be clear and actionable
- Consider adding batch operations in future stories

