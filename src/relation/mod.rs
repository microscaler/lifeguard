//! Relation module for entity relationships.
//!
//! This module provides support for defining and querying entity relationships:
//! - `belongs_to`: Many-to-one relationship
//! - `has_one`: One-to-one relationship
//! - `has_many`: One-to-many relationship
//! - `has_many_through`: Many-to-many relationship (via join table)
//! - `Linked`: Multi-hop relationship queries (e.g., `User → Posts → Comments`)
//! - Eager loading: Load related entities automatically (`selectinload` strategy)
//! - Lazy loading: Load related entities on-demand (deferred queries)
//!
//! # Architecture
//!
//! The relation module follows `Sea-ORM`'s organizational patterns:
//! - **Traits**: Core relation traits (`RelationTrait`, `Related`, `FindRelated`, `Linked`)
//! - **Def**: Relation definition types (`RelationDef`, `RelationType`)
//! - **Identity**: Identity types for single and composite keys
//! - **Helpers**: Helper functions for join conditions and eager loading

// Identity types
pub mod identity;
#[doc(inline)]
pub use identity::{Identity, BorrowedIdentityIter, IntoIdentity};

// Relation definitions
pub mod def;
#[doc(inline)]
pub use def::{RelationDef, RelationType, join_tbl_on_condition, build_where_condition};

// Core traits
pub mod traits;
#[doc(inline)]
pub use traits::{RelationTrait, RelationBuilder, RelationMetadata, Related, FindRelated, Linked, FindLinked};

// Helper functions
pub mod helpers;
#[doc(inline)]
pub use helpers::join_condition;

// Eager loading
pub mod eager;
#[doc(inline)]
pub use eager::load_related;

// Lazy loading
pub mod lazy;
#[doc(inline)]
pub use lazy::LazyLoader;
