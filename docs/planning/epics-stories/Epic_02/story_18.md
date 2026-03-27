# Story 18: Model Managers

## Description

Implement model managers that allow custom query methods on models. This matches Django ORM's manager pattern and enables better encapsulation.

## Acceptance Criteria

- [ ] Custom manager traits
- [ ] Model-level query methods
- [ ] Chainable manager methods
- [ ] Default managers
- [ ] Custom managers per model
- [ ] Unit tests demonstrate manager usage

## Technical Details

- Manager trait:
  ```rust
  pub trait LifeModelManager<T> {
      fn objects(&self) -> QueryBuilder;
      fn create(&self, pool: &LifeguardPool, data: LifeRecord) -> Result<T>;
      // Custom methods
  }
  ```
- Manager implementation:
  ```rust
  impl UserManager for User {
      fn active_users(&self, pool: &LifeguardPool) -> Result<Vec<User>> {
          User::find()
              .filter(User::IsActive.eq(true))
              .all(pool)
      }
      
      fn by_email_domain(&self, pool: &LifeguardPool, domain: &str) -> Result<Vec<User>> {
          User::find()
              .filter(User::Email.like(&format!("%@{}", domain)))
              .all(pool)
      }
  }
  ```
- Usage:
  ```rust
  let active_users = User::manager().active_users(&pool)?;
  let gmail_users = User::manager().by_email_domain(&pool, "gmail.com")?;
  ```

## Dependencies

- Story 05: Type-Safe Query Builders

## Notes

- Managers provide better encapsulation
- Custom query methods on models
- Matches Django ORM pattern
- Consider adding manager macros

