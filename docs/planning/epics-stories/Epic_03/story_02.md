# Story 02: Build Migration Runner

## Description

Create a migration runner that executes migrations in the correct order, tracks migration history, and handles rollbacks.

## Acceptance Criteria

- [ ] Migration runner executes migrations in version order
- [ ] Migration history table (`_lifeguard_migrations`) tracks applied migrations
- [ ] Runner skips already-applied migrations (idempotent)
- [ ] Rollback support (execute `down()` methods in reverse order)
- [ ] Error handling: rollback on failure, clear error messages
- [ ] Unit tests cover: apply, rollback, idempotency

## Technical Details

- Create `_lifeguard_migrations` table: `version`, `name`, `applied_at`
- Migration runner:
  1. Query migration history
  2. Find unapplied migrations
  3. Execute `up()` methods in version order
  4. Record in history table
- Rollback:
  1. Query migration history (reverse order)
  2. Execute `down()` methods
  3. Remove from history table
- Use transactions for atomicity

## Dependencies

- Story 01: Implement LifeMigration Trait

## Notes

- Migrations should run in transactions (all or nothing)
- Consider adding dry-run mode
- Migration history should be immutable (append-only)

