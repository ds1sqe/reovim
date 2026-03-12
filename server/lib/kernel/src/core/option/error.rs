//! Option error types.

use std::fmt;

use super::{OptionScopeId, OptionValue, scope::OptionScope};

/// Error type for option operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionError {
    /// Option not found in registry.
    NotFound(String),

    /// Option already registered.
    AlreadyExists(String),

    /// Validation failed.
    ValidationFailed {
        /// Option name
        name: String,
        /// Reason for failure
        reason: String,
    },

    /// Type mismatch.
    TypeMismatch {
        /// Option name
        name: String,
        /// Expected type
        expected: &'static str,
        /// Actual type
        got: &'static str,
    },

    /// Alias conflicts with existing name.
    AliasConflict(String),

    /// Scope mismatch (e.g., setting buffer-local for global-only option).
    ScopeMismatch {
        /// Option name
        name: String,
        /// Option's declared scope
        option_scope: OptionScope,
        /// Requested scope
        requested: OptionScopeId,
    },
}

impl fmt::Display for OptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(name) => write!(f, "option not found: {name}"),
            Self::AlreadyExists(name) => write!(f, "option already exists: {name}"),
            Self::ValidationFailed { name, reason } => {
                write!(f, "validation failed for '{name}': {reason}")
            }
            Self::TypeMismatch {
                name,
                expected,
                got,
            } => {
                write!(f, "type mismatch for '{name}': expected {expected}, got {got}")
            }
            Self::AliasConflict(alias) => {
                write!(f, "alias conflicts with existing name: {alias}")
            }
            Self::ScopeMismatch {
                name,
                option_scope,
                requested,
            } => {
                write!(
                    f,
                    "scope mismatch for '{name}': option is {option_scope}, requested {requested}"
                )
            }
        }
    }
}

impl std::error::Error for OptionError {}

/// Result of a successful set operation.
#[derive(Debug, Clone)]
pub struct SetResult {
    /// Previous value (None if was default).
    pub old_value: Option<OptionValue>,
    /// New value that was set.
    pub new_value: OptionValue,
}

