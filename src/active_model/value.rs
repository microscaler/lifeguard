//! `ActiveValue` enum for field value metadata.
//!
//! This module provides the `ActiveValue` enum which wraps field values with
//! information about whether they are set, unset, or have been modified.
//! Similar to `SeaORM`'s `ActiveValue`.

use sea_query::Value;

/// Wrapper for `ActiveModel` field values with metadata
///
/// Similar to `SeaORM`'s `ActiveValue`, this enum wraps field values with
/// information about whether they are set, unset, or have been modified.
///
/// # Example
///
/// ```no_run
/// use lifeguard::ActiveValue;
///
/// // Set value
/// let value = ActiveValue::Set(sea_query::Value::Int(Some(42)));
///
/// // Unset value (field not initialized)
/// let unset = ActiveValue::Unset;
///
/// // Not set (explicitly set to None for Option fields)
/// let not_set = ActiveValue::NotSet;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum ActiveValue {
    /// Value is set (field has a value)
    Set(Value),
    /// Value is not set (field is uninitialized/`None`)
    NotSet,
    /// Value is unset (field was never set, different from `NotSet` for `Option` fields)
    Unset,
}

impl ActiveValue {
    /// Convert to `Option<Value>`
    ///
    /// Returns `Some(Value)` if the value is `Set`, `None` otherwise.
    #[must_use]
    pub fn into_value(self) -> Option<Value> {
        match self {
            ActiveValue::Set(v) => Some(v),
            ActiveValue::NotSet | ActiveValue::Unset => None,
        }
    }

    /// Convert from `Option<Value>`
    ///
    /// Creates an `ActiveValue` from an `Option<Value>`:
    /// - `Some(value)` → `ActiveValue::Set(value)`
    /// - `None` → `ActiveValue::NotSet`
    #[must_use]
    pub fn from_value(value: Option<Value>) -> Self {
        match value {
            Some(v) => ActiveValue::Set(v),
            None => ActiveValue::NotSet,
        }
    }

    /// Check if the value is set
    #[must_use]
    pub fn is_set(&self) -> bool {
        matches!(self, ActiveValue::Set(_))
    }

    /// Check if the value is not set
    #[must_use]
    pub fn is_not_set(&self) -> bool {
        matches!(self, ActiveValue::NotSet)
    }

    /// Check if the value is unset
    #[must_use]
    pub fn is_unset(&self) -> bool {
        matches!(self, ActiveValue::Unset)
    }

    /// Get the value if set, otherwise return `None`
    #[must_use]
    pub fn as_value(&self) -> Option<&Value> {
        match self {
            ActiveValue::Set(v) => Some(v),
            ActiveValue::NotSet | ActiveValue::Unset => None,
        }
    }
}

impl From<Value> for ActiveValue {
    fn from(value: Value) -> Self {
        ActiveValue::Set(value)
    }
}

impl From<Option<Value>> for ActiveValue {
    fn from(value: Option<Value>) -> Self {
        ActiveValue::from_value(value)
    }
}

impl From<ActiveValue> for Option<Value> {
    fn from(value: ActiveValue) -> Self {
        value.into_value()
    }
}
