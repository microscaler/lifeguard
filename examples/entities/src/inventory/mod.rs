//! Inventory service entities
//!
//! Example entities for demonstrating Lifeguard migration generation.

pub mod category;
pub mod inventory_item;
pub mod product;

// Re-export entities for convenience
pub use category::Category;
pub use inventory_item::InventoryItem;
pub use product::Product;
