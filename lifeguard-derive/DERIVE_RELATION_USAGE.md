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

## Limitations

1. **Composite Primary Keys**: Full support for composite primary keys in `find_related()` requires additional metadata and is a future enhancement.

2. **RelationMetadata Usage**: The `RelationMetadata` trait is generated but not yet used in `find_related()` due to trait bound limitations. This will be enhanced in a future version.

3. **Entity Type**: The macro assumes `Entity` is the entity type in the same module. For entities in different modules, use the full path (e.g., `super::users::Entity`).

## Future Enhancements

- Automatic join condition generation from metadata
- Composite primary key support
- Runtime use of RelationMetadata in find_related()
- Support for has_many_through relationships
