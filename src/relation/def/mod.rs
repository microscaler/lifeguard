//! Relation definition module for storing relationship metadata.
//!
//! This module provides types and utilities for defining relationships between entities,
//! including the `RelationDef` struct, `RelationType` enum, and condition building functions.

pub mod types;
pub mod struct_def;
pub mod condition;

// Re-export public types
#[doc(inline)]
pub use types::RelationType;
#[doc(inline)]
pub use struct_def::RelationDef;
#[doc(inline)]
pub use condition::{join_tbl_on_condition, join_tbl_on_expr, build_where_condition};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation::identity::Identity;
    use sea_query::{TableName, IntoIden, ConditionType};

    #[test]
    fn test_relation_def_rev() {
        let rel_def = RelationDef {
            rel_type: RelationType::BelongsTo,
            from_tbl: sea_query::TableRef::Table(TableName(None, "posts".into_iden()), None),
            to_tbl: sea_query::TableRef::Table(TableName(None, "users".into_iden()), None),
            from_col: Identity::Unary("user_id".into()),
            to_col: Identity::Unary("id".into()),
            through_tbl: None,
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        };

        let reversed = rel_def.clone().rev();
        // Can't easily compare TableRef, so just verify the method doesn't panic
        assert_eq!(reversed.from_col, rel_def.to_col);
        assert_eq!(reversed.to_col, rel_def.from_col);
        assert_eq!(reversed.is_owner, !rel_def.is_owner);
    }

    #[test]
    fn test_relation_def_rev_composite() {
        // Edge case: Reversing composite key relationship
        let rel_def = RelationDef {
            rel_type: RelationType::BelongsTo,
            from_tbl: sea_query::TableRef::Table(TableName(None, "posts".into_iden()), None),
            to_tbl: sea_query::TableRef::Table(TableName(None, "users".into_iden()), None),
            from_col: Identity::Binary("user_id".into(), "tenant_id".into()),
            to_col: Identity::Binary("id".into(), "tenant_id".into()),
            through_tbl: None,
            is_owner: true,
            skip_fk: false,
            on_condition: None,
            condition_type: ConditionType::All,
        };

        let reversed = rel_def.clone().rev();
        assert_eq!(reversed.from_col, rel_def.to_col);
        assert_eq!(reversed.to_col, rel_def.from_col);
        assert_eq!(reversed.is_owner, !rel_def.is_owner);
    }
}
