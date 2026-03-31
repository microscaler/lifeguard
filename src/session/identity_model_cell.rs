//! [`Send`] handle for linking a [`LifeRecord`] mutation path to an identity-map [`Rc`]`<`[`RefCell`]`<M>`>`.

use crate::model::ModelTrait;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

/// Opaque handle to the identity-map cell used by derived `LifeRecord::attach_session_with_model` (PRD §9).
///
/// `Rc<RefCell<M>>` is not [`Send`] because [`RefCell`] is not [`Sync`]. Session/identity-map usage
/// (PRD U-5) is **single-threaded** per [`super::Session`]; this type is [`Send`] only so
/// [`crate::active_model::ActiveModelTrait`] (which requires [`Send`]) can still be implemented for
/// derived records. **Do not** link a record and session across threads while both sides mutate the
/// shared model.
#[derive(Clone)]
pub struct SessionIdentityModelCell<M: ModelTrait> {
    rc: Rc<RefCell<M>>,
}

// SAFETY: See type-level doc. Sending this across threads is only sound if the `Rc` is not
// accessed concurrently from multiple threads; session UoW assumes one thread per map/session.
unsafe impl<M: ModelTrait + Send> Send for SessionIdentityModelCell<M> {}

impl<M: ModelTrait> SessionIdentityModelCell<M> {
    #[must_use]
    pub fn new(rc: &Rc<RefCell<M>>) -> Self {
        Self {
            rc: Rc::clone(rc),
        }
    }

    pub fn replace_with(&self, model: M) {
        *self.rc.borrow_mut() = model;
    }
}

impl<M: ModelTrait> fmt::Debug for SessionIdentityModelCell<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionIdentityModelCell(..)")
    }
}
