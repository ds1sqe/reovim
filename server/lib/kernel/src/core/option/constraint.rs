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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_range() {
        let constraint = OptionConstraint::range(1, 10);

        // Valid values
        assert!(constraint.validate(&OptionValue::int(1)).is_ok());
        assert!(constraint.validate(&OptionValue::int(5)).is_ok());
        assert!(constraint.validate(&OptionValue::int(10)).is_ok());

        // Invalid values
        assert!(constraint.validate(&OptionValue::int(0)).is_err());
        assert!(constraint.validate(&OptionValue::int(11)).is_err());
    }

    #[test]
    fn test_constraint_string_length() {
        let constraint = OptionConstraint::string_length(2, 5);

        assert!(constraint.validate(&OptionValue::string("ab")).is_ok());
        assert!(constraint.validate(&OptionValue::string("abcde")).is_ok());
        assert!(constraint.validate(&OptionValue::string("a")).is_err());
        assert!(constraint.validate(&OptionValue::string("abcdef")).is_err());
    }

    #[test]
    fn test_constraint_choice() {
        let value = OptionValue::choice("valid", vec!["valid".into(), "other".into()]);
        let constraint = OptionConstraint::none();
        assert!(constraint.validate(&value).is_ok());
    }

    // === min/max constructors ===

    #[test]
    fn test_constraint_min_only() {
        let constraint = OptionConstraint::min(5);
        assert!(constraint.validate(&OptionValue::int(5)).is_ok());
        assert!(constraint.validate(&OptionValue::int(100)).is_ok());
        assert!(constraint.validate(&OptionValue::int(4)).is_err());
    }

    #[test]
    fn test_constraint_max_only() {
        let constraint = OptionConstraint::max(10);
        assert!(constraint.validate(&OptionValue::int(10)).is_ok());
        assert!(constraint.validate(&OptionValue::int(0)).is_ok());
        assert!(constraint.validate(&OptionValue::int(11)).is_err());
    }

    // === ConstraintError Display coverage ===

    #[test]
    fn test_constraint_error_below_minimum_display() {
        let err = ConstraintError::BelowMinimum { value: 3, min: 5 };
        let msg = err.to_string();
        assert!(msg.contains('3'));
        assert!(msg.contains("below minimum"));
        assert!(msg.contains('5'));
    }

    #[test]
    fn test_constraint_error_above_maximum_display() {
        let err = ConstraintError::AboveMaximum { value: 20, max: 10 };
        let msg = err.to_string();
        assert!(msg.contains("20"));
        assert!(msg.contains("above maximum"));
        assert!(msg.contains("10"));
    }

    #[test]
    fn test_constraint_error_string_too_short_display() {
        let err = ConstraintError::StringTooShort { len: 1, min: 3 };
        let msg = err.to_string();
        assert!(msg.contains('1'));
        assert!(msg.contains("below minimum"));
        assert!(msg.contains('3'));
    }

    #[test]
    fn test_constraint_error_string_too_long_display() {
        let err = ConstraintError::StringTooLong { len: 10, max: 5 };
        let msg = err.to_string();
        assert!(msg.contains("10"));
        assert!(msg.contains("above maximum"));
        assert!(msg.contains('5'));
    }

    #[test]
    fn test_constraint_error_invalid_choice_display() {
        let err = ConstraintError::InvalidChoice {
            value: "bad".to_string(),
            choices: vec!["good".to_string(), "ok".to_string()],
        };
        let msg = err.to_string();
        assert!(msg.contains("bad"));
        assert!(msg.contains("not a valid choice"));
    }

    #[test]
    fn test_constraint_error_is_std_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(ConstraintError::BelowMinimum { value: 1, min: 5 });
        assert!(err.to_string().contains("below minimum"));
    }

    // === invalid choice validation ===

    #[test]
    fn test_constraint_invalid_choice_validation() {
        // Construct the Choice variant directly to bypass debug_assert in choice()
        let value = OptionValue::Choice {
            value: "invalid".into(),
            choices: vec!["a".into(), "b".into()],
        };
        let constraint = OptionConstraint::none();
        let result = constraint.validate(&value);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConstraintError::InvalidChoice { .. }));
    }

    // === bool passthrough ===

    #[test]
    fn test_constraint_bool_passthrough() {
        // Range constraint should not affect booleans
        let constraint = OptionConstraint::range(1, 10);
        assert!(constraint.validate(&OptionValue::Bool(true)).is_ok());
        assert!(constraint.validate(&OptionValue::Bool(false)).is_ok());
    }
}
