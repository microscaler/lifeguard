# Story 10: Validators

## Description

Implement validation system for LifeModel and LifeRecord. This allows field-level and model-level validation before database operations.

## Acceptance Criteria

- [ ] Validator trait definition
- [ ] Field-level validators (e.g., `#[validate(email)]`, `#[validate(length(min = 1, max = 100))]`)
- [ ] Model-level validators
- [ ] Validation error collection
- [ ] Integration with hooks (validation in `before_insert`, `before_update`)
- [ ] Unit tests demonstrate all validation types

## Technical Details

- Validator trait:
  ```rust
  pub trait Validator<T> {
      fn validate(&self, value: &T) -> Result<(), ValidationError>;
  }
  ```
- Built-in validators:
  - `email` - Email format validation
  - `length(min, max)` - String length validation
  - `range(min, max)` - Numeric range validation
  - `regex(pattern)` - Pattern matching
  - `custom(function)` - Custom validation function
- Validation errors:
  ```rust
  pub struct ValidationError {
      pub field: String,
      pub message: String,
      pub code: String,
  }
  ```
- Validation runs in `before_insert` and `before_update` hooks

## Dependencies

- Story 09: Entity Hooks & Lifecycle Events

## Notes

- Validation prevents invalid data from reaching database
- Should be extensible for custom validators
- Consider async validators for database lookups

