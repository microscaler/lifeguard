//! Inventory service entities
//!
//! Example entities for demonstrating Lifeguard migration generation.

pub mod category;
pub mod product;
pub mod inventory_item;

// Re-export entities for convenience
pub use category::Category;
pub use product::Product;
pub use inventory_item::InventoryItem;
