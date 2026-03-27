# Story 06: Relations - Complete SeaORM Parity

## Description

Implement complete relation support for LifeModel, replicating all SeaORM relation features. This includes Relation enum generation, Related trait implementation, join operations, and eager/lazy loading.

## Acceptance Criteria

- [ ] `#[derive(LifeRelation)]` macro generates Relation enum (replicates `DeriveRelation`)
- [ ] Relation enum supports: `has_one`, `has_many`, `belongs_to`, `many_to_many`
- [ ] `impl Related<OtherEntity> for Entity` trait implementation
- [ ] Relation attributes: `from`, `to`, `on_update`, `on_delete`
- [ ] Eager loading: `User::find().with(Post::Author).all(pool)`
- [ ] Lazy loading: `user.posts(pool)` (loads on demand)
- [ ] Join operations: `join()`, `left_join()`, `right_join()`, `inner_join()`, `join_rev()`
- [ ] N+1 query prevention (batch loading)
- [ ] Many-to-many via join table
- [ ] Cascade behaviors: `NoAction`, `Cascade`, `SetNull`, `SetDefault`, `Restrict`
- [ ] Unit tests demonstrate all relation features

## Technical Details

### Relation Enum Generation
- Generate enum with variants for each relation
- Attributes:
  - `#[has_one = "Entity"]` - One-to-one
  - `#[has_many = "Entity"]` - One-to-many
  - `#[belongs_to = "Entity", from = "Column", to = "Column"]` - Many-to-one
  - `#[many_to_many = "Entity", via = "JoinTable"]` - Many-to-many

### Related Trait
```rust
impl Related<super::posts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Posts.def()
    }
}
```

### Join Operations
- `find().join(JoinType::InnerJoin, Post::Entity)` - JOIN related entity
- `find().left_join(Post::Entity)` - LEFT JOIN
- `find().right_join(Post::Entity)` - RIGHT JOIN
- `find().inner_join(Post::Entity)` - INNER JOIN
- `find().join_rev(JoinType::InnerJoin, Post::Entity)` - Reverse join

### Loading Strategies (SQLAlchemy-style)
- `joinedload()` - Single query with JOINs (eager, one query)
- `subqueryload()` - Separate query per relation (eager, multiple queries)
- `selectinload()` - IN clause for batch loading (eager, prevents N+1)
- `noload()` - No loading (lazy, raises error on access)
- `raiseload()` - Raise error on access (lazy, explicit failure)
- Lazy loading: separate query on access (default for some relations)

### Cascade Behaviors
- `on_update`: `NoAction`, `Cascade`, `SetNull`, `SetDefault`, `Restrict`
- `on_delete`: `NoAction`, `Cascade`, `SetNull`, `SetDefault`, `Restrict`

## Dependencies

- Epic 02: ORM Core (LifeModel, LifeRecord must exist)
- Story 05: Type-Safe Query Builders (for join operations)

## Notes

- Relations are essential for ORM usability
- Complete SeaORM API parity is critical
- N+1 prevention is critical for performance
- Consider adding relation caching in future

