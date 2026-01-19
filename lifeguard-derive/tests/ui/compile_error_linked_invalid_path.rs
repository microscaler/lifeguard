// Test that DeriveLinked macro produces compile error for invalid path syntax

use lifeguard_derive::DeriveLinked;

#[derive(DeriveLinked)]
pub enum LinkedRelation {
    // Error: Only one hop (needs at least 2)
    #[lifeguard(linked = "PostEntity")]
    Comments,
}
