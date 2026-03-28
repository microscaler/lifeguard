//! Cursor Pagination functionality.
//!
//! Provides cursor-based keyset pagination `CursorPaginator` abstraction 
//! bound natively to the `SelectQuery` engine.

use sea_query::{Expr, ExprTrait, IntoColumnRef, Order};
use std::marker::PhantomData;

use crate::{LifeExecutor, LifeError};
use crate::query::traits::LifeModelTrait;
use crate::query::traits::FromRow;
use crate::query::SelectQuery;

/// A cursor-based paginator utilizing deterministic indexes for offsets.
///
/// Unlike standard `OFFSET` pagination (which degrades as datasets expand),
/// cursor pagination achieves `O(1)` query efficiency by maintaining static comparisons 
/// against sort indexes (e.g. `WHERE id > last_seen_id ORDER BY id ASC LIMIT X`).
pub struct CursorPaginator<E, C>
where
    E: LifeModelTrait,
    C: IntoColumnRef + Clone,
{
    query: SelectQuery<E>,
    column: C,
    order: Order,
    after_val: Option<sea_query::Value>,
    before_val: Option<sea_query::Value>,
    limit_val: Option<u64>,
    _phantom: PhantomData<E>,
}

impl<E, C> CursorPaginator<E, C>
where
    E: LifeModelTrait,
    C: IntoColumnRef + Clone,
{
    /// Create a new Cursor Paginator mapping to a `SelectQuery` engine.
    pub(crate) fn new(query: SelectQuery<E>, column: C) -> Self {
        Self {
            query,
            column,
            order: Order::Asc, // Natural progression defaults to ASC
            after_val: None,
            before_val: None,
            limit_val: None,
            _phantom: PhantomData,
        }
    }

    /// Retrieve records explicitly sequenced *after* the provided value
    pub fn after<V: Into<sea_query::Value>>(mut self, value: V) -> Self {
        self.after_val = Some(value.into());
        self
    }

    /// Retrieve records explicitly sequenced *before* the provided value
    pub fn before<V: Into<sea_query::Value>>(mut self, value: V) -> Self {
        self.before_val = Some(value.into());
        self
    }

    /// Limit the cursor slice to the first N results traversing globally forward natively via `Order::Asc`
    pub fn first(mut self, limit: u64) -> Self {
        self.limit_val = Some(limit);
        self.order = Order::Asc;
        self
    }

    /// Limit the cursor slice to the last N results traversing globally backwards natively via `Order::Desc`
    pub fn last(mut self, limit: u64) -> Self {
        self.limit_val = Some(limit);
        self.order = Order::Desc;
        self
    }

    /// Execute the paginated cursor sequence extracting results structurally.
    pub fn fetch<Ex: LifeExecutor>(mut self, executor: &Ex) -> Result<Vec<E::Model>, LifeError>
    where
        E::Model: FromRow,
    {
        // 1. Evaluate Forward/Backward Bound Filters
        if let Some(val) = self.after_val.take() {
            self.query = self.query.filter(Expr::col(self.column.clone()).gt(val));
        }

        if let Some(val) = self.before_val.take() {
            self.query = self.query.filter(Expr::col(self.column.clone()).lt(val));
        }
        
        // 2. Set Sort Boundaries Native to Keys
        self.query = self.query.order_by(self.column.clone(), self.order.clone());

        // 3. Throttle Fetch Size
        if let Some(limit) = self.limit_val.take() {
            self.query = self.query.limit(limit);
        }

        // 4. Resolve via Base Pipeline!
        self.query.all(executor)
    }
}
