//! Option value constraints and validation.

use std::fmt;

use super::value::OptionValue;

/// Constraints for option value validation.
///
/// These constraints are checked when setting option values.
/// Not all constraints apply to all value types.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptionConstraint {
    /// For Integer: minimum value (inclusive).
    pub min: Option<i64>,
    /// For Integer: maximum value (inclusive).
    pub max: Option<i64>,
    /// For String: minimum length.
    pub min_length: Option<usize>,
    /// For String: maximum length.
    pub max_length: Option<usize>,
}

impl OptionConstraint {
    /// Create a constraint with no restrictions.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            min: None,
            max: None,
            min_length: None,
            max_length: None,
        }
    }

    /// Create a constraint with integer range (inclusive).
    #[must_use]
    pub const fn range(min: i64, max: i64) -> Self {
        Self {
            min: Some(min),
            max: Some(max),
            min_length: None,
            max_length: None,
        }
    }

    /// Create a constraint with minimum integer value.
    #[must_use]
    pub const fn min(min: i64) -> Self {
        Self {
            min: Some(min),
            max: None,
            min_length: None,
            max_length: None,
        }
    }

    /// Create a constraint with maximum integer value.
    #[must_use]
    pub const fn max(max: i64) -> Self {
        Self {
            min: None,
            max: Some(max),
            min_length: None,
            max_length: None,
        }
    }

    /// Create a constraint with string length range.
    #[must_use]
    pub const fn string_length(min_length: usize, max_length: usize) -> Self {
        Self {
            min: None,
            max: None,
            min_length: Some(min_length),
            max_length: Some(max_length),
        }
    }

    /// Validate a value against this constraint.
    ///
    /// # Errors
    ///
    /// Returns `ConstraintError` if the value violates the constraint.
    pub fn validate(&self, value: &OptionValue) -> Result<(), ConstraintError> {
        match value {
            OptionValue::Integer(i) => {
                if let Some(min) = self.min
                    && *i < min
                {
                    return Err(ConstraintError::BelowMinimum { value: *i, min });
                }
                if let Some(max) = self.max
                    && *i > max
                {
                    return Err(ConstraintError::AboveMaximum { value: *i, max });
                }
            }
            OptionValue::String(s) => {
                if let Some(min_len) = self.min_length
                    && s.len() < min_len
                {
                    return Err(ConstraintError::StringTooShort {
                        len: s.len(),
                        min: min_len,
                    });
                }
                if let Some(max_len) = self.max_length
                    && s.len() > max_len
                {
                    return Err(ConstraintError::StringTooLong {
                        len: s.len(),
                        max: max_len,
                    });
                }
            }
            OptionValue::Choice { value, choices } => {
                if !choices.contains(value) {
                    return Err(ConstraintError::InvalidChoice {
                        value: value.clone(),
                        choices: choices.clone(),
                    });
                }
            }
            OptionValue::Bool(_) => {
                // No constraints for boolean values
            }
        }
        Ok(())
    }
}

/// Constraint validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintError {
    /// Integer value is below minimum.
    BelowMinimum {
        /// The actual value
        value: i64,
        /// The minimum allowed
        min: i64,
    },
    /// Integer value is above maximum.
    AboveMaximum {
        /// The actual value
        value: i64,
        /// The maximum allowed
        max: i64,
    },
    /// String is too short.
    StringTooShort {
        /// Actual length
        len: usize,
        /// Minimum required length
        min: usize,
    },
    /// String is too long.
    StringTooLong {
        /// Actual length
        len: usize,
        /// Maximum allowed length
        max: usize,
    },
    /// Choice value is not in the allowed choices.
    InvalidChoice {
        /// The invalid value
        value: String,
        /// The valid choices
        choices: Vec<String>,
    },
}

impl fmt::Display for ConstraintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BelowMinimum { value, min } => {
                write!(f, "value {value} is below minimum {min}")
            }
            Self::AboveMaximum { value, max } => {
                write!(f, "value {value} is above maximum {max}")
            }
            Self::StringTooShort { len, min } => {
                write!(f, "string length {len} is below minimum {min}")
            }
            Self::StringTooLong { len, max } => {
                write!(f, "string length {len} is above maximum {max}")
            }
            Self::InvalidChoice { value, choices } => {
                write!(f, "'{value}' is not a valid choice (valid: {choices:?})")
            }
        }
    }
}

impl std::error::Error for ConstraintError {}
