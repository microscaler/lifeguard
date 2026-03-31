//! Explicit [`Session`] handle for PRD Phase E (U-3, U-4).
//!
//! [`Session`] wraps a shared [`ModelIdentityMap`](super::ModelIdentityMap) and a **sendable**
//! [`SessionDirtyNotifier`] so derived `LifeRecord` can call `attach_session(&session)` without
//! breaking `Send` (required by [`ActiveModel`](crate::active_model) graph closures).
//!
//! Pending dirty keys are merged into the identity map at [`Session::flush_dirty`] time.
//!
//! # Pooling (U-4)
//!
//! Flush with any [`LifeExecutor`](crate::executor::LifeExecutor), including [`PooledLifeExecutor`](crate::pool::PooledLifeExecutor).
//! For a single transaction across rows, wrap the executor in [`crate::Transaction`]. See
//! `docs/planning/DESIGN_SESSION_UOW.md`.

use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::active_model::ActiveModelError;
use crate::executor::LifeExecutor;
use crate::model::ModelTrait;
use crate::query::LifeModelTrait;

use super::ModelIdentityMap;

/// Notifies a [`Session`] that a primary-key fingerprint should be treated as dirty.
///
/// Cloning shares the same backing queue. Safe to store on a derived `LifeRecord` (`Send` + `Sync`).
#[derive(Clone)]
pub struct SessionDirtyNotifier {
    pending: Arc<Mutex<HashSet<String>>>,
}

impl fmt::Debug for SessionDirtyNotifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionDirtyNotifier").finish_non_exhaustive()
    }
}

impl SessionDirtyNotifier {
    pub(crate) fn new(pending: Arc<Mutex<HashSet<String>>>) -> Self {
        Self { pending }
    }

    /// Queue `key` for the next [`Session::flush_dirty`]. No-op if `key` is `None`.
    pub fn notify_identity_map_dirty(&self, key: Option<String>) {
        let Some(k) = key else {
            return;
        };
        let mut g = match self.pending.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        g.insert(k);
    }
}

/// Unit-of-work boundary: shared identity map and dirty tracking (explicit; no thread-local — U-3).
///
/// [`Clone`]: all clones share the same map and pending-dirty queue. The inner map is **not**
/// [`Send`]/[`Sync`] (same as [`ModelIdentityMap`](super::ModelIdentityMap)); use one session per
/// thread / coroutine, or keep records on the same thread as the session.
#[derive(Clone)]
pub struct Session<E>
where
    E: LifeModelTrait,
    E::Model: ModelTrait<Entity = E> + Clone,
{
    inner: Rc<RefCell<ModelIdentityMap<E>>>,
    pending_dirty: Arc<Mutex<HashSet<String>>>,
}

impl<E> Default for Session<E>
where
    E: LifeModelTrait,
    E::Model: ModelTrait<Entity = E> + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<E> fmt::Debug for Session<E>
where
    E: LifeModelTrait,
    E::Model: ModelTrait<Entity = E> + Clone,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner.try_borrow() {
            Ok(m) => f
                .debug_struct("Session")
                .field("len", &m.len())
                .field("dirty_len", &m.dirty_len())
                .finish(),
            Err(_) => f.write_str("Session(..)"),
        }
    }
}

impl<E> Session<E>
where
    E: LifeModelTrait,
    E::Model: ModelTrait<Entity = E> + Clone,
{
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(ModelIdentityMap::new())),
            pending_dirty: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// [`SessionDirtyNotifier`] for `LifeRecord::attach_session`.
    #[must_use]
    pub fn dirty_notifier(&self) -> SessionDirtyNotifier {
        SessionDirtyNotifier::new(Arc::clone(&self.pending_dirty))
    }

    fn drain_pending_into_map(&self) {
        let keys: Vec<String> = {
            let mut p = match self.pending_dirty.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            p.drain().collect()
        };
        let mut map = self.inner.borrow_mut();
        for k in keys {
            map.mark_dirty_key(&k);
        }
    }

    pub fn register_loaded(&self, model: E::Model) -> Rc<RefCell<E::Model>> {
        self.inner.borrow_mut().register_loaded(model)
    }

    #[must_use]
    pub fn get_existing(&self, model: &E::Model) -> Option<Rc<RefCell<E::Model>>> {
        self.inner.borrow().get_existing(model)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.borrow().len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.borrow().is_empty()
    }

    #[must_use]
    pub fn dirty_len(&self) -> usize {
        self.inner.borrow().dirty_len()
    }

    /// Marks dirty on the underlying map (immediate). Prefer [`SessionDirtyNotifier`] when mutating an attached `LifeRecord`.
    pub fn mark_dirty(&self, model: &E::Model) {
        self.inner.borrow_mut().mark_dirty(model);
    }

    pub fn mark_dirty_key(&self, key: &str) {
        self.inner.borrow_mut().mark_dirty_key(key);
    }

    pub fn clear_dirty(&self) {
        let mut p = match self.pending_dirty.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        p.clear();
        self.inner.borrow_mut().clear_dirty();
    }

    /// Merges any [`SessionDirtyNotifier`] keys into the map, then flushes (same semantics as [`ModelIdentityMap::flush_dirty`](super::ModelIdentityMap::flush_dirty)).
    pub fn flush_dirty<F>(&self, executor: &dyn LifeExecutor, f: F) -> Result<(), ActiveModelError>
    where
        F: FnMut(&dyn LifeExecutor, Rc<RefCell<E::Model>>) -> Result<(), ActiveModelError>,
    {
        self.drain_pending_into_map();
        self.inner.borrow_mut().flush_dirty(executor, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::{LifeError, LifeExecutor};
    use crate::model::ModelTrait;
    use crate::relation::identity::Identity;
    use crate::{LifeEntityName, LifeModelTrait};
    use may_postgres::Row;
    use sea_query::{Iden, IdenStatic, Value};

    struct NopExecutor;

    impl LifeExecutor for NopExecutor {
        fn execute(&self, _query: &str, _params: &[&dyn may_postgres::types::ToSql]) -> Result<u64, LifeError> {
            Ok(0)
        }

        fn query_one(
            &self,
            _query: &str,
            _params: &[&dyn may_postgres::types::ToSql],
        ) -> Result<Row, LifeError> {
            Err(LifeError::QueryError("nop".into()))
        }

        fn query_all(
            &self,
            _query: &str,
            _params: &[&dyn may_postgres::types::ToSql],
        ) -> Result<Vec<Row>, LifeError> {
            Ok(vec![])
        }
    }

    #[derive(Copy, Clone, Debug)]
    enum UCol {
        Id,
    }

    impl Iden for UCol {
        fn unquoted(&self) -> &'static str {
            match self {
                UCol::Id => "id",
            }
        }
    }

    impl IdenStatic for UCol {
        fn as_str(&self) -> &'static str {
            match self {
                UCol::Id => "id",
            }
        }
    }

    crate::impl_column_def_helper_for_test!(UCol);

    #[derive(Copy, Clone, Debug, Default)]
    struct UEnt;

    impl LifeEntityName for UEnt {
        fn table_name(&self) -> &'static str {
            "u"
        }
    }

    impl LifeModelTrait for UEnt {
        type Model = UMod;
        type Column = UCol;
    }

    #[derive(Clone, Debug)]
    struct UMod {
        id: i32,
    }

    impl ModelTrait for UMod {
        type Entity = UEnt;

        fn get(&self, column: UCol) -> Value {
            match column {
                UCol::Id => Value::Int(Some(self.id)),
            }
        }

        fn set(&mut self, column: UCol, value: Value) -> Result<(), crate::model::ModelError> {
            match column {
                UCol::Id => {
                    if let Value::Int(Some(v)) = value {
                        self.id = v;
                        Ok(())
                    } else {
                        Err(crate::model::ModelError::Other("bad id".into()))
                    }
                }
            }
        }

        fn get_primary_key_value(&self) -> Value {
            Value::Int(Some(self.id))
        }

        fn get_primary_key_identity(&self) -> Identity {
            Identity::Unary(sea_query::DynIden::from(UCol::Id.as_str()))
        }

        fn get_primary_key_values(&self) -> Vec<Value> {
            vec![Value::Int(Some(self.id))]
        }
    }

    #[test]
    fn session_register_and_flush_merges_pending() {
        let s = Session::<UEnt>::new();
        let n = s.dirty_notifier();
        let _ = s.register_loaded(UMod { id: 1 });
        let key = crate::session::fingerprint_pk_values(&[Value::Int(Some(1))]);
        n.notify_identity_map_dirty(Some(key));
        let ex = NopExecutor;
        let ex_ref: &dyn LifeExecutor = &ex;
        s.flush_dirty(ex_ref, |_, _| Ok(())).expect("flush");
        assert_eq!(s.dirty_len(), 0);
    }

    #[test]
    fn session_dirty_notifier_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SessionDirtyNotifier>();
    }
}
