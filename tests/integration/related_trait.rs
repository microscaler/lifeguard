//! Integration tests for Related and FindRelated traits
//!
//! These tests validate that the Related trait and FindRelated extension trait
//! work correctly with a real PostgreSQL database.
//!
//! Test relationships:
//! - User has_many Posts (one-to-many)
//! - Post belongs_to User (many-to-one)

use lifeguard::{
    ActiveModelTrait, FindRelated, LifeModelTrait, LifeExecutor, MayPostgresExecutor,
    Related, SelectQuery, test_helpers::TestDatabase, ModelTrait, LifeEntityName,
    RelationDef, RelationType,
};
use lifeguard::relation::identity::Identity;
use sea_query::{TableRef, TableName, ConditionType, IntoIden};
use lifeguard_derive::{LifeModel, LifeRecord};
use sea_query::{Expr, Iden, IdenStatic};

// ============================================================================
// Test Entities
// ============================================================================

#[derive(LifeModel, LifeRecord)]
#[table_name = "test_users_related"]
pub struct TestUser {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    pub name: String,
    pub email: String,
}

#[derive(LifeModel, LifeRecord)]
#[table_name = "test_posts_related"]
pub struct TestPost {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    pub title: String,
    pub content: String,
    pub user_id: i32, // Foreign key to test_users_related
}

// ============================================================================
// Entity and Column Definitions for Related Trait
// ============================================================================
// Note: LifeModel macro generates TestUserEntity, TestUserModel, TestUserColumn, etc.
// We use those generated types here.

// ============================================================================
// Related Trait Implementation
// ============================================================================

// Post belongs_to User (many-to-one)
// This means: Post has a foreign key user_id that references User.id
impl Related<TestUserEntity> for TestPostEntity {
    fn to() -> RelationDef {
        RelationDef {
            rel_type: RelationType::BelongsTo,
            from_tbl: TableRef::Table(TableName(None, "test_posts_related".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "test_users_related".into_iden()), None),
            from_col: Identity::Unary("user_id".into()),
            to_col: Identity::Unary("id".into()),
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        }
    }
}

// User has_many Posts (one-to-many)
// This means: User.id is referenced by Post.user_id
impl Related<TestPostEntity> for TestUserEntity {
    fn to() -> RelationDef {
        RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(TableName(None, "test_users_related".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "test_posts_related".into_iden()), None),
            from_col: Identity::Unary("id".into()),
            to_col: Identity::Unary("user_id".into()),
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        }
    }
}

// ============================================================================
// Test Helpers
// ============================================================================

fn setup_test_schema(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    // Create users table
    executor.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_users_related (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL
        )
        "#,
        &[],
    )?;

    // Create posts table
    executor.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_posts_related (
            id SERIAL PRIMARY KEY,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            user_id INTEGER NOT NULL REFERENCES test_users_related(id)
        )
        "#,
        &[],
    )?;

    Ok(())
}

fn cleanup_test_data(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute("DELETE FROM test_posts_related", &[])?;
    executor.execute("DELETE FROM test_users_related", &[])?;
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn test_related_trait_to_method() {
    // Test that Related::to() returns a RelationDef
    let rel_def: RelationDef = TestPostEntity::to();
    // Just verify it compiles and returns a RelationDef
    let _ = rel_def;
    
    let rel_def: RelationDef = TestUserEntity::to();
    let _ = rel_def;
}

#[test]
fn test_find_related_returns_query() {
    // Test that find_related() returns a SelectQuery
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a user
    let mut user_record = TestUserRecord::new();
    user_record.set_name("Test User".to_string());
    user_record.set_email("test@example.com".to_string());
    let user = user_record.insert(&executor).expect("Failed to insert user");

    // Test that find_related() returns a query
    let query = user.find_related::<TestPostEntity>();
    let _ = query; // Just verify it compiles
}

#[test]
fn test_find_related_has_many_relationship() {
    // Test has_many relationship: User has_many Posts
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a user
    let mut user_record = TestUserRecord::new();
    user_record.set_name("John Doe".to_string());
    user_record.set_email("john@example.com".to_string());
    let user = user_record.insert(&executor).expect("Failed to insert user");

    // Create posts for this user
    let mut post1_record = TestPostRecord::new();
    post1_record.set_title("First Post".to_string());
    post1_record.set_content("Content of first post".to_string());
    post1_record.set_user_id(user.id);
    let _post1 = post1_record.insert(&executor).expect("Failed to insert post1");

    let mut post2_record = TestPostRecord::new();
    post2_record.set_title("Second Post".to_string());
    post2_record.set_content("Content of second post".to_string());
    post2_record.set_user_id(user.id);
    let _post2 = post2_record.insert(&executor).expect("Failed to insert post2");

    // Create a post for a different user
    let mut other_user_record = TestUserRecord::new();
    other_user_record.set_name("Jane Doe".to_string());
    other_user_record.set_email("jane@example.com".to_string());
    let other_user = other_user_record.insert(&executor).expect("Failed to insert other user");

    let mut post3_record = TestPostRecord::new();
    post3_record.set_title("Other User's Post".to_string());
    post3_record.set_content("Content of other post".to_string());
    post3_record.set_user_id(other_user.id);
    let _post3 = post3_record.insert(&executor).expect("Failed to insert post3");

    // Find all posts for the first user using find_related()
    let posts = user.find_related::<TestPostEntity>()
        .all(&executor)
        .expect("Failed to query related posts");

    // Should return exactly 2 posts (post1 and post2, not post3)
    assert_eq!(posts.len(), 2, "Should find 2 posts for the user");
    
    // Verify the posts belong to the correct user
    for post in &posts {
        assert_eq!(post.user_id, user.id, "Post should belong to the user");
    }

    // Verify post titles
    let titles: Vec<&str> = posts.iter().map(|p| p.title.as_str()).collect();
    assert!(titles.contains(&"First Post"), "Should contain first post");
    assert!(titles.contains(&"Second Post"), "Should contain second post");
    assert!(!titles.contains(&"Other User's Post"), "Should not contain other user's post");
}

#[test]
fn test_find_related_empty_result() {
    // Test find_related() when there are no related entities
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a user with no posts
    let mut user_record = TestUserRecord::new();
    user_record.set_name("Lonely User".to_string());
    user_record.set_email("lonely@example.com".to_string());
    let user = user_record.insert(&executor).expect("Failed to insert user");

    // Find related posts (should be empty)
    let posts = user.find_related::<TestPostEntity>()
        .all(&executor)
        .expect("Failed to query related posts");

    assert_eq!(posts.len(), 0, "Should find no posts for user with no posts");
}

#[test]
fn test_find_related_multiple_users() {
    // Test that find_related() correctly filters by the specific user's ID
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create multiple users
    let mut user1_record = TestUserRecord::new();
    user1_record.set_name("User 1".to_string());
    user1_record.set_email("user1@example.com".to_string());
    let user1 = user1_record.insert(&executor).expect("Failed to insert user1");

    let mut user2_record = TestUserRecord::new();
    user2_record.set_name("User 2".to_string());
    user2_record.set_email("user2@example.com".to_string());
    let user2 = user2_record.insert(&executor).expect("Failed to insert user2");

    // Create posts for each user
    let mut post1_record = TestPostRecord::new();
    post1_record.set_title("User 1 Post".to_string());
    post1_record.set_content("Content".to_string());
    post1_record.set_user_id(user1.id);
    let _post1 = post1_record.insert(&executor).expect("Failed to insert post1");

    let mut post2_record = TestPostRecord::new();
    post2_record.set_title("User 2 Post".to_string());
    post2_record.set_content("Content".to_string());
    post2_record.set_user_id(user2.id);
    let _post2 = post2_record.insert(&executor).expect("Failed to insert post2");

    // Find posts for user1
    let user1_posts = user1.find_related::<TestPostEntity>()
        .all(&executor)
        .expect("Failed to query user1 posts");
    assert_eq!(user1_posts.len(), 1, "User1 should have 1 post");
    assert_eq!(user1_posts[0].user_id, user1.id, "Post should belong to user1");

    // Find posts for user2
    let user2_posts = user2.find_related::<TestPostEntity>()
        .all(&executor)
        .expect("Failed to query user2 posts");
    assert_eq!(user2_posts.len(), 1, "User2 should have 1 post");
    assert_eq!(user2_posts[0].user_id, user2.id, "Post should belong to user2");
}

#[test]
fn test_find_related_with_query_modifications() {
    // Test that find_related() returns a query that can be further modified
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a user
    let mut user_record = TestUserRecord::new();
    user_record.set_name("Test User".to_string());
    user_record.set_email("test@example.com".to_string());
    let user = user_record.insert(&executor).expect("Failed to insert user");

    // Create multiple posts
    let mut post1_record = TestPostRecord::new();
    post1_record.set_title("Post A".to_string());
    post1_record.set_content("Content A".to_string());
    post1_record.set_user_id(user.id);
    let _post1 = post1_record.insert(&executor).expect("Failed to insert post1");

    let mut post2_record = TestPostRecord::new();
    post2_record.set_title("Post B".to_string());
    post2_record.set_content("Content B".to_string());
    post2_record.set_user_id(user.id);
    let _post2 = post2_record.insert(&executor).expect("Failed to insert post2");

    // Find related posts and limit to 1
    let posts = user.find_related::<TestPostEntity>()
        .limit(1)
        .all(&executor)
        .expect("Failed to query related posts");

    assert_eq!(posts.len(), 1, "Should return only 1 post when limit is 1");
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_find_related_with_nonexistent_user_id() {
    // Test find_related() with a user that doesn't exist in the database
    // This should still work - it will just return an empty result
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_test_schema(&executor).expect("Failed to setup schema");
    cleanup_test_data(&executor).expect("Failed to cleanup");

    // Create a user model with an ID that doesn't exist in the database
    // We'll manually create a model instance
    let user = TestUser {
        id: 99999, // Non-existent ID
        name: "Ghost User".to_string(),
        email: "ghost@example.com".to_string(),
    };

    // Find related posts (should be empty)
    let posts = user.find_related::<TestPostEntity>()
        .all(&executor)
        .expect("Failed to query related posts");

    assert_eq!(posts.len(), 0, "Should find no posts for non-existent user");
}

#[test]
fn test_related_trait_compiles() {
    // Compile-time test: Verify that Related trait can be implemented
    // This test just ensures the trait is properly defined
    let _rel_def: RelationDef = TestPostEntity::to();
    let _rel_def: RelationDef = TestUserEntity::to();
}

// ============================================================================
// Composite Key Tests
// ============================================================================

#[derive(LifeModel, LifeRecord)]
#[table_name = "test_tenants_composite"]
pub struct TestTenant {
    #[primary_key]
    pub id: i32,
    #[primary_key]
    pub region_id: i32,
    pub name: String,
}

#[derive(LifeModel, LifeRecord)]
#[table_name = "test_resources_composite"]
pub struct TestResource {
    #[primary_key]
    #[auto_increment]
    pub id: i32,
    pub name: String,
    pub tenant_id: i32,
    pub region_id: i32,
}

// Resource belongs_to Tenant (composite key relationship)
impl Related<TestTenantEntity> for TestResourceEntity {
    fn to() -> RelationDef {
        RelationDef {
            rel_type: RelationType::BelongsTo,
            from_tbl: TableRef::Table(TableName(None, "test_resources_composite".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "test_tenants_composite".into_iden()), None),
            from_col: Identity::Binary("tenant_id".into(), "region_id".into()),
            to_col: Identity::Binary("id".into(), "region_id".into()),
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        }
    }
}

// Tenant has_many Resources (composite key relationship)
impl Related<TestResourceEntity> for TestTenantEntity {
    fn to() -> RelationDef {
        RelationDef {
            rel_type: RelationType::HasMany,
            from_tbl: TableRef::Table(TableName(None, "test_tenants_composite".into_iden()), None),
            to_tbl: TableRef::Table(TableName(None, "test_resources_composite".into_iden()), None),
            from_col: Identity::Binary("id".into(), "region_id".into()),
            to_col: Identity::Binary("tenant_id".into(), "region_id".into()),
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        }
    }
}

fn setup_composite_test_schema(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    // Create tenants table with composite primary key
    executor.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_tenants_composite (
            id INTEGER NOT NULL,
            region_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            PRIMARY KEY (id, region_id)
        )
        "#,
        &[],
    )?;

    // Create resources table with composite foreign key
    executor.execute(
        r#"
        CREATE TABLE IF NOT EXISTS test_resources_composite (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            tenant_id INTEGER NOT NULL,
            region_id INTEGER NOT NULL,
            FOREIGN KEY (tenant_id, region_id) REFERENCES test_tenants_composite(id, region_id)
        )
        "#,
        &[],
    )?;

    Ok(())
}

fn cleanup_composite_test_data(executor: &MayPostgresExecutor) -> Result<(), lifeguard::executor::LifeError> {
    executor.execute("DELETE FROM test_resources_composite", &[])?;
    executor.execute("DELETE FROM test_tenants_composite", &[])?;
    Ok(())
}

#[test]
fn test_composite_key_related_trait() {
    // Test that Related trait works with composite keys
    let rel_def: RelationDef = TestResourceEntity::to();
    assert_eq!(rel_def.from_col.arity(), 2);
    assert_eq!(rel_def.to_col.arity(), 2);
    
    let rel_def: RelationDef = TestTenantEntity::to();
    assert_eq!(rel_def.from_col.arity(), 2);
    assert_eq!(rel_def.to_col.arity(), 2);
}

#[test]
fn test_find_related_composite_key() {
    // Test find_related() with composite primary keys
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_composite_test_schema(&executor).expect("Failed to setup schema");
    cleanup_composite_test_data(&executor).expect("Failed to cleanup");

    // Create a tenant with composite key
    let mut tenant_record = TestTenantRecord::new();
    tenant_record.set_id(1);
    tenant_record.set_region_id(10);
    tenant_record.set_name("Test Tenant".to_string());
    let tenant = tenant_record.insert(&executor).expect("Failed to insert tenant");

    // Create resources for this tenant
    let mut resource1_record = TestResourceRecord::new();
    resource1_record.set_name("Resource 1".to_string());
    resource1_record.set_tenant_id(tenant.id);
    resource1_record.set_region_id(tenant.region_id);
    let _resource1 = resource1_record.insert(&executor).expect("Failed to insert resource1");

    let mut resource2_record = TestResourceRecord::new();
    resource2_record.set_name("Resource 2".to_string());
    resource2_record.set_tenant_id(tenant.id);
    resource2_record.set_region_id(tenant.region_id);
    let _resource2 = resource2_record.insert(&executor).expect("Failed to insert resource2");

    // Create a tenant in a different region
    let mut tenant2_record = TestTenantRecord::new();
    tenant2_record.set_id(1); // Same ID but different region
    tenant2_record.set_region_id(20);
    tenant2_record.set_name("Other Tenant".to_string());
    let tenant2 = tenant2_record.insert(&executor).expect("Failed to insert tenant2");

    // Create a resource for tenant2
    let mut resource3_record = TestResourceRecord::new();
    resource3_record.set_name("Other Resource".to_string());
    resource3_record.set_tenant_id(tenant2.id);
    resource3_record.set_region_id(tenant2.region_id);
    let _resource3 = resource3_record.insert(&executor).expect("Failed to insert resource3");

    // Find resources for the first tenant (should find 2 resources)
    let resources = tenant.find_related::<TestResourceEntity>()
        .all(&executor)
        .expect("Failed to query related resources");

    assert_eq!(resources.len(), 2, "Should find 2 resources for the tenant");
    
    // Verify the resources belong to the correct tenant
    for resource in &resources {
        assert_eq!(resource.tenant_id, tenant.id, "Resource should belong to the tenant");
        assert_eq!(resource.region_id, tenant.region_id, "Resource should be in the same region");
    }

    // Verify resource names
    let names: Vec<&str> = resources.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"Resource 1"), "Should contain resource 1");
    assert!(names.contains(&"Resource 2"), "Should contain resource 2");
    assert!(!names.contains(&"Other Resource"), "Should not contain other tenant's resource");
}

#[test]
fn test_find_related_composite_key_empty() {
    // Test find_related() with composite keys when no related entities exist
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_composite_test_schema(&executor).expect("Failed to setup schema");
    cleanup_composite_test_data(&executor).expect("Failed to cleanup");

    // Create a tenant with no resources
    let mut tenant_record = TestTenantRecord::new();
    tenant_record.set_id(1);
    tenant_record.set_region_id(10);
    tenant_record.set_name("Lonely Tenant".to_string());
    let tenant = tenant_record.insert(&executor).expect("Failed to insert tenant");

    // Find related resources (should be empty)
    let resources = tenant.find_related::<TestResourceEntity>()
        .all(&executor)
        .expect("Failed to query related resources");

    assert_eq!(resources.len(), 0, "Should find no resources for tenant with no resources");
}

#[test]
fn test_composite_key_identity_values_match() {
    // Edge case: Verify that composite key Identity and values match
    let mut test_db = TestDatabase::new().expect("Failed to create test database");
    let _client = test_db.connect().expect("Failed to connect to database");
    
    let executor = test_db.executor().expect("Failed to create executor");
    setup_composite_test_schema(&executor).expect("Failed to setup schema");
    cleanup_composite_test_data(&executor).expect("Failed to cleanup");

    // Create a tenant
    let mut tenant_record = TestTenantRecord::new();
    tenant_record.set_id(42);
    tenant_record.set_region_id(100);
    tenant_record.set_name("Test Tenant".to_string());
    let tenant = tenant_record.insert(&executor).expect("Failed to insert tenant");

    // Verify get_primary_key_identity() returns Binary Identity
    let identity = tenant.get_primary_key_identity();
    assert_eq!(identity.arity(), 2, "Composite key should have arity 2");

    // Verify get_primary_key_values() returns 2 values
    let values = tenant.get_primary_key_values();
    assert_eq!(values.len(), 2, "Composite key should have 2 values");
    assert_eq!(values.len(), identity.arity(), "Values count should match Identity arity");
}
