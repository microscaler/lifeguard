//! Model trait for accessing and manipulating model data
//!
//! This module provides the `ModelTrait` which allows dynamic access to model fields
//! and primary key values. Similar to SeaORM's `ModelTrait`.

use crate::query::LifeModelTrait;
use sea_query::Value;

/// Trait for Model-level operations
///
/// This trait provides methods for accessing and manipulating model data at runtime.
/// It's similar to SeaORM's `ModelTrait` and allows dynamic column access.
///
/// # Example
///
/// ```no_run
/// use lifeguard::{ModelTrait, LifeModelTrait};
///
/// # struct Entity; // Entity
/// # impl lifeguard::LifeModelTrait for Entity {
/// #     type Model = EntityModel;
/// # }
/// # struct EntityModel { id: i32, name: String };
/// # impl lifeguard::ModelTrait for EntityModel {
/// #     type Entity = Entity;
/// #     fn get(&self, _col: Entity::Column) -> sea_query::Value { todo!() }
/// #     fn get_primary_key_value(&self) -> sea_query::Value { todo!() }
/// # }
/// let model = EntityModel { id: 1, name: "John".to_string() };
/// let id_value = model.get(Entity::Column::Id);
/// let pk_value = model.get_primary_key_value();
/// ```
pub trait ModelTrait: Clone + Send + std::fmt::Debug {
    /// The Entity type that this Model belongs to
    type Entity: LifeModelTrait;

    /// Get the value of a column from the model
    ///
    /// # Arguments
    ///
    /// * `column` - The column to get the value for
    ///
    /// # Returns
    ///
    /// The column value as a `sea_query::Value`
    fn get(&self, column: <Self::Entity as LifeModelTrait>::Column) -> Value;

    /// Get the primary key value(s) from the model
    ///
    /// For single-column primary keys, returns the value directly.
    /// For composite primary keys, this would return a tuple (future enhancement).
    ///
    /// # Returns
    ///
    /// The primary key value as a `sea_query::Value`
    fn get_primary_key_value(&self) -> Value;
}
