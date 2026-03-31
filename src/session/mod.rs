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
//! [`ModelIdentityMap::flush_dirty_with_map_key`] walks dirty rows in **lexicographic map-key order**
//! (pending-insert keys first; see [`register_pending_insert`]) and passes the key so the closure can
//! call `insert` vs `update`.
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
//! sendable [`SessionDirtyNotifier`] for `LifeRecord::attach_session`, `attach_session_with_model`, and `detach_session` (entities with a PK).
//!
//! Flush with [`LifeguardPool`](crate::LifeguardPool) via [`Session::flush_dirty`] and a [`PooledLifeExecutor`](crate::pool::PooledLifeExecutor) (or any [`LifeExecutor`](crate::executor::LifeExecutor)). For one DB transaction around the flush on a **direct** client, use [`Session::flush_dirty_in_transaction`](crate::session::Session::flush_dirty_in_transaction). For the same on a pool, use [`Session::flush_dirty_in_transaction_pooled`](crate::session::Session::flush_dirty_in_transaction_pooled).
//!
//! # See also
//!
//! - Project PRD §9 (session / UoW).

mod identity_model_cell;
mod pk;
mod uow;

pub use identity_model_cell::SessionIdentityModelCell;
pub use pk::fingerprint_pk_values;
pub use uow::{Session, SessionDirtyNotifier};

/// Prefix for [`ModelIdentityMap::register_pending_insert`] keys (not a primary-key fingerprint).
pub const PENDING_INSERT_KEY_PREFIX: &str = "__lg_insert__\x1f";

/// `true` when `key` was returned from [`ModelIdentityMap::register_pending_insert`].
#[inline]
#[must_use]
pub fn is_pending_insert_key(key: &str) -> bool {
    key.starts_with(PENDING_INSERT_KEY_PREFIX)
}

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
/// rows). For **insert-only** rows (no stable PK fingerprint yet), use
/// [`register_pending_insert`](Self::register_pending_insert) and flush with
/// [`flush_dirty_with_map_key`](Self::flush_dirty_with_map_key) so the closure can branch on
/// [`is_pending_insert_key`].
pub struct ModelIdentityMap<E>
where
    E: LifeModelTrait,
    E::Model: ModelTrait<Entity = E> + Clone,
{
    map: HashMap<String, Rc<RefCell<E::Model>>>,
    dirty: HashSet<String>,
    /// Monotonic id for [`Self::register_pending_insert`] keys (`PENDING_INSERT_KEY_PREFIX` + id).
    next_pending_id: u64,
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
            next_pending_id: 0,
        }
    }

    /// Register a new row that will be **inserted** (no stable PK fingerprint in the map yet).
    ///
    /// Returns `(map_key, rc)` — keep `map_key` for [`Self::promote_pending_to_loaded`] after a
    /// successful insert, and use [`Self::flush_dirty_with_map_key`] in the flush closure to call
    /// `insert` when [`is_pending_insert_key`](crate::session::is_pending_insert_key)(`map_key`).
    pub fn register_pending_insert(&mut self, model: E::Model) -> (String, Rc<RefCell<E::Model>>) {
        let id = self.next_pending_id;
        self.next_pending_id = self.next_pending_id.wrapping_add(1);
        let key = format!("{PENDING_INSERT_KEY_PREFIX}{id}");
        let rc = Rc::new(RefCell::new(model));
        self.map.insert(key.clone(), rc.clone());
        self.dirty.insert(key.clone());
        (key, rc)
    }

    /// After a successful insert, replace the pending entry with the loaded row (real PK).
    ///
    /// Removes the synthetic pending key, then [`register_loaded`](Self::register_loaded) with
    /// `model` (typically with generated PK from the database).
    pub fn promote_pending_to_loaded(
        &mut self,
        pending_key: &str,
        model: E::Model,
    ) -> Result<Rc<RefCell<E::Model>>, ActiveModelError> {
        if !is_pending_insert_key(pending_key) {
            return Err(ActiveModelError::Other(
                "promote_pending_to_loaded: key is not a pending insert key".to_string(),
            ));
        }
        if !self.map.contains_key(pending_key) {
            return Err(ActiveModelError::Other(
                "promote_pending_to_loaded: key not in identity map".to_string(),
            ));
        }
        self.map.remove(pending_key);
        self.dirty.remove(pending_key);
        Ok(self.register_loaded(model))
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

    /// Flush every dirty row in **lexicographic order of map key** (pending-insert keys sort under
    /// [`PENDING_INSERT_KEY_PREFIX`], then normal PK fingerprints) by calling `f` with the executor,
    /// the shared [`Rc`], and the internal map key string (use [`is_pending_insert_key`] to branch
    /// insert vs update).
    pub fn flush_dirty_with_map_key<F>(
        &mut self,
        executor: &dyn LifeExecutor,
        mut f: F,
    ) -> Result<(), ActiveModelError>
    where
        F: FnMut(&dyn LifeExecutor, Rc<RefCell<E::Model>>, &str) -> Result<(), ActiveModelError>,
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
            match f(executor, rc, key.as_str()) {
                Ok(()) => {
                    self.dirty.remove(&key);
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Flush every dirty row in **lexicographic order of map key** by calling `f` with the executor
    /// and the shared [`Rc`]. On success for a row, its dirty flag is cleared. On the first error,
    /// remaining dirty keys are left unchanged (including the failing key).
    pub fn flush_dirty<F>(&mut self, executor: &dyn LifeExecutor, mut f: F) -> Result<(), ActiveModelError>
    where
        F: FnMut(&dyn LifeExecutor, Rc<RefCell<E::Model>>) -> Result<(), ActiveModelError>,
    {
        self.flush_dirty_with_map_key(executor, |ex, rc, _key| f(ex, rc))
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
    use std::rc::Rc;

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

    #[test]
    #[allow(clippy::expect_used)] // Test code - expect is acceptable
    fn register_pending_insert_flush_with_map_key_and_promote() {
        let mut map = ModelIdentityMap::<SessEntity>::new();
        let (k, _rc) = map.register_pending_insert(SessModel {
            id: 0,
            label: "new",
        });
        assert!(is_pending_insert_key(&k));
        assert_eq!(map.dirty_len(), 1);
        let ex = NopExecutor;
        let ex_ref: &dyn LifeExecutor = &ex;
        map.flush_dirty_with_map_key(ex_ref, |_, _, key| {
            assert!(is_pending_insert_key(key));
            Ok(())
        })
        .expect("flush");
        assert_eq!(map.dirty_len(), 0);
        assert_eq!(map.len(), 1);
        let r2 = map
            .promote_pending_to_loaded(&k, SessModel {
                id: 42,
                label: "saved",
            })
            .expect("promote");
        assert_eq!(r2.borrow().id, 42);
        assert_eq!(map.len(), 1);
        assert!(map
            .get_existing(&SessModel {
                id: 42,
                label: "probe",
            })
            .is_some_and(|r| Rc::ptr_eq(&r, &r2)));
    }

    #[test]
    fn session_identity_model_cell_replace_with_updates_rc() {
        let mut map = ModelIdentityMap::<SessEntity>::new();
        let rc = map.register_loaded(SessModel {
            id: 1,
            label: "a",
        });
        let cell = SessionIdentityModelCell::new(&rc);
        cell.replace_with(SessModel {
            id: 1,
            label: "b",
        });
        assert_eq!(rc.borrow().label, "b");
    }
}
