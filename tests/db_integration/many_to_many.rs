//! Many-to-many (`has_many_through`) `find_related` and `find_related_parent_scoped`: the related
//! rows come through the junction table, exactly those linked to the source row.

use lifeguard::relation::identity::Identity;
use lifeguard::{FindRelated, LifeModelTrait, Related, RelationDef, RelationType};
use sea_query::{ConditionType, Expr, ExprTrait, IntoIden, TableName, TableRef};

pub mod m2m_posts {
    use lifeguard_derive::{LifeModel, LifeRecord};

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_m2m_posts"]
    pub struct M2mPost {
        #[primary_key]
        pub id: i32,
        pub title: String,
    }
}

pub mod m2m_tags {
    use lifeguard_derive::{LifeModel, LifeRecord};

    #[derive(LifeModel, LifeRecord)]
    #[table_name = "test_m2m_tags"]
    pub struct M2mTag {
        #[primary_key]
        pub id: i32,
        pub name: String,
    }
}

use m2m_posts::Entity as PostEntity;
use m2m_tags::Entity as TagEntity;

fn table(name: &'static str) -> TableRef {
    TableRef::Table(TableName(None, name.into_iden()), None)
}

impl Related<TagEntity> for PostEntity {
    fn to() -> RelationDef {
        RelationDef {
            rel_type: RelationType::HasManyThrough,
            from_tbl: table("test_m2m_posts"),
            to_tbl: table("test_m2m_tags"),
            from_col: Identity::Unary("id".into()),
            to_col: Identity::Unary("id".into()),
            through_tbl: Some(table("test_m2m_post_tags")),
            through_from_col: Some(Identity::Unary("post_id".into())),
            through_to_col: Some(Identity::Unary("tag_id".into())),
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        }
    }
}

/// Posts 1..=3 and tags 10..=14. Post 1 links tags 10, 11, 12; post 2 links 11, 12, 13
/// (overlapping 11 and 12); post 3 links none; tag 14 is linked to nothing. The junction also
/// has an `id` column, like a typical surrogate key, which must not leak into the tags read.
fn executor() -> lifeguard::MayPostgresExecutor {
    static SCHEMA: std::sync::Once = std::sync::Once::new();
    let ctx = crate::context::get_test_context();
    SCHEMA.call_once(|| {
        let c = may_postgres::connect(&ctx.pg_url).expect("connect");
        c.batch_execute(
            "DROP TABLE IF EXISTS test_m2m_post_tags, test_m2m_posts, test_m2m_tags;
             CREATE TABLE test_m2m_posts (id INT PRIMARY KEY, title TEXT NOT NULL);
             CREATE TABLE test_m2m_tags (id INT PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE test_m2m_post_tags (
               id SERIAL PRIMARY KEY,
               post_id INT NOT NULL REFERENCES test_m2m_posts(id),
               tag_id INT NOT NULL REFERENCES test_m2m_tags(id),
               UNIQUE (post_id, tag_id));
             INSERT INTO test_m2m_posts VALUES (1, 'first'), (2, 'second'), (3, 'lonely');
             INSERT INTO test_m2m_tags VALUES (10, 't10'), (11, 't11'), (12, 't12'), (13, 't13'), (14, 't14');
             INSERT INTO test_m2m_post_tags (post_id, tag_id) VALUES
               (1, 10), (1, 11), (1, 12), (2, 11), (2, 12), (2, 13);",
        )
        .expect("schema");
    });
    let mut db = lifeguard::test_helpers::TestDatabase::with_url(&ctx.pg_url);
    let _c = db.connect().expect("connect");
    db.executor().expect("executor")
}

fn post(exec: &lifeguard::MayPostgresExecutor, id: i32) -> m2m_posts::M2mPostModel {
    PostEntity::find()
        .filter(Expr::col("id").eq(id))
        .one(exec)
        .expect("post")
}

fn tag_ids(mut tags: Vec<m2m_tags::M2mTagModel>) -> Vec<(i32, String)> {
    tags.sort_by_key(|t| t.id);
    tags.into_iter().map(|t| (t.id, t.name)).collect()
}

#[test]
fn find_related_through_returns_exactly_the_linked_rows() {
    let exec = executor();
    let p1 = tag_ids(
        post(&exec, 1)
            .find_related::<TagEntity>()
            .expect("q")
            .all(&exec)
            .expect("all"),
    );
    assert_eq!(
        p1,
        vec![(10, "t10".into()), (11, "t11".into()), (12, "t12".into())],
        "id and name come from the tag row, not the junction's id"
    );
    let p2 = tag_ids(
        post(&exec, 2)
            .find_related::<TagEntity>()
            .expect("q")
            .all(&exec)
            .expect("all"),
    );
    assert_eq!(
        p2,
        vec![(11, "t11".into()), (12, "t12".into()), (13, "t13".into())]
    );
    let p3 = post(&exec, 3)
        .find_related::<TagEntity>()
        .expect("q")
        .all(&exec)
        .expect("all");
    assert!(p3.is_empty(), "no links, no tags");
}

#[test]
fn find_related_parent_scoped_through_applies_the_parent_scope() {
    let exec = executor();
    let p1 = post(&exec, 1);
    let kept = p1
        .find_related_parent_scoped::<TagEntity, _>(
            Expr::col(("test_m2m_posts", "title")).eq("first"),
        )
        .expect("q")
        .all(&exec)
        .expect("all");
    assert_eq!(
        tag_ids(kept),
        vec![(10, "t10".into()), (11, "t11".into()), (12, "t12".into())]
    );
    let none = p1
        .find_related_parent_scoped::<TagEntity, _>(
            Expr::col(("test_m2m_posts", "title")).eq("second"),
        )
        .expect("q")
        .all(&exec)
        .expect("all");
    assert!(none.is_empty(), "the parent scope excludes post 1");
}
