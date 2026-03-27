# Story 06: Transaction Support

## Description

Implement transaction support for Lifeguard, replicating SeaORM's transaction API. Transactions should work with `may_postgres` connections and support commit/rollback operations.

## Acceptance Criteria

- [ ] `pool.begin()` - Start transaction, returns `Transaction`
- [ ] `transaction.commit()` - Commit transaction
- [ ] `transaction.rollback()` - Rollback transaction
- [ ] Transactions work with LifeExecutor trait
- [ ] Nested transactions supported (savepoints)
- [ ] Transaction isolation levels configurable
- [ ] Unit tests demonstrate transaction usage

## Technical Details

- Use `may_postgres::Transaction` for transaction management
- Transaction API:
  ```rust
  let transaction = pool.begin()?;
  // ... operations using transaction
  transaction.commit()?;
  // or
  transaction.rollback()?;
  ```
- Support isolation levels: `ReadUncommitted`, `ReadCommitted`, `RepeatableRead`, `Serializable`
- Nested transactions use PostgreSQL savepoints
- LifeExecutor should work with both connections and transactions

## Dependencies

- Story 03: Implement LifeExecutor Trait
- Story 04: Redesign LifeguardPool for may_postgres

## Notes

- Transactions are essential for data integrity
- Should match SeaORM's transaction API
- Consider adding transaction callbacks/hooks

