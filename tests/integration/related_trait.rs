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
};
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
    fn to() -> SelectQuery<TestUserEntity> {
        // For belongs_to, we'd typically join User table
        // But for now, we just return a base query
        // The find_related() method will add the WHERE clause
        SelectQuery::new()
    }
}

// User has_many Posts (one-to-many)
// This means: User.id is referenced by Post.user_id
impl Related<TestPostEntity> for TestUserEntity {
    fn to() -> SelectQuery<TestPostEntity> {
        // For has_many, we return a query for Posts
        // The find_related() method will filter by user_id
        SelectQuery::new()
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
    // Test that Related::to() returns a SelectQuery
    let query: SelectQuery<TestPostEntity> = TestPostEntity::to();
    // Just verify it compiles and returns a query
    let _ = query;
    
    let query: SelectQuery<TestUserEntity> = TestUserEntity::to();
    let _ = query;
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
    let _query: SelectQuery<TestPostEntity> = TestPostEntity::to();
    let _query: SelectQuery<TestUserEntity> = TestUserEntity::to();
}
