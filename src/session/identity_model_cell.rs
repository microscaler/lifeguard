//! Link from a derived [`LifeRecord`] to an identity-map [`Rc`]`<`[`RefCell`]`<M>`>` (PRD §9).
//!
//! # Threading and soundness
//!
//! [`SessionIdentityModelCell`] holds a clone of the same [`Rc`] as [`super::ModelIdentityMap`]. That
//! [`Rc`] **must not** be accessed concurrently from multiple OS threads: [`RefCell`] is not
//! [`Sync`], and [`Rc`] reference counts are not atomic in the sense required for safe cross-thread
//! sharing.
//!
//! Lifeguard’s session model is **single-threaded per unit of work** ([`super::Session`] is not
//! [`Send`] because it wraps `Rc<RefCell<ModelIdentityMap>>`). [`crate::active_model::ActiveModelTrait`]
//! nonetheless requires [`Send`] on records, so this type uses an **`unsafe impl` [`Send`]** to keep
//! derived [`LifeRecord`] types [`Send`] when `M: Send`.
//!
//! **Contract:** Treat a derived record with an attached session model link like the session itself:
//! only one thread may use the linked [`Rc`] at a time. Before moving a record to another thread,
//! call `detach_session()` on that record (or otherwise end the link) so the [`Rc`] is not used from
//! two threads. If the original thread still holds [`Session`] / the map while another thread uses an
//! attached record, that is **undefined behavior** (data race on [`RefCell`] / [`Rc`]).
//!
//! A type-system–sound alternative is shared storage such as [`Arc`](std::sync::Arc)`<`[`Mutex`](std::sync::Mutex)`<M>>`
//! (or [`RwLock`](std::sync::RwLock)) instead of [`Rc`]/[`RefCell`] for identity-map entries; that would be a
//! larger API and performance change. Note: [`Arc`](std::sync::Arc)`<`[`RefCell`](std::cell::RefCell)`<M>>` is **not** a drop-in
//! `Send` fix — [`RefCell`] is [`!Sync`](Sync), so `Arc<RefCell<_>>` does not regain `Send` the way `Arc<Mutex<_>>` does.
//!
//! See **`SECURITY_PROMPT.md`** (§A.3, `SessionIdentityModelCell`) for audit-tracked remediation options.

use crate::model::ModelTrait;
use std::cell::{BorrowMutError, RefCell};
use std::fmt;
use std::rc::Rc;

/// Opaque handle to the identity-map cell used by derived `LifeRecord::attach_session_with_model` (PRD §9).
///
/// See the **Threading and soundness** section in the module documentation for why this type
/// implements [`Send`] via `unsafe impl` and the **protocol** you must follow to avoid undefined behavior.
#[derive(Clone)]
pub struct SessionIdentityModelCell<M: ModelTrait> {
    rc: Rc<RefCell<M>>,
}

// SAFETY: `Rc` uses non-atomic reference counts; using clones from two OS threads is undefined
// behavior. `RefCell` is `!Sync`, so the shared cell must not be accessed concurrently from multiple
// threads either.
//
// We use `unsafe impl Send` only to satisfy `ActiveModelTrait: Send` on derived records.
// `LifeRecord` stores `Option<SessionIdentityModelCell<M>>`, and `Option<T>: Send` requires `T: Send`,
// so without this impl `LifeRecord` could not be `Send` even when no session link is active.
//
// Runtime invariant (not enforced by the type system): every clone of the inner `Rc` (including the
// `ModelIdentityMap` entry and this cell) must be used from at most one OS thread at a time, matching
// the single-threaded `Session` / map model (PRD §9). Before moving a record that used
// `attach_session_with_model`, call `detach_session` on that record (or otherwise end the link) so no
// other thread can touch the same `Rc`.
//
// Violating this after moving the record is UB (races on `Rc` refcount and/or `RefCell`). A sound
// alternative is `Arc<Mutex<M>>` (or `RwLock`) for identity-map storage; see module docs.
unsafe impl<M: ModelTrait + Send> Send for SessionIdentityModelCell<M> {}

impl<M: ModelTrait> SessionIdentityModelCell<M> {
    #[must_use]
    pub fn new(rc: &Rc<RefCell<M>>) -> Self {
        Self { rc: Rc::clone(rc) }
    }

    pub fn replace_with(&self, model: M) -> Result<(), BorrowMutError> {
        let mut guard = self.rc.try_borrow_mut()?;
        *guard = model;
        Ok(())
    }
}

impl<M: ModelTrait> fmt::Debug for SessionIdentityModelCell<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionIdentityModelCell(..)")
    }
}
