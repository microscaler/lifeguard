//! **`find_related` + related-side scope** (PRD §0.3 row 2, SC-1)
//!
//! Parent scopes from [`UserEntity::find`](lifeguard::query::traits::LifeModelTrait::find) **are not**
//! merged into [`FindRelated::find_related`](lifeguard::FindRelated::find_related). Constrain the
//! **related** row set by chaining [`.scope`](lifeguard::SelectQuery::scope) (or [`.filter`](lifeguard::SelectQuery::filter))
//! on the [`SelectQuery`](lifeguard::SelectQuery) returned from `find_related`, or use
//! [`FindRelated::find_related_scoped`](lifeguard::FindRelated::find_related_scoped).
//!
//! See [`docs/planning/DESIGN_FIND_RELATED_SCOPES.md`](../docs/planning/DESIGN_FIND_RELATED_SCOPES.md) and
//! [`docs/planning/PRD_FOLLOWON_NEXT_THREE.md`](../docs/planning/PRD_FOLLOWON_NEXT_THREE.md).
//!
//! Run (compile check only; no database required for this binary):
//! `cargo check --example find_related_scope_example`

use lifeguard::relation::identity::Identity;
use lifeguard::{ColumnTrait, FindRelated, Related, RelationDef, RelationType};
use lifeguard_derive::{LifeModel, LifeRecord};
use sea_query::{ConditionType, IntoIden, TableName, TableRef};

mod users {
    #![allow(clippy::wildcard_imports)] // derive-generated code expects parent prelude
    use super::*;

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "example_scope_demo_users"]
    #[allow(dead_code)] // Constructed via `ExampleScopeUserModel` / derive; table shape for Entity.
    pub struct ExampleScopeUser {
        #[primary_key]
        #[auto_increment]
        pub id: i32,
        pub name: String,
        pub email: String,
    }
}

mod posts {
    #![allow(clippy::wildcard_imports)]
    use super::*;

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "example_scope_demo_posts"]
    #[allow(dead_code)]
    pub struct ExampleScopePost {
        #[primary_key]
        #[auto_increment]
        pub id: i32,
        pub title: String,
        pub content: String,
        pub user_id: i32,
    }
}

use posts::Column as PostColumn;
use posts::Entity as PostEntity;
use users::Entity as UserEntity;
use users::ExampleScopeUserModel;

impl Related<UserEntity> for PostEntity {
    fn to() -> RelationDef {
        RelationDef {
            rel_type: RelationType::BelongsTo,
            from_tbl: TableRef::Table(
                TableName(None, "example_scope_demo_posts".into_iden()),
                None,
            ),
            to_tbl: TableRef::Table(
                TableName(None, "example_scope_demo_users".into_iden()),
                None,
            ),
            from_col: Identity::Unary("user_id".into()),
            to_col: Identity::Unary("id".into()),
            through_tbl: None,
            through_from_col: None,
            through_to_col: None,
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        }
    }
}

impl Related<PostEntity> for UserEntity {
    fn to() -> RelationDef {
        RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(
                TableName(None, "example_scope_demo_users".into_iden()),
                None,
            ),
            to_tbl: TableRef::Table(
                TableName(None, "example_scope_demo_posts".into_iden()),
                None,
            ),
            from_col: Identity::Unary("id".into()),
            to_col: Identity::Unary("user_id".into()),
            through_tbl: None,
            through_from_col: None,
            through_to_col: None,
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        }
    }
}

fn main() {
    let user = ExampleScopeUserModel {
        id: 1,
        name: "Ada".to_string(),
        email: "ada@example.com".to_string(),
    };

    // Related-side filter only: restricts rows in `example_scope_demo_posts`, not `..._users`.
    let _posts: lifeguard::SelectQuery<PostEntity> = user
        .find_related::<PostEntity>()
        .expect("find_related")
        .scope(ColumnTrait::eq(PostColumn::Title, "Draft"));

    // Equivalent one-call form:
    let _posts2: lifeguard::SelectQuery<PostEntity> = user
        .find_related_scoped(ColumnTrait::eq(PostColumn::Title, "Draft"))
        .expect("find_related_scoped");

    // For predicates on the parent (`users`) table in the same SQL as the related load, use
    // `find_related_parent_scoped` (direct edges only) — see crate rustdoc on `FindRelated`.
}
