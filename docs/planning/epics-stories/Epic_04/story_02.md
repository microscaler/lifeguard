# Story 02: Build Testkit Infrastructure

## Description

Create a testkit that makes it easy to test database operations. This should include test database setup, transaction rollback, and fixture loading.

## Acceptance Criteria

- [ ] Testkit provides test database setup/teardown
- [ ] Transactions rollback after each test (clean state)
- [ ] Fixture loading helpers (insert test data)
- [ ] Test database isolation (each test gets clean DB)
- [ ] Unit tests demonstrate testkit usage

## Technical Details

- Testkit should:
  - Create test database or use transactions
  - Run migrations before tests
  - Rollback transactions after each test
  - Provide helpers: `insert_fixture()`, `clear_table()`, etc.
- Consider: `lifeguard-testkit` crate
- Support both: in-memory test DB and real PostgreSQL

## Dependencies

- Epic 01: Foundation
- Epic 03: Migrations

## Notes

- Testkit is essential for testing ORM functionality
- Consider adding test data builders
- Support parallel test execution

