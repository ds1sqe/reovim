//! Option specification.

use std::borrow::Cow;

use {
    super::{
        constraint::OptionConstraint, error::OptionError, scope::OptionScope, value::OptionValue,
    },
    crate::api::ModuleId,
};

/// Complete specification for an editor option.
///
/// This is the **MECHANISM** type - it defines the structure of options.
/// **POLICY** (which options exist and what they do) is in modules.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::*;
///
/// let spec = OptionSpec::new("tabwidth", "Tab width in spaces", OptionValue::int(4))
///     .with_short("tw")
///     .with_constraint(OptionConstraint::range(1, 32))
///     .with_scope(OptionScope::Buffer);
/// ```
#[derive(Debug, Clone)]
pub struct OptionSpec {
    /// Full option name (e.g., "number", "tabwidth").
    pub name: Cow<'static, str>,
    /// Short alias (e.g., "nu" for "number").
    pub short_form: Option<Cow<'static, str>>,
    /// Human-readable description.
    pub description: Cow<'static, str>,
    /// Default value (also defines the type).
    pub default: OptionValue,
    /// Constraints for validation.
    pub constraint: OptionConstraint,
    /// Scope (global, buffer-local, window-local).
    pub scope: OptionScope,
    /// Module that registered this option (for lifecycle cleanup).
    pub owner: Option<ModuleId>,
}

impl OptionSpec {
    /// Create a new option specification.
    #[must_use]
    pub fn new(
        name: impl Into<Cow<'static, str>>,
        description: impl Into<Cow<'static, str>>,
        default: OptionValue,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            default,
            short_form: None,
            constraint: OptionConstraint::none(),
            scope: OptionScope::default(),
            owner: None,
        }
    }

    /// Add a short alias.
    #[must_use]
    pub fn with_short(mut self, short: impl Into<Cow<'static, str>>) -> Self {
        self.short_form = Some(short.into());
        self
    }

    /// Add validation constraints.
    #[must_use]
    pub const fn with_constraint(mut self, constraint: OptionConstraint) -> Self {
        self.constraint = constraint;
        self
    }

    /// Set the scope.
    #[must_use]
    pub const fn with_scope(mut self, scope: OptionScope) -> Self {
        self.scope = scope;
        self
    }

    /// Set the owning module.
    #[must_use]
    pub fn with_owner(mut self, owner: ModuleId) -> Self {
        self.owner = Some(owner);
        self
    }

    /// Get the owning module, if set.
    #[must_use]
    pub const fn owner(&self) -> Option<&ModuleId> {
        self.owner.as_ref()
    }

    /// Check if this option matches a name (full name or alias).
    #[must_use]
    pub fn matches_name(&self, query: &str) -> bool {
        self.name == query
            || self
                .short_form
                .as_ref()
                .is_some_and(|s: &Cow<'static, str>| s.as_ref() == query)
    }

    /// Validate a value against this option's type and constraints.
    ///
    /// # Errors
    ///
    /// Returns error if value type doesn't match or violates constraints.
    pub fn validate(&self, value: &OptionValue) -> Result<(), OptionError> {
        // Type check
        if !self.default.same_type(value) {
            return Err(OptionError::TypeMismatch {
                name: self.name.to_string(),
                expected: self.default.type_name(),
                got: value.type_name(),
            });
        }

        // Constraint check
        self.constraint
            .validate(value)
            .map_err(|e| OptionError::ValidationFailed {
                name: self.name.to_string(),
                reason: e.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option_spec_builder() {
        let spec = OptionSpec::new("number", "Show line numbers", OptionValue::bool(false))
            .with_short("nu")
            .with_scope(OptionScope::Window);

        assert_eq!(spec.name, "number");
        assert_eq!(spec.short_form.as_deref(), Some("nu"));
        assert_eq!(spec.scope, OptionScope::Window);
    }

    #[test]
    fn test_option_spec_matches_name() {
        let spec = OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu");

        assert!(spec.matches_name("number"));
        assert!(spec.matches_name("nu"));
        assert!(!spec.matches_name("other"));
    }

    #[test]
    fn test_option_spec_with_owner() {
        let module = crate::api::ModuleId::new("vim");
        let spec = OptionSpec::new("number", "Show line numbers", OptionValue::bool(false))
            .with_owner(module.clone());

        assert_eq!(spec.owner(), Some(&module));
    }

    #[test]
    fn test_option_spec_default_no_owner() {
        let spec = OptionSpec::new("number", "Show line numbers", OptionValue::bool(false));
        assert_eq!(spec.owner(), None);
    }

    #[test]
    fn test_option_spec_validate() {
        let spec = OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4))
            .with_constraint(OptionConstraint::range(1, 32));

        assert!(spec.validate(&OptionValue::int(4)).is_ok());
        assert!(spec.validate(&OptionValue::int(1)).is_ok());
        assert!(spec.validate(&OptionValue::int(32)).is_ok());
        assert!(spec.validate(&OptionValue::int(0)).is_err());
        assert!(spec.validate(&OptionValue::int(33)).is_err());
        assert!(spec.validate(&OptionValue::bool(true)).is_err()); // Type mismatch
    }
}
