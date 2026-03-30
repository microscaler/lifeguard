//! Mechanism for defining and resolving nested `ActiveModel` graph hierarchies.

use crate::{ActiveModelError, LifeExecutor};

/// Represents an edge in a directed graph of `ActiveModels` pending insertion or update.
/// Contains the closure responsible for executing the persistence action topologically.
pub enum GraphEdge<R> {
    /// A "Belongs To" parent record that must be saved **FIRST**, before the root record.
    /// The closure is given mutable access to the root record `R` so it can assign its generated PK to the root's FK column.
    BelongsTo(Box<dyn FnOnce(&mut R, &dyn LifeExecutor) -> Result<(), ActiveModelError> + Send>),

    /// A "Has Many" or "Has One" child record that must be saved **LAST**, after the root record.
    /// The closure is given immutable access to the root record `R` so it can read its generated PK.
    HasMany(Box<dyn FnOnce(&R, &dyn LifeExecutor) -> Result<(), ActiveModelError> + Send>),
}

/// A state container injected into every `LifeRecord` (via the `Lifeguard-Derive` macro)
/// used to hold deferred parent (`BelongsTo`) and child (`HasMany`) edge records.
pub struct GraphState<R> {
    /// List of un-executed edges.
    pub edges: Vec<GraphEdge<R>>,
}

impl<R> Default for GraphState<R> {
    fn default() -> Self {
        Self { edges: Vec::new() }
    }
}

impl<R> GraphState<R> {
    /// Create a new, empty relation graph queue.
    #[must_use]
    pub fn new() -> Self {
        Self { edges: Vec::new() }
    }

    /// Queue a parent record to be saved *before* the current root record.
    pub fn add_belongs_to(
        &mut self,
        action: Box<dyn FnOnce(&mut R, &dyn LifeExecutor) -> Result<(), ActiveModelError> + Send>,
    ) {
        self.edges.push(GraphEdge::BelongsTo(action));
    }

    /// Queue a child record to be saved *after* the current root record.
    pub fn add_has_many(
        &mut self,
        action: Box<dyn FnOnce(&R, &dyn LifeExecutor) -> Result<(), ActiveModelError> + Send>,
    ) {
        self.edges.push(GraphEdge::HasMany(action));
    }
}

/// A container around `GraphState` that manually implements `Clone` and `Debug`
/// by safely ignoring the internal state (which contains un-clonable closures).
/// This prevents compilation errors when `#[derive(Clone)]` is applied to `LifeRecord` structs.
pub struct GraphContainer<R>(pub Option<Box<GraphState<R>>>);

impl<R> Default for GraphContainer<R> {
    fn default() -> Self {
        Self(None)
    }
}

impl<R> Clone for GraphContainer<R> {
    fn clone(&self) -> Self {
        // We drop the graph on clone because graph closures cannot be cloned.
        // During `.save()`, `__graph` state does not propagate to intermediate hook changes.
        GraphContainer(None)
    }
}

impl<R> std::fmt::Debug for GraphContainer<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "GraphContainer(..)")
    }
}
