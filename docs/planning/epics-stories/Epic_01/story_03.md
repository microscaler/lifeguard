# Story 03: Implement LifeExecutor Trait

## Description

Create the `LifeExecutor` trait that abstracts database execution over `may_postgres` connections. This trait will be the foundation for all database operations.

## Acceptance Criteria

- [ ] `LifeExecutor` trait defined with execute methods
- [ ] Trait supports: `execute`, `query_one`, `query_all`
- [ ] Implementation works with `may_postgres::Connection`
- [ ] Error handling returns appropriate Lifeguard error types
- [ ] Unit tests cover all trait methods

## Technical Details

```rust
pub trait LifeExecutor {
    fn execute(&mut self, query: &str, params: &[&dyn ToSql]) -> Result<u64>;
    fn query_one<T>(&mut self, query: &str, params: &[&dyn ToSql]) -> Result<T>;
    fn query_all<T>(&mut self, query: &str, params: &[&dyn ToSql]) -> Result<Vec<T>>;
}
```

- Use `may_postgres::Connection` methods
- Map `may_postgres` errors to Lifeguard error types
- Support parameterized queries (SQL injection prevention)
- Return row count for execute operations

## Dependencies

- Story 02: Integrate may_postgres as Database Client

## Notes

- This trait abstracts the database client, allowing for future extensions
- Error types should be descriptive and actionable
- Consider adding transaction support in future stories

