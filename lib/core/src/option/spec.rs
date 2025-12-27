//! Option specification types for the extensible settings system.
//!
//! This module defines the core types for describing editor options:
//! - `OptionValue` - Typed values (bool, int, string, choice)
//! - `OptionSpec` - Complete specification for an option
//! - `OptionConstraint` - Validation constraints
//! - `OptionScope` - Where the option applies (global, buffer, window)
//! - `OptionCategory` - Grouping for UI display

use std::borrow::Cow;

/// Option value types with type safety.
///
/// Each variant represents a different type of option value,
/// enabling type-checked access and validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionValue {
    /// Boolean on/off option (e.g., `number`, `relativenumber`)
    Bool(bool),

    /// Integer option (e.g., `tabwidth`, `scrolloff`)
    Integer(i64),

    /// Free-form string option
    String(String),

    /// Enum-style choice option with predefined values
    /// (e.g., `colormode` with choices `ansi`, `256`, `truecolor`)
    Choice {
        /// Current selected value
        value: String,
        /// List of valid choices
        choices: Vec<String>,
    },
}

impl OptionValue {
    /// Create a boolean option value.
    #[must_use]
    pub const fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    /// Create an integer option value.
    #[must_use]
    pub const fn int(value: i64) -> Self {
        Self::Integer(value)
    }

    /// Create a string option value.
    #[must_use]
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    /// Create a choice option value.
    #[must_use]
    pub fn choice(value: impl Into<String>, choices: Vec<String>) -> Self {
        Self::Choice {
            value: value.into(),
            choices,
        }
    }

    /// Get the value as a boolean, if it is one.
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Get the value as an integer, if it is one.
    #[must_use]
    pub const fn as_int(&self) -> Option<i64> {
        match self {
            Self::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Get the value as a string reference.
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
            Self::Bool(_) => "boolean",
            Self::Integer(_) => "integer",
            Self::String(_) => "string",
            Self::Choice { .. } => "choice",
        }
    }

    /// Check if this value matches the type of another value.
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

impl std::fmt::Display for OptionValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Integer(i) => write!(f, "{i}"),
            Self::String(s) => write!(f, "{s}"),
            Self::Choice { value, .. } => write!(f, "{value}"),
        }
    }
}

/// Category for grouping options in UI.
///
/// Options are organized by category in the settings menu
/// and tab completion.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum OptionCategory {
    /// Core editor options (number, relativenumber, etc.)
    #[default]
    Editor,

    /// Display/rendering options (colormode, theme)
    Display,

    /// Window/split options
    Window,

    /// Cursor options
    Cursor,

    /// Plugin-specific options with plugin ID
    Plugin(Cow<'static, str>),
}

impl OptionCategory {
    /// Create a plugin category.
    #[must_use]
    pub fn plugin(id: impl Into<Cow<'static, str>>) -> Self {
        Self::Plugin(id.into())
    }

    /// Get the display name for this category.
    #[must_use]
    pub fn display_name(&self) -> &str {
        match self {
            Self::Editor => "Editor",
            Self::Display => "Display",
            Self::Window => "Window",
            Self::Cursor => "Cursor",
            Self::Plugin(id) => id,
        }
    }

    /// Check if this is a plugin category.
    #[must_use]
    pub const fn is_plugin(&self) -> bool {
        matches!(self, Self::Plugin(_))
    }

    /// Get the plugin ID if this is a plugin category.
    #[must_use]
    pub fn plugin_id(&self) -> Option<&str> {
        match self {
            Self::Plugin(id) => Some(id),
            _ => None,
        }
    }
}

/// Scope where an option applies.
///
/// Determines whether an option is global or specific to
/// a buffer or window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptionScope {
    /// Applies globally to the entire editor
    #[default]
    Global,

    /// Per-buffer setting (e.g., `filetype`, `tabwidth`)
    Buffer,

    /// Per-window setting (e.g., `number`, `relativenumber`)
    Window,
}

impl OptionScope {
    /// Get the display name for this scope.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Buffer => "buffer",
            Self::Window => "window",
        }
    }
}

/// Constraints for value validation.
///
/// These constraints are checked when setting an option value
/// to ensure it's valid.
#[derive(Debug, Clone, Default)]
pub struct OptionConstraint {
    /// For Integer: minimum value (inclusive)
    pub min: Option<i64>,

    /// For Integer: maximum value (inclusive)
    pub max: Option<i64>,

    /// For String: regex pattern to match
    pub pattern: Option<String>,

    /// For String: minimum length
    pub min_length: Option<usize>,

    /// For String: maximum length
    pub max_length: Option<usize>,
}

impl OptionConstraint {
    /// Create an empty constraint (no validation).
    #[must_use]
    pub const fn none() -> Self {
        Self {
            min: None,
            max: None,
            pattern: None,
            min_length: None,
            max_length: None,
        }
    }

    /// Create an integer range constraint.
    #[must_use]
    pub const fn range(min: i64, max: i64) -> Self {
        Self {
            min: Some(min),
            max: Some(max),
            pattern: None,
            min_length: None,
            max_length: None,
        }
    }

    /// Create a minimum value constraint.
    #[must_use]
    pub const fn min(min: i64) -> Self {
        Self {
            min: Some(min),
            max: None,
            pattern: None,
            min_length: None,
            max_length: None,
        }
    }

    /// Create a maximum value constraint.
    #[must_use]
    pub const fn max(max: i64) -> Self {
        Self {
            min: None,
            max: Some(max),
            pattern: None,
            min_length: None,
            max_length: None,
        }
    }

    /// Validate a value against this constraint.
    ///
    /// Returns `Ok(())` if valid, or an error message if not.
    ///
    /// # Errors
    ///
    /// Returns an error string if the value violates any constraint:
    /// - Integer below minimum or above maximum
    /// - String length below minimum or above maximum
    /// - Choice value not in allowed choices list
    pub fn validate(&self, value: &OptionValue) -> Result<(), String> {
        match value {
            OptionValue::Integer(i) => {
                if let Some(min) = self.min
                    && *i < min
                {
                    return Err(format!("value {i} is below minimum {min}"));
                }
                if let Some(max) = self.max
                    && *i > max
                {
                    return Err(format!("value {i} exceeds maximum {max}"));
                }
            }
            OptionValue::String(s) => {
                if let Some(min_len) = self.min_length
                    && s.len() < min_len
                {
                    return Err(format!("string length {} is below minimum {min_len}", s.len()));
                }
                if let Some(max_len) = self.max_length
                    && s.len() > max_len
                {
                    return Err(format!("string length {} exceeds maximum {max_len}", s.len()));
                }
                if let Some(pattern) = &self.pattern {
                    // Note: regex validation would require the regex crate
                    // For now, we just store the pattern for documentation
                    let _ = pattern;
                }
            }
            OptionValue::Choice { value, choices } => {
                if !choices.contains(value) {
                    return Err(format!(
                        "invalid choice '{value}', expected one of: {}",
                        choices.join(", ")
                    ));
                }
            }
            OptionValue::Bool(_) => {
                // Booleans have no constraints
            }
        }
        Ok(())
    }
}

/// Dependency on another option.
///
/// Options can declare dependencies on other options,
/// which affects their visibility and validity.
#[derive(Debug, Clone)]
pub struct OptionDependency {
    /// The option this depends on
    pub option: Cow<'static, str>,

    /// Type of dependency
    pub kind: DependencyKind,
}

impl OptionDependency {
    /// Create a dependency that requires another option to be enabled.
    #[must_use]
    pub fn requires_enabled(option: impl Into<Cow<'static, str>>) -> Self {
        Self {
            option: option.into(),
            kind: DependencyKind::RequiresEnabled,
        }
    }

    /// Create a dependency that requires a specific value.
    #[must_use]
    pub fn requires_value(option: impl Into<Cow<'static, str>>, value: OptionValue) -> Self {
        Self {
            option: option.into(),
            kind: DependencyKind::RequiresValue(value),
        }
    }

    /// Create a visibility dependency (only shown when other is enabled).
    #[must_use]
    pub fn visible_when(option: impl Into<Cow<'static, str>>) -> Self {
        Self {
            option: option.into(),
            kind: DependencyKind::VisibleWhen,
        }
    }
}

/// Type of dependency between options.
#[derive(Debug, Clone)]
pub enum DependencyKind {
    /// This option requires the other to be enabled (for bools)
    RequiresEnabled,

    /// This option requires the other to have a specific value
    RequiresValue(OptionValue),

    /// This option is only shown when other is enabled
    VisibleWhen,
}

/// Complete specification for an option.
///
/// This struct describes everything about an option:
/// name, type, default value, constraints, and metadata.
#[derive(Debug, Clone)]
pub struct OptionSpec {
    /// Full option name (e.g., "number", "`plugin.treesitter.highlight_timeout_ms`")
    pub name: Cow<'static, str>,

    /// Short alias (e.g., "nu" for "number")
    pub short: Option<Cow<'static, str>>,

    /// Human-readable description
    pub description: Cow<'static, str>,

    /// Option category for grouping in UI
    pub category: OptionCategory,

    /// The default value
    pub default: OptionValue,

    /// Constraints for validation
    pub constraint: OptionConstraint,

    /// Dependencies on other options
    pub depends_on: Vec<OptionDependency>,

    /// Scope (global, buffer-local, window-local)
    pub scope: OptionScope,

    // --- UI Metadata for Settings Menu ---
    /// Section name for settings menu (overrides `category.display_name()`)
    ///
    /// If `None`, falls back to `category.display_name()`.
    pub section: Option<Cow<'static, str>>,

    /// Display order within section (lower = earlier, default 100)
    ///
    /// Options are sorted by this value within their section.
    pub display_order: u32,

    /// Whether to show this option in the settings menu (default true)
    ///
    /// Some options may be `:set`-only and not shown in the menu.
    pub show_in_menu: bool,
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
            short: None,
            description: description.into(),
            category: OptionCategory::default(),
            default,
            constraint: OptionConstraint::default(),
            depends_on: Vec::new(),
            scope: OptionScope::default(),
            section: None,
            display_order: 100,
            show_in_menu: true,
        }
    }

    /// Set the short alias.
    #[must_use]
    pub fn with_short(mut self, short: impl Into<Cow<'static, str>>) -> Self {
        self.short = Some(short.into());
        self
    }

    /// Set the category.
    #[must_use]
    pub fn with_category(mut self, category: OptionCategory) -> Self {
        self.category = category;
        self
    }

    /// Set the constraint.
    #[must_use]
    pub fn with_constraint(mut self, constraint: OptionConstraint) -> Self {
        self.constraint = constraint;
        self
    }

    /// Set the scope.
    #[must_use]
    pub const fn with_scope(mut self, scope: OptionScope) -> Self {
        self.scope = scope;
        self
    }

    /// Add a dependency.
    #[must_use]
    pub fn with_dependency(mut self, dependency: OptionDependency) -> Self {
        self.depends_on.push(dependency);
        self
    }

    /// Set the section name for the settings menu.
    ///
    /// This overrides the default section derived from `category.display_name()`.
    #[must_use]
    pub fn with_section(mut self, section: impl Into<Cow<'static, str>>) -> Self {
        self.section = Some(section.into());
        self
    }

    /// Set the display order within the section.
    ///
    /// Lower values appear first. Default is 100.
    #[must_use]
    pub const fn with_display_order(mut self, order: u32) -> Self {
        self.display_order = order;
        self
    }

    /// Set whether to show this option in the settings menu.
    ///
    /// If `false`, the option is still accessible via `:set` command.
    #[must_use]
    pub const fn with_show_in_menu(mut self, show: bool) -> Self {
        self.show_in_menu = show;
        self
    }

    /// Get the effective section name for the settings menu.
    ///
    /// Returns `section` if set, otherwise falls back to `category.display_name()`.
    #[must_use]
    pub fn effective_section(&self) -> &str {
        self.section
            .as_deref()
            .unwrap_or_else(|| self.category.display_name())
    }

    /// Validate a value against this option's constraints.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The value type doesn't match the option's expected type
    /// - The value fails constraint validation
    pub fn validate(&self, value: &OptionValue) -> Result<(), String> {
        // Check type matches
        if !value.same_type(&self.default) {
            return Err(format!(
                "expected {} but got {}",
                self.default.type_name(),
                value.type_name()
            ));
        }

        // Check constraints
        self.constraint.validate(value)
    }

    /// Check if this option matches a name (full name or short alias).
    #[must_use]
    pub fn matches_name(&self, name: &str) -> bool {
        self.name == name || self.short.as_deref() == Some(name)
    }

    /// Get the plugin ID if this is a plugin option.
    #[must_use]
    pub fn plugin_id(&self) -> Option<&str> {
        self.category.plugin_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option_value_type_name() {
        assert_eq!(OptionValue::Bool(true).type_name(), "boolean");
        assert_eq!(OptionValue::Integer(42).type_name(), "integer");
        assert_eq!(OptionValue::String("test".into()).type_name(), "string");
        assert_eq!(
            OptionValue::Choice {
                value: "a".into(),
                choices: vec!["a".into(), "b".into()]
            }
            .type_name(),
            "choice"
        );
    }

    #[test]
    fn test_constraint_range_validation() {
        let constraint = OptionConstraint::range(1, 10);

        assert!(constraint.validate(&OptionValue::Integer(5)).is_ok());
        assert!(constraint.validate(&OptionValue::Integer(1)).is_ok());
        assert!(constraint.validate(&OptionValue::Integer(10)).is_ok());
        assert!(constraint.validate(&OptionValue::Integer(0)).is_err());
        assert!(constraint.validate(&OptionValue::Integer(11)).is_err());
    }

    #[test]
    fn test_option_spec_validation() {
        let spec = OptionSpec::new("tabwidth", "Tab width in spaces", OptionValue::Integer(4))
            .with_constraint(OptionConstraint::range(1, 8));

        assert!(spec.validate(&OptionValue::Integer(4)).is_ok());
        assert!(spec.validate(&OptionValue::Integer(0)).is_err());
        assert!(spec.validate(&OptionValue::Bool(true)).is_err()); // wrong type
    }

    #[test]
    fn test_choice_validation() {
        let value = OptionValue::Choice {
            value: "dark".into(),
            choices: vec!["dark".into(), "light".into()],
        };

        let constraint = OptionConstraint::none();
        assert!(constraint.validate(&value).is_ok());

        let invalid = OptionValue::Choice {
            value: "invalid".into(),
            choices: vec!["dark".into(), "light".into()],
        };
        assert!(constraint.validate(&invalid).is_err());
    }
}
