# Story 16: Session/Unit of Work Pattern

## Description

Implement Session/Unit of Work pattern that provides identity map, automatic change tracking, and session-level transaction management. This matches SQLAlchemy's session pattern.

## Acceptance Criteria

- [ ] `Session` type that wraps LifeguardPool
- [ ] Identity map (one LifeModel instance per primary key per session)
- [ ] Automatic dirty tracking (track changes to models)
- [ ] Session-level transaction management
- [ ] `session.commit()` - Commit all changes
- [ ] `session.rollback()` - Rollback all changes
- [ ] `session.refresh(model)` - Reload from database
- [ ] Unit tests demonstrate session pattern

## Technical Details

- Session API:
  ```rust
  let session = Session::new(&pool);
  
  // Identity map ensures one instance per PK
  let user1 = User::find_by_id(&session, 1)?;
  let user2 = User::find_by_id(&session, 1)?;
  assert!(std::ptr::eq(&user1, &user2)); // Same instance
  
  // Automatic change tracking
  let mut user = user1;
  user.email = "new@example.com".to_string();
  session.save(&user)?; // Automatically tracks change
  
  // Commit all changes
  session.commit()?;
  ```
- Identity map:
  - Key: `(table_name, primary_key)`
  - Value: `Arc<LifeModel>`
  - Ensures one instance per PK per session
- Dirty tracking:
  - Track modified fields
  - Only update changed fields
  - Automatic SQL generation

## Dependencies

- Story 03: Implement Basic CRUD Operations
- Epic 01: Story 06 (Transaction Support)

## Notes

- Session pattern is essential for complex applications
- Reduces queries (identity map)
- Simplifies transaction management
- Matches SQLAlchemy's approach

