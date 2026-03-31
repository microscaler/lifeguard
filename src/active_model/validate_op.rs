//! Operation discriminator for [`super::traits::ActiveModelBehavior::validate_fields`] /
//! [`super::traits::ActiveModelBehavior::validate_model`] (PRD Phase B).

/// How [`super::validation::run_validators`] combines errors from [`super::traits::ActiveModelBehavior::validate_fields`]
/// and [`super::traits::ActiveModelBehavior::validate_model`] (PRD V-3).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ValidationStrategy {
    /// Stop on the first failing validator hook (default).
    #[default]
    FailFast,
    /// Run `validate_fields`, then `validate_model`, and return all `Validation` errors in one `Vec`.
    ///
    /// Non-validation errors (e.g. `DatabaseError`) still abort immediately on first occurrence.
    Aggregate,
}

/// Which persistence path is running validation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ValidateOp {
    /// `ActiveModelTrait::insert` / `save` insert branch
    Insert,
    /// `ActiveModelTrait::update` / `save` update branch
    Update,
    /// `ActiveModelTrait::delete` (after `before_delete`, before SQL)
    Delete,
}

/// One failed validation rule (field-level or model-level).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// `None` for model-level rules that are not tied to a single column.
    pub field: Option<String>,
    pub message: String,
}

impl ValidationError {
    /// Field-scoped error (e.g. email format).
    #[must_use]
    pub fn field(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: Some(field.into()),
            message: message.into(),
        }
    }

    /// Model-scoped error (e.g. cross-field invariant).
    #[must_use]
    pub fn model(message: impl Into<String>) -> Self {
        Self {
            field: None,
            message: message.into(),
        }
    }
}
