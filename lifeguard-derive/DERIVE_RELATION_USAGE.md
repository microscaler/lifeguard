# DeriveRelation Macro Usage Guide

## Overview

The `DeriveRelation` macro automatically generates `Related` trait implementations from a `Relation` enum definition. This eliminates the need to manually implement `Related` for each relationship.

## Basic Usage

### Simple has_many Relationship

```rust
use lifeguard_derive::DeriveRelation;

#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(has_many = "super::posts::Entity")]
    Posts,
}
```

This generates:
```rust
impl Related<super::posts::Entity> for Entity {
    fn to() -> RelationDef {
        RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(Entity::table_name().into()),
            to_tbl: TableRef::Table(super::posts::Entity::table_name().into()),
            from_col: Identity::Unary("id".into()),
            to_col: Identity::Unary("user_id".into()),
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        }
    }
}
```

### belongs_to Relationship with Metadata

```rust
#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id"
    )]
    User,
}
```

This generates:
```rust
impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        RelationDef {
            rel_type: RelationType::BelongsTo,
            from_tbl: TableRef::Table(Entity::table_name().into()),
            to_tbl: TableRef::Table(super::users::Entity::table_name().into()),
            from_col: Identity::Unary(Column::UserId.as_str().into()),
            to_col: Identity::Unary(super::users::Column::Id.as_str().into()),
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        }
    }
}

impl RelationMetadata<Entity> for super::users::Entity {
    fn foreign_key_column() -> Option<&'static str> {
        Some("user_id")  // Extracted from "Column::UserId"
    }
}
```

## Relationship Types

### has_many
One-to-many relationship. The foreign key is in the related entity's table.

```rust
#[lifeguard(has_many = "super::posts::Entity")]
Posts,
```

### has_one
One-to-one relationship. The foreign key is in the related entity's table.

```rust
#[lifeguard(has_one = "super::profile::Entity")]
Profile,
```

### belongs_to
Many-to-one relationship. The foreign key is in the current entity's table.

```rust
#[lifeguard(
    belongs_to = "super::users::Entity",
    from = "Column::UserId",
    to = "super::users::Column::Id"
)]
User,
```

## Column Metadata

The `from` and `to` attributes specify the foreign key and primary key columns:

- `from`: The foreign key column in the current entity (for belongs_to) or related entity (for has_many/has_one)
- `to`: The primary key column in the related entity

Column references use the format: `Column::ColumnName` or `super::module::Column::ColumnName`

The macro automatically converts PascalCase column names to snake_case:
- `Column::UserId` → `"user_id"`
- `Column::OwnerId` → `"owner_id"`

## Complete Example

```rust
use lifeguard_derive::{LifeModel, DeriveRelation};
use lifeguard::{Related, FindRelated, LifeModelTrait};

#[derive(LifeModel)]
#[table_name = "posts"]
pub struct Post {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    pub title: String,
    pub user_id: i32,
}

#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id"
    )]
    User,
}

// Usage:
// let post: PostModel = ...;
// let user_posts = post.find_related::<super::users::Entity>().all(executor)?;
```

## Composite Key Support

The macro now supports composite primary keys and composite foreign keys:

```rust
#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(
        belongs_to = "super::tenants::Entity",
        from = "Column::TenantId, Column::RegionId",
        to = "super::tenants::Column::Id, super::tenants::Column::RegionId"
    )]
    Tenant,
}
```

This generates a `RelationDef` with `Identity::Binary` (or `Ternary`/`Many` for 3+ columns):

```rust
impl Related<super::tenants::Entity> for Entity {
    fn to() -> RelationDef {
        RelationDef {
            rel_type: RelationType::BelongsTo,
            from_col: Identity::Binary(
                Column::TenantId.as_str().into(),
                Column::RegionId.as_str().into()
            ),
            to_col: Identity::Binary(
                super::tenants::Column::Id.as_str().into(),
                super::tenants::Column::RegionId.as_str().into()
            ),
            // ... other fields
        }
    }
}
```

## RelationDef Structure

The macro generates `RelationDef` which contains:
- `rel_type`: `RelationType::HasMany`, `HasOne`, or `BelongsTo`
- `from_tbl` / `to_tbl`: Table references
- `from_col` / `to_col`: `Identity` enum (supports single and composite keys)
- `is_owner`: Whether this entity owns the relationship
- `skip_fk`: Whether to skip foreign key constraint generation
- `on_condition`: Optional custom join condition
- `condition_type`: `ConditionType::All` or `Any`

## Limitations

1. **Entity Type**: The macro assumes `Entity` is the entity type in the same module. For entities in different modules, use the full path (e.g., `super::users::Entity`).

2. **Column Path Resolution**: When using just `Column::Name`, the macro uses `<Entity as LifeModelTrait>::Column`. For cross-module references, use the full path like `super::users::Column::Id`.

## Future Enhancements

- Support for has_many_through relationships
- Automatic default column inference improvements
- Enhanced error messages for invalid column references