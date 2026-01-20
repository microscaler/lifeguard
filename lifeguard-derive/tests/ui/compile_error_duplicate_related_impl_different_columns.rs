//! Test that multiple relations targeting the same entity with different column configurations
//! cause a compile error
//!
//! This test verifies that when multiple relations target the same entity (e.g., PostEntity)
//! with different `from`/`to` column configurations, the macro correctly reports a compile error
//! instead of silently discarding the second configuration.
//!
//! Example scenario:
//! - CreatedPosts with from = "Column::CreatorId" 
//! - EditedPosts with from = "Column::EditorId"
//! Both target PostEntity, but use different foreign key columns.
//!
//! This should cause a compile error because Rust doesn't allow multiple `impl Related<PostEntity> for Entity`
//! implementations, and silently using only the first configuration would lead to incorrect queries.

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
    CreatorId,
    EditorId,
}

impl sea_query::Iden for Column {
    fn unquoted(&self) -> &str {
        match self {
            Column::Id => "id",
            Column::CreatorId => "creator_id",
            Column::EditorId => "editor_id",
        }
    }
}

impl sea_query::IdenStatic for Column {
    fn as_str(&self) -> &'static str {
        match self {
            Column::Id => "id",
            Column::CreatorId => "creator_id",
            Column::EditorId => "editor_id",
        }
    }
}

lifeguard::impl_column_def_helper_for_test!(Column);

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
    CreatorId,
    EditorId,
}

impl sea_query::Iden for PostColumn {
    fn unquoted(&self) -> &str {
        match self {
            PostColumn::Id => "id",
            PostColumn::CreatorId => "creator_id",
            PostColumn::EditorId => "editor_id",
        }
    }
}

impl sea_query::IdenStatic for PostColumn {
    fn as_str(&self) -> &'static str {
        match self {
            PostColumn::Id => "id",
            PostColumn::CreatorId => "creator_id",
            PostColumn::EditorId => "editor_id",
        }
    }
}

lifeguard::impl_column_def_helper_for_test!(PostColumn);

// This Relation enum has two variants targeting the same entity (PostEntity)
// with DIFFERENT column configurations:
// - CreatedPosts uses to = "PostColumn::CreatorId" (different FK column)
// - EditedPosts uses to = "PostColumn::EditorId" (different FK column)
//
// This should cause a compile error because:
// 1. Rust doesn't allow multiple `impl Related<PostEntity> for Entity` implementations
// 2. The macro would silently discard the second configuration, leading to incorrect queries
#[derive(DeriveRelation)]
pub enum Relation {
    #[lifeguard(
        has_many = "PostEntity",
        from = "Column::Id",
        to = "PostColumn::CreatorId"
    )]
    CreatedPosts,
    #[lifeguard(
        has_many = "PostEntity",
        from = "Column::Id",
        to = "PostColumn::EditorId"
    )]
    EditedPosts,
}
