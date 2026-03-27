# Story 02: Integrate may_postgres as Database Client

## Description

Integrate `may_postgres` as the native database client for Lifeguard. This replaces SeaORM's database connection layer with a coroutine-native client.

## Acceptance Criteria

- [ ] `may_postgres` added to `Cargo.toml` dependencies
- [ ] Basic connection to PostgreSQL works using `may_postgres`
- [ ] Connection string parsing and validation works
- [ ] Connection errors are handled gracefully
- [ ] Unit tests demonstrate successful connection

## Technical Details

- Add `may_postgres = "x.x.x"` to `Cargo.toml`
- Create connection wrapper around `may_postgres::Connection`
- Implement connection string parsing (PostgreSQL URI format)
- Handle connection errors (network failures, authentication, etc.)
- Create example showing basic connection

## Dependencies

- Story 01: Remove SeaORM and Tokio Dependencies

## Notes

- `may_postgres` is a coroutine-native port of `rust-postgres`
- Connection strings should follow PostgreSQL URI format: `postgresql://user:pass@host:port/dbname`
- Test with both local and remote PostgreSQL instances

