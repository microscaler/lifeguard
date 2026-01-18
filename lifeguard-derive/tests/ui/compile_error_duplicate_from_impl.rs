//! Test that verifies duplicate Related and From impls are prevented
//!
//! This test demonstrates a scenario where multiple relations target the same entity.
//! Without the fix, this would generate:
//! - Duplicate `impl Related<PostEntity> for Entity` trait implementations
//! - Duplicate `From<PostModel> for RelatedEntity` impls
//!
//! Both of these would cause compile errors:
//! - "conflicting implementations of trait `Related`"
//! - "conflicting implementations of trait `From`"
//!
//! With the fix, only one Related impl and one From impl are generated per unique target entity path,
//! so this should compile successfully.
//!
//! This test verifies that the macro correctly deduplicates both Related and From impls
//! when multiple relations target the same entity (e.g., CreatedPosts and EditedPosts both pointing to PostEntity).

use lifeguard_derive::DeriveRelation;

// Mock entities for testing
#[derive(Default, Copy, Clone)]
pub struct Entity;

impl sea_query::Iden for Entity {
    fn unquoted(&self) -> &str { "users" }
}

impl lifeguard::LifeEntityName for Entity {
    fn table_name(&self) -> &'static str { "users" }
}

impl lifeguard::LifeModelTrait for Entity {
    type Model = Model;
    type Column = Column;
}

#[derive(Debug, Clone)]
pub struct Model;

#[derive(Copy, Clone, Debug)]
pub enum Column {
    Id,
}

impl sea_query::Iden for Column {
    fn unquoted(&self) -> &str { "id" }
}

impl sea_query::IdenStatic for Column {
    fn as_str(&self) -> &'static str { "id" }
}

#[derive(Default, Copy, Clone)]
pub struct PostEntity;

impl sea_query::Iden for PostEntity {
    fn unquoted(&self) -> &str { "posts" }
}

impl lifeguard::LifeEntityName for PostEntity {
    fn table_name(&self) -> &'static str { "posts" }
}

impl lifeguard::LifeModelTrait for PostEntity {
    type Model = PostModel;
    type Column = PostColumn;
}

#[derive(Debug, Clone)]
pub struct PostModel;

#[derive(Copy, Clone, Debug)]
pub enum PostColumn {
    Id,
    UserId,
}

impl sea_query::Iden for PostColumn {
    fn unquoted(&self) -> &str {
        match self {
            PostColumn::Id => "id",
            PostColumn::UserId => "user_id",
        }
    }
}

impl sea_query::IdenStatic for PostColumn {
    fn as_str(&self) -> &'static str {
        match self {
            PostColumn::Id => "id",
            PostColumn::UserId => "user_id",
        }
    }
}

// This Relation enum has two variants targeting the same entity (PostEntity)
// Without the fix, this would generate:
// - Two `impl Related<PostEntity> for Entity` impls (causing "conflicting implementations" error)
// - Two `From<PostModel> for RelatedEntity` impls (causing "conflicting implementations" error)
//
// With the fix, only one Related impl and one From impl are generated per unique target entity path,
// so this compiles successfully.
#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(has_many = "PostEntity")]
    CreatedPosts,
    #[lifeguard(has_many = "PostEntity")]
    EditedPosts,
}

// If the fix works, this should compile without "conflicting implementations" errors
// The RelatedEntity enum should have both variants, but only one Related impl and one From impl
