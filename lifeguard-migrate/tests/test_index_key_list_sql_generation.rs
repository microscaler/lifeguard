//! Generated CREATE INDEX uses `IndexDefinition::key_list_sql` when set (derive `expr | cols` form).

#![allow(warnings)]

use lifeguard::LifeModelTrait;
use lifeguard_derive::LifeModel;
use lifeguard_migrate::sql_generator;

#[test]
fn generate_sql_emits_expression_index_key_list() {
    #[derive(LifeModel)]
    #[table_name = "users"]
    #[index = "idx_users_lower_email(lower(email) | email)"]
    pub struct UserRow {
        #[primary_key]
        pub id: i32,
        pub email: String,
    }

    let sql = sql_generator::generate_create_table_sql::<Entity>(Entity::table_definition())
        .expect("sql");

    assert!(
        sql.contains("CREATE INDEX idx_users_lower_email ON users(lower(email));"),
        "expected expression key in CREATE INDEX, got:\n{sql}"
    );
}

#[test]
fn generate_sql_emits_structured_column_desc() {
    #[derive(LifeModel)]
    #[table_name = "posts"]
    #[index = "idx_posts_created(created_at DESC NULLS LAST)"]
    pub struct PostRow {
        #[primary_key]
        pub id: i32,
        #[column_type = "TIMESTAMP"]
        pub created_at: chrono::NaiveDateTime,
    }

    let sql = sql_generator::generate_create_table_sql::<Entity>(Entity::table_definition())
        .expect("sql");

    assert!(
        sql.contains("CREATE INDEX idx_posts_created ON posts(created_at DESC NULLS LAST);"),
        "expected structured DESC+NULLS in CREATE INDEX, got:\n{sql}"
    );
}

#[test]
fn generate_sql_emits_opclass_in_index_key_list() {
    #[derive(LifeModel)]
    #[table_name = "articles"]
    #[index = "idx_articles_slug(slug text_pattern_ops)"]
    pub struct ArticleRow {
        #[primary_key]
        pub id: i32,
        #[column_type = "TEXT"]
        pub slug: String,
    }

    let sql = sql_generator::generate_create_table_sql::<Entity>(Entity::table_definition())
        .expect("sql");

    assert!(
        sql.contains("CREATE INDEX idx_articles_slug ON articles(slug text_pattern_ops);"),
        "expected opclass in key list, got:\n{sql}"
    );
}
