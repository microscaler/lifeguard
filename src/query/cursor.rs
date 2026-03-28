//! Cursor Pagination functionality.
//!
//! Provides cursor-based keyset pagination `CursorPaginator` abstraction
//! bound natively to the `SelectQuery` engine.

use sea_query::{Condition, Expr, ExprTrait, IntoColumnRef, Order};
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
///
/// # Non-unique sort keys
///
/// If the cursor column can repeat across rows, use [`Self::after_pk`] / [`Self::before_pk`]
/// together with [`Self::after`] / [`Self::before`] so ordering matches
/// `(cursor column, primary key)` and pages do not skip or duplicate rows. Entities with a
/// single-column primary key expose this via [`LifeModelTrait::cursor_tiebreak_column`].
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
    after_pk_val: Option<sea_query::Value>,
    before_pk_val: Option<sea_query::Value>,
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
            after_pk_val: None,
            before_pk_val: None,
            limit_val: None,
            _phantom: PhantomData,
        }
    }

    /// Retrieve records explicitly sequenced *after* the provided value
    pub fn after<V: Into<sea_query::Value>>(mut self, value: V) -> Self {
        self.after_val = Some(value.into());
        self
    }

    /// Primary key value for the last row of the previous page (must be used with [`Self::after`]
    /// when the cursor column is not unique). Ignored if [`LifeModelTrait::cursor_tiebreak_column`]
    /// is `None`.
    pub fn after_pk<V: Into<sea_query::Value>>(mut self, pk: V) -> Self {
        self.after_pk_val = Some(pk.into());
        self
    }

    /// Retrieve records explicitly sequenced *before* the provided value
    pub fn before<V: Into<sea_query::Value>>(mut self, value: V) -> Self {
        self.before_val = Some(value.into());
        self
    }

    /// Primary key value for the first row of the “next” page when paging backward with [`Self::before`].
    pub fn before_pk<V: Into<sea_query::Value>>(mut self, pk: V) -> Self {
        self.before_pk_val = Some(pk.into());
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
        E::Column: IntoColumnRef + Clone + Copy,
    {
        let order = self.order.clone();
        let tie = E::cursor_tiebreak_column();

        let compound_after = tie.is_some()
            && self.after_val.is_some()
            && self.after_pk_val.is_some()
            && self.before_val.is_none()
            && self.before_pk_val.is_none();
        let compound_before = tie.is_some()
            && self.before_val.is_some()
            && self.before_pk_val.is_some()
            && self.after_val.is_none()
            && self.after_pk_val.is_none();

        if let Some(pk_col) = tie.filter(|_| compound_after || compound_before) {
            let c = Expr::col(self.column.clone());
            let pk = Expr::col(pk_col);

            if compound_after {
                let cv = self.after_val.take().expect("after_val set with compound_after");
                let pkv = self.after_pk_val.take().expect("after_pk_val set with compound_after");
                // Asc: strictly after (cv, pkv) in (col asc, pk asc)
                let cond = Condition::any()
                    .add(c.clone().gt(cv.clone()))
                    .add(
                        Condition::all()
                            .add(c.clone().eq(cv))
                            .add(pk.clone().gt(pkv)),
                    );
                self.query = self.query.filter(cond);
            } else {
                let cv = self.before_val.take().expect("before_val set with compound_before");
                let pkv = self.before_pk_val.take().expect("before_pk_val set with compound_before");
                // Desc on (col, pk): strictly “before” (cv, pkv) in that order
                let cond = Condition::any()
                    .add(c.clone().lt(cv.clone()))
                    .add(
                        Condition::all()
                            .add(c.clone().eq(cv))
                            .add(pk.clone().lt(pkv)),
                    );
                self.query = self.query.filter(cond);
            }

            self.query = self.query.order_by(self.column.clone(), order.clone());
            self.query = self.query.order_by(pk_col, order);
        } else {
            // Drop orphan PK bounds
            let _ = self.after_pk_val.take();
            let _ = self.before_pk_val.take();

            if let Some(val) = self.after_val.take() {
                self.query = self.query.filter(Expr::col(self.column.clone()).gt(val));
            }

            if let Some(val) = self.before_val.take() {
                self.query = self.query.filter(Expr::col(self.column.clone()).lt(val));
            }

            self.query = self.query.order_by(self.column.clone(), order);
        }

        if let Some(limit) = self.limit_val.take() {
            self.query = self.query.limit(limit);
        }

        self.query.all(executor)
    }
}
