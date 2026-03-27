# Story 09: Entity Hooks & Lifecycle Events

## Description

Implement entity hooks system that replicates SeaORM's lifecycle events. This allows intercepting operations for validation, logging, side effects, and operation abortion.

## Acceptance Criteria

- [ ] `LifeRecord` hooks: `before_insert()`, `after_insert()`, `before_update()`, `after_update()`, `before_delete()`, `after_delete()`
- [ ] `LifeModel` hooks: `after_load()`
- [ ] Hook registration via trait implementation
- [ ] Hook execution order (before hooks can abort operations)
- [ ] Hook context (access to model, pool, transaction)
- [ ] Unit tests demonstrate all hook types

## Technical Details

- Hook trait:
  ```rust
  pub trait LifeModelHooks {
      fn after_load(&mut self, pool: &LifeguardPool) -> Result<()> { Ok(()) }
  }
  
  pub trait LifeRecordHooks {
      fn before_insert(&mut self, pool: &LifeguardPool) -> Result<()> { Ok(()) }
      fn after_insert(&mut self, pool: &LifeguardPool, model: LifeModel) -> Result<()> { Ok(()) }
      fn before_update(&mut self, pool: &LifeguardPool) -> Result<()> { Ok(()) }
      fn after_update(&mut self, pool: &LifeguardPool, model: LifeModel) -> Result<()> { Ok(()) }
      fn before_delete(&mut self, pool: &LifeguardPool) -> Result<()> { Ok(()) }
      fn after_delete(&mut self, pool: &LifeguardPool) -> Result<()> { Ok(()) }
  }
  ```
- Hooks can return `Err()` to abort operation
- Hooks execute in order: before hooks → operation → after hooks
- Hook context provides access to pool, transaction, model state

## Dependencies

- Story 02: Build LifeRecord Derive Macro
- Story 03: Implement Basic CRUD Operations
- Epic 01: Story 06 (Transaction Support)

## Notes

- Hooks are essential for validation, logging, and side effects
- Should match SeaORM's hook API
- Consider adding hook chains for multiple hooks

