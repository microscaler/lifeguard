//! [`ActiveValue<T>`] — the state of one column in a change-set.
//!
//! # The model
//!
//! A change-set is a set of *intentions* about a row, not a copy of it. Each
//! column answers two independent questions: **will we write it?** and **what
//! will we write?** Those are separate axes, so the field that records them
//! has four inhabitants rather than two:
//!
//! | State | INSERT | UPDATE |
//! | --- | --- | --- |
//! | [`NotSet`](ActiveValue::NotSet) | omitted (column DEFAULT applies) | omitted |
//! | [`Unchanged`](ActiveValue::Unchanged) | written (it is a real value) | omitted |
//! | [`Set`](ActiveValue::Set) | written | written |
//! | [`SetNull`](ActiveValue::SetNull) | written as NULL (beats the DEFAULT) | written as NULL |
//! | [`Expr`](ActiveValue::Expr) | rejected | written as the expression |
//!
//! An `Option<T>` cannot carry this: it has one slot for two answers, so
//! "leave this column alone" and "write NULL to it" would have to share the
//! `None` case, and any statement builder would have to guess which was meant.
//! There is no correct guess — the two are opposite instructions.
//!
//! # The two rules that follow
//!
//! **Calling a setter always has an effect.** `set_x(None)` writes NULL; to
//! leave a column alone, do not call its setter. Clearing a column is
//! therefore visible at the call site, which is where a reader looks to find
//! out what a write does.
//!
//! **A loaded value is not a pending write.** `from_model` marks every column
//! [`Unchanged`](ActiveValue::Unchanged): the values are there to read, but
//! nothing is staged. An `UPDATE` built from an edited row touches only the
//! columns that were set, so it cannot clobber a concurrent edit to a column
//! this caller never looked at. When you *do* mean to write the whole row
//! back — a unit-of-work flush, say — `overwrite` states that explicitly.

use sea_query::{SimpleExpr, Value};

/// The state of one column in a change-set.
///
/// Construct these through the generated `set_*` / `set_*_null` / `set_*_expr`
/// methods rather than by hand; the variants are public so that matching on a
/// field is possible, not because building one is the expected workflow.
#[derive(Debug, Clone, Default)]
pub enum ActiveValue<T> {
    /// Never touched. Omitted from every statement, so the column keeps its
    /// value on UPDATE and takes its DEFAULT on INSERT.
    #[default]
    NotSet,
    /// Loaded from the database and not modified since.
    ///
    /// Carries `Option<T>` because the stored value may itself be NULL.
    /// Omitted from UPDATE — that is the point of the variant — but written on
    /// INSERT, where it is simply a value we hold.
    Unchanged(Option<T>),
    /// Write this value.
    Set(T),
    /// Write SQL `NULL`.
    ///
    /// Distinct from [`NotSet`](ActiveValue::NotSet): this is an instruction,
    /// not an absence.
    SetNull,
    /// Write the result of a database expression (`SET col = col + 1`).
    ///
    /// UPDATE only; [`insert`](crate::ActiveModelTrait::insert) rejects it,
    /// since the expression usually references the column's current value.
    Expr(SimpleExpr),
}

impl<T> ActiveValue<T> {
    /// The value this column currently holds in the change-set, if any.
    ///
    /// `Set(v)` and `Unchanged(Some(v))` have a value; `SetNull`, `NotSet`,
    /// `Unchanged(None)` and `Expr` do not. Note that "no value" is not the
    /// same as "will not be written" — check [`is_staged`](Self::is_staged)
    /// for that.
    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Set(v) | Self::Unchanged(Some(v)) => Some(v),
            Self::NotSet | Self::SetNull | Self::Unchanged(None) | Self::Expr(_) => None,
        }
    }

    /// Consume the state and return its value, if it has one.
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_value(self) -> Option<T> {
        match self {
            Self::Set(v) | Self::Unchanged(Some(v)) => Some(v),
            Self::NotSet | Self::SetNull | Self::Unchanged(None) | Self::Expr(_) => None,
        }
    }

    /// Whether this column will appear in an `UPDATE`: it was explicitly
    /// assigned a value, a NULL, or an expression.
    pub const fn is_staged(&self) -> bool {
        matches!(self, Self::Set(_) | Self::SetNull | Self::Expr(_))
    }

    /// Whether this column was explicitly staged as SQL `NULL`.
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::SetNull)
    }

    /// Whether this column has never been touched.
    pub const fn is_not_set(&self) -> bool {
        matches!(self, Self::NotSet)
    }

    /// Whether this column came from the database and has not been modified.
    pub const fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged(_))
    }

    /// Whether an INSERT should include this column. `Unchanged` counts: on an
    /// insert it is a value we hold, not a value we are declining to write.
    pub const fn is_insertable(&self) -> bool {
        matches!(self, Self::Set(_) | Self::SetNull | Self::Unchanged(_))
    }

    /// The value as the model sees it: `Option<T>`, collapsing "not set" and
    /// "null" together, since a model has no notion of an unwritten column.
    #[allow(clippy::missing_const_for_fn)]
    pub fn to_model_value(self) -> Option<T> {
        match self {
            Self::Set(v) | Self::Unchanged(Some(v)) => Some(v),
            Self::NotSet | Self::SetNull | Self::Unchanged(None) | Self::Expr(_) => None,
        }
    }

    /// Build from an `Option`, where `None` means SQL `NULL`.
    ///
    /// This is the rule the generated setters follow: calling a setter always
    /// has an effect, so passing `None` is an instruction to write NULL. To
    /// leave a column alone, do not call its setter.
    pub fn from_option(value: Option<T>) -> Self {
        match value {
            Some(v) => Self::Set(v),
            None => Self::SetNull,
        }
    }

    /// Build the `Unchanged` state from a loaded `Option`.
    pub const fn unchanged(value: Option<T>) -> Self {
        Self::Unchanged(value)
    }
}

impl<T: PartialEq> PartialEq for ActiveValue<T> {
    /// `Expr` never compares equal, even to itself: two identical expressions
    /// may still produce different values, so claiming equality would be a
    /// lie the type cannot back up.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::NotSet, Self::NotSet) | (Self::SetNull, Self::SetNull) => true,
            (Self::Set(a), Self::Set(b)) => a == b,
            (Self::Unchanged(a), Self::Unchanged(b)) => a == b,
            _ => false,
        }
    }
}

/// The dynamic, column-erased view of a field's value.
///
/// Returned by [`ActiveModelTrait::into_column_value`](crate::ActiveModelTrait)
/// where the column is chosen at runtime and `T` is therefore unknown. Prefer
/// matching on the typed [`ActiveValue`] when you have the concrete field.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnValue {
    /// The column carries a value.
    Set(Value),
    /// The column is staged as SQL `NULL`.
    Null,
    /// The column is not part of this change-set.
    NotSet,
}

impl ColumnValue {
    /// The value, if the column carries one.
    #[must_use]
    pub const fn as_value(&self) -> Option<&Value> {
        match self {
            Self::Set(v) => Some(v),
            Self::Null | Self::NotSet => None,
        }
    }

    /// Consume and return the value, if the column carries one.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_value(self) -> Option<Value> {
        match self {
            Self::Set(v) => Some(v),
            Self::Null | Self::NotSet => None,
        }
    }

    /// Whether the column carries a value.
    #[must_use]
    pub const fn is_set(&self) -> bool {
        matches!(self, Self::Set(_))
    }

    /// Whether the column is staged as SQL `NULL`.
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Whether the column is absent from the change-set.
    #[must_use]
    pub const fn is_not_set(&self) -> bool {
        matches!(self, Self::NotSet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The distinction the whole type exists for.
    #[test]
    fn not_set_and_set_null_are_different_states() {
        let untouched: ActiveValue<i32> = ActiveValue::NotSet;
        let cleared: ActiveValue<i32> = ActiveValue::SetNull;

        assert!(!untouched.is_staged(), "an untouched column is not written");
        assert!(cleared.is_staged(), "a cleared column IS written");
        assert_ne!(untouched, cleared);

        // Both have no value — which is exactly why `Option<T>` could not
        // tell them apart.
        assert_eq!(untouched.value(), None);
        assert_eq!(cleared.value(), None);
    }

    /// `Unchanged` is the read-modify-write state: it holds a value but does
    /// not ask for it to be written back.
    #[test]
    fn unchanged_holds_a_value_without_staging_a_write() {
        let loaded = ActiveValue::Unchanged(Some(7));
        assert_eq!(loaded.value(), Some(&7));
        assert!(!loaded.is_staged(), "UPDATE must skip it");
        assert!(loaded.is_insertable(), "INSERT must include it");
    }

    /// A loaded NULL is still `Unchanged`, not `SetNull` — reading a NULL
    /// column must not turn into an instruction to write one.
    #[test]
    fn a_loaded_null_is_unchanged_not_staged() {
        let loaded: ActiveValue<i32> = ActiveValue::Unchanged(None);
        assert!(!loaded.is_staged());
        assert!(!loaded.is_null(), "SetNull is an instruction; this is not");
        assert!(loaded.is_insertable());
    }

    /// The setter rule: `None` in means NULL out.
    #[test]
    fn from_option_maps_none_to_a_staged_null() {
        assert_eq!(ActiveValue::from_option(Some(3)), ActiveValue::Set(3));
        assert_eq!(ActiveValue::from_option(None::<i32>), ActiveValue::SetNull);
    }

    #[test]
    fn to_model_value_collapses_absence_and_null() {
        assert_eq!(ActiveValue::Set(1).to_model_value(), Some(1));
        assert_eq!(ActiveValue::Unchanged(Some(1)).to_model_value(), Some(1));
        assert_eq!(ActiveValue::<i32>::SetNull.to_model_value(), None);
        assert_eq!(ActiveValue::<i32>::NotSet.to_model_value(), None);
    }

    #[test]
    fn default_is_not_set() {
        assert!(ActiveValue::<String>::default().is_not_set());
    }

    /// Expressions are opaque: two that look alike may not evaluate alike, so
    /// equality must not pretend otherwise.
    #[test]
    fn expressions_are_never_equal() {
        let a: ActiveValue<i32> = ActiveValue::Expr(sea_query::Expr::val(1));
        let b: ActiveValue<i32> = ActiveValue::Expr(sea_query::Expr::val(1));
        assert_ne!(a, b);
        assert!(a.is_staged());
    }
}
