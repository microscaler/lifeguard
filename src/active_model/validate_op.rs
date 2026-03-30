//! Operation discriminator for [`super::traits::ActiveModelBehavior::validate_fields`] /
//! [`super::traits::ActiveModelBehavior::validate_model`] (PRD Phase B).

/// Which persistence path is running validation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ValidateOp {
    /// `ActiveModelTrait::insert` / `save` insert branch
    Insert,
    /// `ActiveModelTrait::update` / `save` update branch
    Update,
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
