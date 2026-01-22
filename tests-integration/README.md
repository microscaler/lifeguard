# Lifeguard Integration Tests

This crate contains integration tests that require a live database connection. These tests are separated from the main test suite to avoid slowing down the normal test runs.

## Running Integration Tests

Integration tests require a running PostgreSQL database. They can be run using:

```bash
# Get connection string from Kind cluster
TEST_DATABASE_URL=$(./scripts/get_test_connection_string.sh) cargo test --package lifeguard-integration-tests

# Or with nextest
TEST_DATABASE_URL=$(./scripts/get_test_connection_string.sh) cargo nextest run --package lifeguard-integration-tests
```

## Test Coverage

- **Migration Tests**: Full lifecycle testing of the migration system including:
  - Migration file discovery
  - Migration registration
  - Migration execution
  - State tracking
  - Schema verification
  - Checksum validation

## Note

These tests are intentionally excluded from the main test suite (`cargo test --workspace`) to keep the normal test runs fast. They should be run separately when needed or as part of CI/CD pipelines.
