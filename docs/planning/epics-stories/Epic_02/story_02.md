# Story 02: Build LifeRecord Derive Macro

## Description

Create a procedural macro `#[derive(LifeRecord)]` that generates mutable change-set objects for inserts and updates. LifeRecord tracks changes and can be converted to/from LifeModel.

## Acceptance Criteria

- [ ] `#[derive(LifeRecord)]` macro compiles and generates code
- [ ] LifeRecord tracks which fields have changed
- [ ] LifeRecord can be created from LifeModel (for updates)
- [ ] LifeRecord can be converted to LifeModel (after insert)
- [ ] Change tracking works correctly (dirty fields)
- [ ] Unit tests demonstrate insert and update workflows

## Technical Details

- Macro should generate:
  - Mutable struct with `Option<T>` fields (None = unchanged)
  - `from_model(model: LifeModel)` method
  - `to_model()` method (for inserts, None fields use defaults)
  - `dirty_fields()` method (returns list of changed fields)
  - `is_dirty()` method (checks if any fields changed)
- Support for default values (from database defaults or Rust defaults)
- Handle nullable fields correctly

## Dependencies

- Story 01: Build LifeModel Derive Macro

## Notes

- LifeRecord is for mutations, LifeModel is for reads
- Clear separation: LifeModel = immutable, LifeRecord = mutable
- Change tracking enables efficient updates (only update changed fields)

