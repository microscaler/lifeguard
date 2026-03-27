# Story 01: Implement LifeMigration Trait

## Description

Create the `LifeMigration` trait that defines up and down migrations. This trait will be used to define all database schema changes.

## Acceptance Criteria

- [ ] `LifeMigration` trait defined with `up()` and `down()` methods
- [ ] Trait provides access to `LifeExecutor` for running SQL
- [ ] Migrations can be versioned and ordered
- [ ] Unit tests demonstrate migration definition and execution

## Technical Details

```rust
pub trait LifeMigration {
    fn name(&self) -> &str;
    fn version(&self) -> u64;
    fn up(&self, executor: &mut dyn LifeExecutor) -> Result<()>;
    fn down(&self, executor: &mut dyn LifeExecutor) -> Result<()>;
}
```

- Version should be unique and incrementing
- `up()` applies the migration
- `down()` reverses the migration
- Migrations should be idempotent (safe to run multiple times)

## Dependencies

- Epic 01: Foundation (LifeExecutor trait)

## Notes

- Look at SeaORM's migration system for inspiration
- Consider adding migration metadata (author, description, timestamp)

