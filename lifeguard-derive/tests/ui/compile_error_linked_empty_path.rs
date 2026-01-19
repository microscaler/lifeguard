// Test that DeriveLinked macro produces compile error for empty path

use lifeguard_derive::DeriveLinked;

#[derive(DeriveLinked)]
pub enum LinkedRelation {
    // Error: Empty path
    #[lifeguard(linked = "")]
    Comments,
}
