//! Composite primary keys arrive as one table-level clause.
//!
//! A second inline `PRIMARY KEY` column clause is invalid PostgreSQL, and composite key members
//! carry caller-supplied values (join-table ids, foreign keys), so none may take a generated
//! default. Regression for the CRM audience `contact_tags` / `bulk_job_changes` generation.

#![allow(warnings)]

use lifeguard::LifeModelTrait;
use lifeguard_migrate::sql_generator;

// One entity per module: the derive emits `Entity`/`Column` at module scope and two would collide.
mod contact_tag {
    use lifeguard_derive::LifeModel;

    #[derive(LifeModel)]
    #[table_name = "contact_tags"]
    #[check = "workspace_id <> tag_id"]
    pub struct ContactTag {
        pub workspace_id: uuid::Uuid,

        #[primary_key]
        #[foreign_key = "contacts(id) ON DELETE CASCADE"]
        pub contact_id: uuid::Uuid,

        #[primary_key]
        #[foreign_key = "tags(id) ON DELETE CASCADE"]
        pub tag_id: uuid::Uuid,

        #[default_expr = "'USER'"]
        #[check = "source IN ('USER', 'FORM')"]
        pub source: String,
    }
}

mod single_pk {
    use lifeguard_derive::LifeModel;

    #[derive(LifeModel)]
    #[table_name = "single_pk"]
    pub struct SinglePk {
        #[primary_key]
        pub id: uuid::Uuid,

        pub label: String,
    }
}

#[test]
fn composite_primary_key_is_a_table_level_clause() {
    use contact_tag::Entity;
    let sql = sql_generator::generate_create_table_sql::<Entity>(Entity::table_definition())
        .expect("generate composite-key SQL");
    assert!(
        sql.contains("PRIMARY KEY (contact_id, tag_id)"),
        "composite key must arrive as one table-level clause:\n{sql}"
    );
    assert!(
        !sql.contains(" UUID PRIMARY KEY"),
        "no inline PRIMARY KEY may survive on composite members:\n{sql}"
    );
    assert!(
        !sql.contains("gen_random_uuid"),
        "composite members carry caller-supplied ids, never generated defaults:\n{sql}"
    );
    // A table-level CHECK constraint follows the PK clause inside the CREATE, comma-separated.
    assert!(
        sql.contains("PRIMARY KEY (contact_id, tag_id),\n    CONSTRAINT check_contact_tags CHECK"),
        "a following table-level constraint must be comma-separated:\n{sql}"
    );
}

#[test]
fn single_primary_key_keeps_the_inline_style() {
    use single_pk::Entity;
    let sql = sql_generator::generate_create_table_sql::<Entity>(Entity::table_definition())
        .expect("generate single-key SQL");
    assert!(
        sql.contains("id UUID PRIMARY KEY DEFAULT gen_random_uuid()"),
        "single-column keys keep the inline style and generated default:\n{sql}"
    );
    assert!(
        !sql.contains("PRIMARY KEY ("),
        "no table-level clause for a single key:\n{sql}"
    );
}
