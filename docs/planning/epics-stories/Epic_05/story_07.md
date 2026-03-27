# Story 07: Materialized Views and Generated Columns

## Description

Support materialized views and generated columns in LifeModel. Materialized views should be queryable like regular tables.

## Acceptance Criteria

- [ ] Materialized views can be queried via LifeModel
- [ ] Generated columns supported in LifeModel
- [ ] Materialized view refresh: `MaterializedView::refresh(pool)`
- [ ] Unit tests demonstrate materialized views and generated columns

## Technical Details

- Materialized views: treat as read-only LifeModel
- Generated columns: handled automatically (PostgreSQL computes)
- Refresh: `REFRESH MATERIALIZED VIEW [CONCURRENTLY]`
- Support: `CREATE MATERIALIZED VIEW`, `REFRESH MATERIALIZED VIEW`

## Dependencies

- Epic 02: ORM Core (LifeModel)

## Notes

- Materialized views are useful for reporting
- Generated columns reduce application logic
- Consider adding auto-refresh strategies

