// Test that DeriveLinked macro produces compile error for invalid entity path

use lifeguard_derive::DeriveLinked;

#[derive(DeriveLinked)]
pub enum LinkedRelation {
    // Error: Invalid entity path (contains invalid characters)
    #[lifeguard(linked = "Post-Entity -> CommentEntity")]
    Comments,
}
