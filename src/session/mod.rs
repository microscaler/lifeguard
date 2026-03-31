//! Session / unit of work — **identity map** and **dirty tracking** (PRD Phase E).
//!
//! [`ModelIdentityMap`] ensures **at most one** shared in-memory handle per primary key
//! within the map (U-1). It is **explicit** — construct a map per unit of work; there is
//! no global session (U-3).
//!
//! # Dirty keys and flush (U-2)
//!
//! After you mutate a model through [`Rc`]`<`[`RefCell`]`<…>`>``, call [`ModelIdentityMap::mark_dirty`]
//! with that model’s primary key. If you edit a **`#[derive(LifeRecord)]`** value instead, call
//! [`ModelIdentityMap::mark_dirty_key`] with `record.identity_map_key()?` (all PK columns must be set).
//! [`ModelIdentityMap::flush_dirty`] visits **dirty** entries in
//! **lexicographic order of PK fingerprint** (stable, deterministic) and invokes your closure.
//! The closure typically builds a `LifeRecord` and calls [`crate::active_model::ActiveModelTrait::update`]
//! or `save` — the map does not generate SQL itself.
//!
//! Flush is **not** implicitly transactional; wrap the executor in [`crate::Transaction`] if you
//! need all writes in one database transaction.
//!
//! # Threading
//!
//! The map stores [`Rc`]`<`[`RefCell`]`<`[`ModelTrait`]::Model`>`>` — it is **not** [`Send`]
//! / [`Sync`]. Use one map per coroutine/thread or wrap externally if you need sharing (U-5).
//!
//! # Pooling (U-4)
//!
//! See `docs/planning/DESIGN_SESSION_UOW.md` for pooling (U-4). [`Session`] bundles the map and a
//! sendable [`SessionDirtyNotifier`] for `LifeRecord::attach_session` / `detach_session` (entities with a PK).
//!
//! Flush with [`LifeguardPool`](crate::LifeguardPool) via [`Session::flush_dirty`] and a [`PooledLifeExecutor`](crate::pool::PooledLifeExecutor) (or any [`LifeExecutor`](crate::executor::LifeExecutor)). For one DB transaction around the flush on a **direct** client, use [`Session::flush_dirty_in_transaction`](crate::session::Session::flush_dirty_in_transaction). For the same on a pool, use [`Session::flush_dirty_in_transaction_pooled`](crate::session::Session::flush_dirty_in_transaction_pooled).
//!
//! # See also
//!
//! - Project PRD §9 (session / UoW).

mod pk;
mod uow;

pub use pk::fingerprint_pk_values;
pub use uow::{Session, SessionDirtyNotifier};

use crate::active_model::ActiveModelError;
use crate::executor::LifeExecutor;
use crate::model::ModelTrait;
use crate::query::LifeModelTrait;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Identity map: at most one [`Rc`] per primary key fingerprint (U-1), plus optional **dirty**
/// flags for flush (U-2).
///
/// Call [`register_loaded`](Self::register_loaded) when you materialize a row from the database.
/// A second registration with the **same** primary key returns the **first** [`Rc`]; the
/// duplicate model is dropped (first load wins — avoids duplicate identities; refresh semantics
/// are application-defined).
///
/// [`mark_dirty`](Self::mark_dirty) only records keys that already exist in the map (registered
/// rows). Inserts pending only in memory are out of scope — use your normal `insert` path, then
/// [`register_loaded`] after the database assigns keys if you want them in the map.
pub struct ModelIdentityMap<E>
where
    E: LifeModelTrait,
    E::Model: ModelTrait<Entity = E> + Clone,
{
    map: HashMap<String, Rc<RefCell<E::Model>>>,
    dirty: HashSet<String>,
}

impl<E> ModelIdentityMap<E>
where
    E: LifeModelTrait,
    E::Model: ModelTrait<Entity = E> + Clone,
{
    /// Create an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            dirty: HashSet::new(),
        }
    }

    /// Register a model instance. Same PK → same [`Rc`]; duplicate model is dropped.
    pub fn register_loaded(&mut self, model: E::Model) -> Rc<RefCell<E::Model>> {
        let key = fingerprint_pk_values(&model.get_primary_key_values());
        if let Some(existing) = self.map.get(&key) {
            return existing.clone();
        }
        let rc = Rc::new(RefCell::new(model));
        self.map.insert(key, rc.clone());
        rc
    }

    /// Lookup without inserting.
    #[must_use]
    pub fn get_existing(&self, model: &E::Model) -> Option<Rc<RefCell<E::Model>>> {
        let key = fingerprint_pk_values(&model.get_primary_key_values());
        self.map.get(&key).cloned()
    }

    /// Number of distinct primary keys held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the map is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Mark a registered row as **dirty** (needs write-back). No-op if this primary key is not
    /// in the map.
    pub fn mark_dirty(&mut self, model: &E::Model) {
        let key = fingerprint_pk_values(&model.get_primary_key_values());
        if self.map.contains_key(&key) {
            self.dirty.insert(key);
        }
    }

    /// Mark dirty using a fingerprint string (e.g. from [`lifeguard::session::fingerprint_pk_values`]
    /// or a derived [`LifeRecord`](crate::active_model::ActiveModelTrait)’s `identity_map_key()`).
    /// No-op if the key is not registered.
    pub fn mark_dirty_key(&mut self, key: &str) {
        if self.map.contains_key(key) {
            self.dirty.insert(key.to_string());
        }
    }

    /// Remove the dirty flag without persisting.
    pub fn unmark_dirty(&mut self, model: &E::Model) {
        let key = fingerprint_pk_values(&model.get_primary_key_values());
        self.dirty.remove(&key);
    }

    /// Whether this primary key is marked dirty.
    #[must_use]
    pub fn is_marked_dirty(&self, model: &E::Model) -> bool {
        let key = fingerprint_pk_values(&model.get_primary_key_values());
        self.dirty.contains(&key)
    }

    /// Number of distinct dirty keys (subset of registered rows).
    #[must_use]
    pub fn dirty_len(&self) -> usize {
        self.dirty.len()
    }

    /// Drop all dirty flags without persisting.
    pub fn clear_dirty(&mut self) {
        self.dirty.clear();
    }

    /// Flush every dirty row in **lexicographic order of PK fingerprint** by calling `f` with the
    /// executor and the shared [`Rc`]. On success for a row, its dirty flag is cleared. On the
    /// first error, remaining dirty keys are left unchanged (including the failing key).
    pub fn flush_dirty<F>(&mut self, executor: &dyn LifeExecutor, mut f: F) -> Result<(), ActiveModelError>
    where
        F: FnMut(&dyn LifeExecutor, Rc<RefCell<E::Model>>) -> Result<(), ActiveModelError>,
    {
        let mut keys: Vec<String> = self.dirty.iter().cloned().collect();
        keys.sort();
        for key in keys {
            if !self.dirty.contains(&key) {
                continue;
            }
            let Some(rc) = self.map.get(&key).cloned() else {
                self.dirty.remove(&key);
                continue;
            };
            match f(executor, rc) {
                Ok(()) => {
                    self.dirty.remove(&key);
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

impl<E> Default for ModelIdentityMap<E>
where
    E: LifeModelTrait,
    E::Model: ModelTrait<Entity = E> + Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::active_model::ActiveModelError;
    use crate::executor::{LifeError, LifeExecutor};
    use crate::model::ModelTrait;
    use crate::relation::identity::Identity;
    use crate::{LifeEntityName, LifeModelTrait};
    use may_postgres::Row;
    use sea_query::{Iden, IdenStatic, Value};
    use std::cell::RefCell;

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
    enum SessCol {
        Id,
    }

    impl Iden for SessCol {
        fn unquoted(&self) -> &'static str {
            match self {
                SessCol::Id => "id",
            }
        }
    }

    impl IdenStatic for SessCol {
        fn as_str(&self) -> &'static str {
            match self {
                SessCol::Id => "id",
            }
        }
    }

    crate::impl_column_def_helper_for_test!(SessCol);

    #[derive(Copy, Clone, Debug, Default)]
    struct SessEntity;

    impl LifeEntityName for SessEntity {
        fn table_name(&self) -> &'static str {
            "sess"
        }
    }

    impl LifeModelTrait for SessEntity {
        type Model = SessModel;
        type Column = SessCol;
    }

    #[derive(Clone, Debug)]
    struct SessModel {
        id: i32,
        label: &'static str,
    }

    impl ModelTrait for SessModel {
        type Entity = SessEntity;

        fn get(&self, column: SessCol) -> Value {
            match column {
                SessCol::Id => Value::Int(Some(self.id)),
            }
        }

        fn set(&mut self, column: SessCol, value: Value) -> Result<(), crate::model::ModelError> {
            match column {
                SessCol::Id => {
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
            Identity::Unary(sea_query::DynIden::from(SessCol::Id.as_str()))
        }

        fn get_primary_key_values(&self) -> Vec<Value> {
            vec![Value::Int(Some(self.id))]
        }
    }

    #[test]
    fn identity_map_same_rc_same_pk() {
        let mut map = ModelIdentityMap::<SessEntity>::new();
        let r1 = map.register_loaded(SessModel {
            id: 1,
            label: "first",
        });
        let r2 = map.register_loaded(SessModel {
            id: 1,
            label: "second",
        });
        assert!(Rc::ptr_eq(&r1, &r2));
        assert_eq!(r1.borrow().label, "first");
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn identity_map_explicit_new_not_global() {
        let mut a = ModelIdentityMap::<SessEntity>::new();
        let b = ModelIdentityMap::<SessEntity>::new();
        let _ = a.register_loaded(SessModel {
            id: 1,
            label: "a",
        });
        assert!(b.is_empty());
    }

    #[test]
    fn get_existing_finds_registered() {
        let mut map = ModelIdentityMap::<SessEntity>::new();
        let m = SessModel {
            id: 7,
            label: "x",
        };
        let r1 = map.register_loaded(m.clone());
        let probe = SessModel {
            id: 7,
            label: "ignored",
        };
        assert!(map
            .get_existing(&probe)
            .is_some_and(|r2| Rc::ptr_eq(&r1, &r2)));
    }

    #[test]
    fn mark_dirty_ignores_unregistered_pk() {
        let mut map = ModelIdentityMap::<SessEntity>::new();
        let orphan = SessModel {
            id: 99,
            label: "n",
        };
        map.mark_dirty(&orphan);
        assert_eq!(map.dirty_len(), 0);
    }

    #[test]
    fn mark_dirty_key_matches_fingerprint() {
        let mut map = ModelIdentityMap::<SessEntity>::new();
        let _ = map.register_loaded(SessModel {
            id: 5,
            label: "a",
        });
        let key = fingerprint_pk_values(&[Value::Int(Some(5))]);
        map.mark_dirty_key(&key);
        assert_eq!(map.dirty_len(), 1);
        assert!(map.is_marked_dirty(&SessModel {
            id: 5,
            label: "x",
        }));
    }

    #[test]
    fn flush_dirty_lexicographic_order() {
        let mut map = ModelIdentityMap::<SessEntity>::new();
        let _ = map.register_loaded(SessModel {
            id: 10,
            label: "a",
        });
        let _ = map.register_loaded(SessModel {
            id: 2,
            label: "b",
        });
        map.mark_dirty(&SessModel {
            id: 10,
            label: "a",
        });
        map.mark_dirty(&SessModel {
            id: 2,
            label: "b",
        });
        let order = RefCell::new(Vec::new());
        let ex = NopExecutor;
        let ex_ref: &dyn LifeExecutor = &ex;
        let flush_result = map.flush_dirty(ex_ref, |_, rc| {
            order.borrow_mut().push(rc.borrow().id);
            Ok(())
        });
        assert!(flush_result.is_ok(), "flush_dirty should succeed");
        // "i:10" sorts before "i:2" as strings
        assert_eq!(order.into_inner(), vec![10, 2]);
        assert_eq!(map.dirty_len(), 0);
    }

    #[test]
    fn flush_dirty_error_leaves_failed_key_dirty() {
        let mut map = ModelIdentityMap::<SessEntity>::new();
        let _ = map.register_loaded(SessModel {
            id: 10,
            label: "a",
        });
        let _ = map.register_loaded(SessModel {
            id: 2,
            label: "b",
        });
        map.mark_dirty(&SessModel {
            id: 10,
            label: "a",
        });
        map.mark_dirty(&SessModel {
            id: 2,
            label: "b",
        });
        let ex = NopExecutor;
        let ex_ref: &dyn LifeExecutor = &ex;
        let flush_result = map.flush_dirty(ex_ref, |_, rc| {
            if rc.borrow().id == 2 {
                Err(ActiveModelError::Other("fail".into()))
            } else {
                Ok(())
            }
        });
        assert_eq!(
            flush_result,
            Err(ActiveModelError::Other("fail".into()))
        );
        assert!(!map.is_marked_dirty(&SessModel {
            id: 10,
            label: "x",
        }));
        assert!(map.is_marked_dirty(&SessModel {
            id: 2,
            label: "x",
        }));
    }
}
