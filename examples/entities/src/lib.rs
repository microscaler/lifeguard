//! Example Entities Library
//!
//! This library provides example Lifeguard entity definitions for demonstration purposes.
//! 
//! **Note**: RERP accounting entities have been moved to `rerp/entities/`.
//! This examples directory is kept for Lifeguard documentation and testing purposes.
//!
//! ## Usage
//!
//! ```rust
//! use lifeguard::LifeModelTrait;
//! use example_entities::inventory::Product;
//!
//! // Access entity metadata
//! let entity = Product::Entity::default();
//! println!("Table: {}", entity.table_name());
//! ```
//!
//! ## Entity Organization
//!
//! Entities are organized by service domain:
//! - `inventory` - Inventory management entities (products, categories, inventory items)

pub mod inventory;

// Include generated entity registry from build script
#[allow(missing_docs)]
pub mod entity_registry {
    include!(concat!(env!("OUT_DIR"), "/entity_registry.rs"));
}

// Re-export for convenience
pub use inventory::*;
