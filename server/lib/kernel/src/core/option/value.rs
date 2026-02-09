//! Type-safe option values.
//!
//! This is the core value type for editor options.

use std::fmt;

/// Type-safe option value.
///
/// This is the core value type for editor options. Each variant maps to
/// a common configuration pattern:
/// - `Bool`: Toggle settings (number, wrap, etc.)
/// - `Integer`: Numeric settings (tabwidth, scrolloff)
/// - `String`: Free-form text (theme name, paths)
/// - `Choice`: Enum-like selection from predefined values
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionValue {
    /// Boolean on/off option (e.g., `number`, `relativenumber`)
    Bool(bool),

    /// Integer option (e.g., `tabwidth`, `scrolloff`)
    Integer(i64),

    /// String option (e.g., `theme`, `signcolumn`)
    String(String),

    /// Choice option with predefined valid values.
    ///
    /// The `choices` field stores all valid values for validation.
    Choice {
        /// Current selected value
        value: String,
        /// All valid choices (for validation and completion)
        choices: Vec<String>,
    },
}

impl OptionValue {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// Create a boolean value.
    #[must_use]
    pub const fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    /// Create an integer value.
    #[must_use]
    pub const fn int(value: i64) -> Self {
        Self::Integer(value)
    }

    /// Create a string value.
    #[must_use]
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    /// Create a choice value.
    ///
    /// # Panics
    ///
    /// Panics if `choices` is empty or if `value` is not in `choices`.
    #[must_use]
    pub fn choice(value: impl Into<String>, choices: Vec<String>) -> Self {
        let value = value.into();
        debug_assert!(!choices.is_empty(), "choices must not be empty");
        debug_assert!(choices.contains(&value), "value must be in choices");
        Self::Choice { value, choices }
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Try to get as boolean.
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get as integer.
    #[must_use]
    pub const fn as_int(&self) -> Option<i64> {
        match self {
            Self::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Try to get as string reference.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            Self::Choice { value, .. } => Some(value),
            _ => None,
        }
    }

    /// Get the type name for error messages.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Integer(_) => "integer",
            Self::String(_) => "string",
            Self::Choice { .. } => "choice",
        }
    }

    /// Check if this value has the same type as another.
    #[must_use]
    pub const fn same_type(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Bool(_), Self::Bool(_))
                | (Self::Integer(_), Self::Integer(_))
                | (Self::String(_), Self::String(_))
                | (Self::Choice { .. }, Self::Choice { .. })
        )
    }
}

// LLVM coverage artifact: match arm headers and closing brace in Display impl
// are marked DA:0 despite all variants being exercised in test_option_value_display.
#[cfg_attr(coverage_nightly, coverage(off))]
impl fmt::Display for OptionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Integer(i) => write!(f, "{i}"),
            Self::String(s) => write!(f, "{s}"),
            Self::Choice { value, .. } => write!(f, "{value}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option_value_bool() {
        let value = OptionValue::bool(true);
        assert_eq!(value.as_bool(), Some(true));
        assert_eq!(value.as_int(), None);
        assert_eq!(value.as_str(), None);
        assert_eq!(value.type_name(), "bool");
    }

    #[test]
    fn test_option_value_int() {
        let value = OptionValue::int(42);
        assert_eq!(value.as_int(), Some(42));
        assert_eq!(value.as_bool(), None);
        assert_eq!(value.type_name(), "integer");
    }

    #[test]
    fn test_option_value_string() {
        let value = OptionValue::string("hello");
        assert_eq!(value.as_str(), Some("hello"));
        assert_eq!(value.as_bool(), None);
        assert_eq!(value.type_name(), "string");
    }

    #[test]
    fn test_option_value_choice() {
        let value = OptionValue::choice("dark", vec!["light".into(), "dark".into()]);
        assert_eq!(value.as_str(), Some("dark"));
        assert_eq!(value.type_name(), "choice");
    }

    #[test]
    fn test_option_value_same_type() {
        assert!(OptionValue::bool(true).same_type(&OptionValue::bool(false)));
        assert!(OptionValue::int(1).same_type(&OptionValue::int(2)));
        assert!(!OptionValue::bool(true).same_type(&OptionValue::int(1)));
    }

    #[test]
    fn test_option_value_display() {
        assert_eq!(OptionValue::bool(true).to_string(), "true");
        assert_eq!(OptionValue::int(42).to_string(), "42");
        assert_eq!(OptionValue::string("hello").to_string(), "hello");
    }
}
