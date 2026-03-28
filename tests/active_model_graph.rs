//! Integration tests for ActiveModel Nested Graph Persistence (`save_graph`)
//!
//! These tests validate that the recursive topological graph walking logic
//! correctly identifies hierarchies, inserts them in the correct transaction order
//! (BelongsTo -> Root -> HasMany), and dynamically propagates newly assigned
//! auto-increment sequences down strings of Primary and Foreign key dependencies.
//!
//! Note: These tests require a running PostgreSQL database. Set TEST_DATABASE_URL
//! environment variable or use the test infrastructure from test_helpers.

use lifeguard::{
    ActiveModelTrait, LifeExecutor, MayPostgresExecutor,
    test_helpers::TestDatabase,
};
use lifeguard_derive::{LifeModel, LifeRecord};


mod context;

fn get_db() -> TestDatabase {
    let ctx = context::get_test_context();
    TestDatabase::with_url(&ctx.pg_url)
}

pub mod org_mod {
    use super::*;
    // 1. BelongsTo (Organization)
    #[derive(LifeModel, LifeRecord, Clone, Debug)]
    #[table_name = "test_organizations"]
    pub struct Organization {
        #[primary_key]
        #[auto_increment]
        pub id: i32,
        pub name: String,
    }
}
pub use org_mod::*;

pub mod user_mod {
    use super::*;
    // 2. Root (User)
    #[derive(LifeModel, LifeRecord, Clone, Debug)]
    #[table_name = "test_users_graph"]
    pub struct User {
        #[primary_key]
        #[auto_increment]
        pub user_id: i32,
        pub username: String,
        pub organization_id: i32, // FK to Organizations
    }
    
    // Manual mapping for topological testing since macro parsing isn't finished yet
    impl lifeguard::Related<org_mod::Entity> for Entity {
        fn to() -> lifeguard::relation::RelationDef {
            lifeguard::relation::RelationDef {
                rel_type: lifeguard::relation::RelationType::BelongsTo,
                from_tbl: sea_query::DynIden::from("test_users_graph").into(),
                to_tbl: sea_query::DynIden::from("test_organizations").into(),
                from_col: lifeguard::relation::identity::Identity::Unary(sea_query::DynIden::from("organization_id")),
                to_col: lifeguard::relation::identity::Identity::Unary(sea_query::DynIden::from("id")),
                is_owner: false,
                skip_fk: false,
                through_from_col: None,
                through_to_col: None,
                through_tbl: None,
                on_condition: None,
                condition_type: sea_query::ConditionType::Any,
            }
        }
    }
}
pub use user_mod::*;

pub mod post_mod {
    use super::*;
    // 3. HasMany (Post)
    #[derive(LifeModel, LifeRecord, Clone, Debug)]
    #[table_name = "test_posts"]
    pub struct Post {
        #[primary_key]
        #[auto_increment]
        pub id: i32,
        pub title: String,
        pub author_id: i32, // FK to Users
    }

    impl lifeguard::Related<post_mod::Entity> for user_mod::Entity {
        fn to() -> lifeguard::relation::RelationDef {
            lifeguard::relation::RelationDef {
                rel_type: lifeguard::relation::RelationType::HasMany,
                from_tbl: sea_query::DynIden::from("test_users_graph").into(),
                to_tbl: sea_query::DynIden::from("test_posts").into(),
                from_col: lifeguard::relation::identity::Identity::Unary(sea_query::DynIden::from("user_id")),
                to_col: lifeguard::relation::identity::Identity::Unary(sea_query::DynIden::from("author_id")),
                is_owner: false,
                skip_fk: false,
                through_from_col: None,
                through_to_col: None,
                through_tbl: None,
                on_condition: None,
                condition_type: sea_query::ConditionType::Any,
            }
        }
    }
}
pub use post_mod::*;

// Helper function to set up test database schema
fn setup_test_schema(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_organizations (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL
        )
        "#,
        &[],
    )?;
    executor.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_users_graph (
            user_id SERIAL PRIMARY KEY,
            username TEXT NOT NULL,
            organization_id INTEGER NOT NULL REFERENCES test_organizations(id)
        )
        "#,
        &[],
    )?;
    executor.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_posts (
            id SERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            author_id INTEGER NOT NULL REFERENCES test_users_graph(user_id)
        )
        "#,
        &[],
    )?;
    Ok(())
}

fn cleanup_test_data(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute("DROP TABLE IF EXISTS test_posts CASCADE", &[])?;
    executor.execute("DROP TABLE IF EXISTS test_users_graph CASCADE", &[])?;
    executor.execute("DROP TABLE IF EXISTS test_organizations CASCADE", &[])?;
    Ok(())
}

#[test]
fn test_nested_graph_persistence() {
    let mut test_db = get_db();
    let _client = test_db.connect().expect("Failed to connect to database");
    let executor = test_db.executor().expect("Failed to create executor");

    cleanup_test_data(&executor).expect("Clean previous");
    setup_test_schema(&executor).expect("Failed to setup schema");

    // 1. Build Parent Organization
    let mut org = OrganizationRecord::new();
    org.set_name("ACME Corp".to_string());

    // 2. Build Root User
    let mut user = UserRecord::new();
    user.set_username("Alice".to_string());
    user.set_organization_id(0); // Will be automatically overwritten by save_graph topological mapping!

    // 3. Build HasMany Posts
    let mut post1 = PostRecord::new();
    post1.set_title("First Post".to_string());
    post1.set_author_id(0); // Will be overwritten

    let mut post2 = PostRecord::new();
    post2.set_title("Second Post".to_string());
    post2.set_author_id(0); // Will be overwritten

    // 4. Connect the Graph
    user.set_parent(org);
    user.add_child(post1);
    user.add_child(post2);

    // 5. Execute Single-Transaction Graph Persistence
    let saved_user = user.save_graph(&executor).expect("Failed to execute topological graph save");

    // 6. Verification
    assert!(saved_user.user_id > 0, "Root model should have an auto-incremented ID");
    assert!(saved_user.organization_id > 0, "Root model should have dynamically acquired parent ID");
    
    // Verify DB State
    let org_rows = executor.query_all("SELECT id, name FROM test_organizations", &[]).unwrap();
    assert_eq!(org_rows.len(), 1);
    assert_eq!(org_rows[0].get::<_, i32>(0), saved_user.organization_id);

    let post_rows = executor.query_all("SELECT author_id FROM test_posts", &[]).unwrap();
    assert_eq!(post_rows.len(), 2);
    assert_eq!(post_rows[0].get::<_, i32>(0), saved_user.user_id);
    assert_eq!(post_rows[1].get::<_, i32>(0), saved_user.user_id);

    cleanup_test_data(&executor).expect("Failed to cleanup");
}
