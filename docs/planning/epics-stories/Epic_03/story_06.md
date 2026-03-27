# Story 06: Advanced Migration Operations

## Description

Implement advanced migration operations that replicate SeaORM's `refresh()`, `reset()`, and enhanced status reporting. Also ensure atomic migrations with proper transaction handling.

## Acceptance Criteria

- [ ] `Migrator::refresh()` - Rollback all and reapply all migrations
- [ ] `Migrator::reset()` - Rollback all migrations
- [ ] `Migrator::status()` - Detailed migration status (applied, pending, failed)
- [ ] Atomic migrations (all changes in single transaction)
- [ ] Rollback on failure (if any migration fails, rollback all)
- [ ] Migration status includes: version, name, applied_at, execution time
- [ ] Unit tests demonstrate all advanced operations

## Technical Details

- `refresh()`:
  1. Rollback all migrations (execute `down()` in reverse order)
  2. Reapply all migrations (execute `up()` in order)
- `reset()`:
  1. Rollback all migrations
  2. Clear migration history (optional)
- `status()`:
  - Returns list of all migrations with status
  - Status: `Pending`, `Applied`, `Failed`
  - Include metadata: `applied_at`, `execution_time`, `error_message`
- Atomic migrations:
  - Wrap entire migration in transaction
  - Rollback on any error
  - PostgreSQL: native transaction support
  - Other databases: simulate with savepoints if possible

## Dependencies

- Story 02: Build Migration Runner
- Epic 01: Story 06 (Transaction Support)

## Notes

- `refresh()` is useful for development/testing
- `reset()` is dangerous - use with caution
- Atomic migrations ensure data integrity
- Status reporting helps with debugging

