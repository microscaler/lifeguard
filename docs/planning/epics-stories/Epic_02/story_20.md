# Story 20: Scopes

## Description

Implement named query scopes that allow defining reusable query fragments on models. This matches ActiveRecord's scopes and improves code organization.

## Acceptance Criteria

- [ ] Named scopes on models
- [ ] Scope chaining (combine multiple scopes)
- [ ] Default scopes (applied automatically)
- [ ] Parameterized scopes
- [ ] `unscoped()` to remove default scopes
- [ ] Unit tests demonstrate all scope features

## Technical Details

- Scope definition:
  ```rust
  impl User {
      fn active() -> QueryBuilder {
          User::find().filter(User::IsActive.eq(true))
      }
      
      fn with_email_domain(domain: &str) -> QueryBuilder {
          User::find().filter(User::Email.like(&format!("%@{}", domain)))
      }
      
      fn default_scope() -> QueryBuilder {
          User::find().filter(User::DeletedAt.is_null())
      }
  }
  ```
- Scope usage:
  ```rust
  // Chain scopes
  let users = User::active()
      .with_email_domain("example.com")
      .all(&pool)?;
  
  // Default scope applied automatically
  let users = User::find().all(&pool)?; // Includes default_scope
  
  // Remove default scope
  let all_users = User::unscoped().all(&pool)?;
  ```
- Scope implementation:
  - Scopes return `QueryBuilder`
  - Can be chained
  - Default scopes applied via trait

## Dependencies

- Story 05: Type-Safe Query Builders

## Notes

- Scopes improve code organization
- Reusable query fragments
- Matches ActiveRecord pattern
- Consider adding scope macros for convenience

