# save_as on Auto-Increment Primary Keys

## Behavior Documentation

When `save_as` is used on an auto-increment primary key, the behavior depends on whether a value is explicitly provided:

### Case 1: Value is Set

If a value is explicitly set for an auto-increment PK with `save_as`:

```rust
#[derive(LifeModel)]
#[table_name = "users"]
pub struct User {
    #[primary_key]
    #[auto_increment]
    #[save_as = "gen_random_uuid()"]
    pub id: String,
}

let mut user = UserActiveModel::default();
user.id = Some("custom-id".to_string());
user.insert(&executor)?;
```

**Behavior**: The `save_as` expression is used instead of the database's auto-increment mechanism. The value provided is ignored in favor of the expression.

**SQL Generated**: 
```sql
INSERT INTO users (id) VALUES (gen_random_uuid());
```

**Note**: The explicitly set value is not used - the expression takes precedence.

### Case 2: Value is Not Set

If no value is set for an auto-increment PK with `save_as`:

```rust
let mut user = UserActiveModel::default();
// id is not set
user.insert(&executor)?;
```

**Behavior**: The `save_as` expression is used, and the database generates a value based on the expression. A RETURNING clause is NOT needed because the expression handles value generation.

**SQL Generated**:
```sql
INSERT INTO users (id) VALUES (gen_random_uuid()) RETURNING id;
```

**Note**: RETURNING is still added to capture the generated value, but the expression is what generates it, not the database's auto-increment.

## Important Considerations

1. **Expression Override**: When `save_as` is present and a value is set, the expression is used, not the set value. This may be unexpected behavior.

2. **Database Auto-Increment Disabled**: Using `save_as` on an auto-increment PK means the database's native auto-increment mechanism is bypassed. The expression is responsible for generating the value.

3. **RETURNING Clause**: RETURNING is only used when no value is set. If a value is set (even though it's ignored), RETURNING is not needed.

4. **Use Cases**: 
   - **UUID Generation**: `save_as = "gen_random_uuid()"` is a common use case
   - **Custom Sequences**: `save_as = "nextval('custom_seq')"` for custom sequences
   - **Timestamp-based IDs**: `save_as = "extract(epoch from now())::bigint"` for timestamp-based IDs

## Recommendations

1. **Avoid Setting Values**: When using `save_as` on auto-increment PKs, avoid explicitly setting the value. Let the expression handle it.

2. **Document Behavior**: If using `save_as` on auto-increment PKs, document that the expression takes precedence over any set values.

3. **Consider Alternatives**: For UUID generation, consider using `default_expr` instead of `save_as` if you want the database to handle it at the schema level.

## Implementation Details

The implementation in `life_record.rs` handles this as follows:

1. **Value Set**: If value is set, `save_as` expression is used (if present), otherwise the value is used
2. **Value Not Set**: If value is not set, RETURNING clause is added to capture the generated value
3. **Expression Priority**: `save_as` expression always takes precedence when present and value is set

This behavior is consistent with SeaORM's approach where expressions override values.
