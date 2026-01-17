//! Tests for DeriveRelation macro
//!
//! These tests verify that the DeriveRelation macro correctly generates
//! Related trait implementations from Relation enum definitions.

use lifeguard_derive::DeriveRelation;
use lifeguard::{Related, SelectQuery, LifeModelTrait, LifeEntityName, RelationMetadata};

// Test entities
#[derive(Default, Copy, Clone)]
pub struct UserEntity;

impl sea_query::Iden for UserEntity {
    fn unquoted(&self) -> &str {
        "users"
    }
}

impl LifeEntityName for UserEntity {
    fn table_name(&self) -> &'static str {
        "users"
    }
}

impl LifeModelTrait for UserEntity {
    type Model = UserModel;
    type Column = UserColumn;
}

#[derive(Default, Copy, Clone)]
pub struct PostEntity;

impl sea_query::Iden for PostEntity {
    fn unquoted(&self) -> &str {
        "posts"
    }
}

impl LifeEntityName for PostEntity {
    fn table_name(&self) -> &'static str {
        "posts"
    }
}

impl LifeModelTrait for PostEntity {
    type Model = PostModel;
    type Column = PostColumn;
}

#[derive(Default, Copy, Clone)]
pub struct CommentEntity;

impl sea_query::Iden for CommentEntity {
    fn unquoted(&self) -> &str {
        "comments"
    }
}

impl LifeEntityName for CommentEntity {
    fn table_name(&self) -> &'static str {
        "comments"
    }
}

impl LifeModelTrait for CommentEntity {
    type Model = CommentModel;
    type Column = CommentColumn;
}

// Test models and columns (simplified)
pub struct UserModel;
pub struct PostModel;
pub struct CommentModel;

#[derive(Copy, Clone, Debug)]
pub enum UserColumn {
    Id,
}

#[derive(Copy, Clone, Debug)]
pub enum PostColumn {
    Id,
    UserId,
}

#[derive(Copy, Clone, Debug)]
pub enum CommentColumn {
    Id,
    PostId,
}

// Test Entity (assumed to be PostEntity for this test)
#[derive(Default, Copy, Clone)]
pub struct Entity;

impl sea_query::Iden for Entity {
    fn unquoted(&self) -> &str {
        "posts"
    }
}

impl LifeEntityName for Entity {
    fn table_name(&self) -> &'static str {
        "posts"
    }
}

impl LifeModelTrait for Entity {
    type Model = PostModel;
    type Column = PostColumn;
}

// Test Relation enum with DeriveRelation
#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(has_many = "CommentEntity")]
    Comments,
    #[lifeguard(
        belongs_to = "UserEntity",
        from = "Column::UserId",
        to = "UserEntity::Column::Id"
    )]
    User,
}

#[test]
fn test_derive_relation_generates_related_impls() {
    // Test that Related trait implementations are generated
    // This is a compile-time test - if it compiles, the macro worked
    
    // Test has_many relationship
    let _query: SelectQuery<CommentEntity> = CommentEntity::to();
    
    // Test belongs_to relationship
    let _query: SelectQuery<UserEntity> = UserEntity::to();
}

#[test]
fn test_derive_relation_with_metadata() {
    // Test that RelationMetadata is generated when from/to columns are provided
    // The belongs_to relationship should have RelationMetadata implementation
    
    // Check if RelationMetadata is implemented (compile-time check)
    let fk_col = <CommentEntity as RelationMetadata<Entity>>::foreign_key_column();
    // Should return None for has_many (no from/to specified)
    assert_eq!(fk_col, None);
    
    // For belongs_to with from/to, RelationMetadata should be implemented
    // But we can't easily test this without the actual generated code
    // This test mainly verifies compilation
}

#[test]
fn test_derive_relation_multiple_relationships() {
    // Test that multiple relationships in the same enum work
    // This is verified by the enum having multiple variants
    let _ = Relation::Comments;
    let _ = Relation::User;
}
