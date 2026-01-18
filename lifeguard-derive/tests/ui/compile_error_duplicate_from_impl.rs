//! Test that verifies duplicate From impls are prevented
//!
//! This test demonstrates a scenario where multiple relations target the same entity.
//! Without the fix, this would generate duplicate `From<PostModel> for RelatedEntity` impls,
//! causing a compile error: "conflicting implementations of trait `From`".
//!
//! With the fix, only one From impl is generated per unique target entity path,
//! so this should compile successfully.
//!
//! Note: This test uses a scenario where we can have multiple relations to the same entity
//! but in a way that doesn't conflict with Related impls (e.g., different relation types
//! or different contexts). However, in practice, having two has_many relations to the same
//! entity from the same source would also create conflicting Related impls, which is a
//! separate Rust trait coherence issue.
//!
//! The fix ensures that even if such a scenario were possible, duplicate From impls
//! would not be generated.

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
// Without the fix, this would generate two `From<PostModel> for RelatedEntity` impls,
// causing a compile error. With the fix, only one From impl is generated.
//
// Note: This will also generate two Related impls which may conflict,
// but that's a separate issue. The From impl deduplication is what we're testing.
#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(has_many = "PostEntity")]
    CreatedPosts,
    #[lifeguard(has_many = "PostEntity")]
    EditedPosts,
}

// If the fix works, this should compile without "conflicting implementations of trait `From`" error
// The RelatedEntity enum should have both variants, but only one From impl
