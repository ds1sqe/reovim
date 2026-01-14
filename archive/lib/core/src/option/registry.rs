//! Thread-safe option registry for storing and managing options.
//!
//! The `OptionRegistry` is the central storage for all editor options,
//! both built-in and plugin-defined. It provides:
//!
//! - Thread-safe access via `RwLock`
//! - Validation on set operations
//! - Alias resolution (short names to full names)
//! - TOML serialization for profile persistence

use std::{borrow::Cow, collections::HashMap, sync::RwLock};

use super::spec::{OptionCategory, OptionSpec, OptionValue};

/// Error type for option operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionError {
    /// Option not found in registry
    NotFound(String),

    /// Option already registered
    AlreadyExists(String),

    /// Validation failed
    ValidationFailed(String),

    /// Type mismatch
    TypeMismatch {
        expected: &'static str,
        got: &'static str,
    },

    /// Alias conflicts with existing name
    AliasConflict(String),
}

impl std::fmt::Display for OptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(name) => write!(f, "option not found: {name}"),
            Self::AlreadyExists(name) => write!(f, "option already exists: {name}"),
            Self::ValidationFailed(msg) => write!(f, "validation failed: {msg}"),
            Self::TypeMismatch { expected, got } => {
                write!(f, "type mismatch: expected {expected}, got {got}")
            }
            Self::AliasConflict(alias) => write!(f, "alias conflicts with existing name: {alias}"),
        }
    }
}

impl std::error::Error for OptionError {}

/// Thread-safe registry for all editor options.
///
/// This is the central storage for option specifications and values.
/// Plugins register their options here, and the runtime uses it to
/// handle `:set` commands and profile loading.
#[derive(Debug, Default)]
pub struct OptionRegistry {
    /// Registered option specs indexed by full name
    specs: RwLock<HashMap<String, OptionSpec>>,

    /// Current values (overrides from defaults)
    values: RwLock<HashMap<String, OptionValue>>,

    /// Short name -> full name mapping
    aliases: RwLock<HashMap<String, String>>,
}

impl OptionRegistry {
    /// Create a new empty option registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an option specification.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Option with same name already exists
    /// - Short alias conflicts with existing name or alias
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (a thread panicked while holding it).
    #[allow(clippy::needless_pass_by_value)]
    pub fn register(&self, spec: OptionSpec) -> Result<(), OptionError> {
        let name = spec.name.to_string();

        // Check if already exists
        {
            let specs = self.specs.read().unwrap();
            if specs.contains_key(&name) {
                return Err(OptionError::AlreadyExists(name));
            }
        }

        // Check alias conflicts
        if let Some(short) = &spec.short {
            let short_str = short.to_string();
            let specs = self.specs.read().unwrap();
            let aliases = self.aliases.read().unwrap();

            if specs.contains_key(&short_str) || aliases.contains_key(&short_str) {
                return Err(OptionError::AliasConflict(short_str));
            }
        }

        // Register the spec
        {
            let mut specs = self.specs.write().unwrap();
            specs.insert(name.clone(), spec.clone());
        }

        // Register the alias
        if let Some(short) = &spec.short {
            let mut aliases = self.aliases.write().unwrap();
            aliases.insert(short.to_string(), name);
        }

        Ok(())
    }

    /// Register or replace an existing option specification.
    ///
    /// Unlike `register`, this will overwrite an existing option.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned (a thread panicked while holding it).
    #[allow(clippy::needless_pass_by_value)]
    pub fn register_or_replace(&self, spec: OptionSpec) {
        let name = spec.name.to_string();

        // Remove old alias if it exists
        {
            let specs = self.specs.read().unwrap();
            if let Some(old_spec) = specs.get(&name)
                && let Some(old_short) = &old_spec.short
            {
                let mut aliases = self.aliases.write().unwrap();
                aliases.remove(old_short.as_ref());
            }
        }

        // Register new spec
        {
            let mut specs = self.specs.write().unwrap();
            specs.insert(name.clone(), spec.clone());
        }

        // Register new alias
        if let Some(short) = &spec.short {
            let mut aliases = self.aliases.write().unwrap();
            aliases.insert(short.to_string(), name);
        }
    }

    /// Resolve a name (which may be an alias) to the full option name.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn resolve_name(&self, name: &str) -> Option<String> {
        // First check if it's a full name
        {
            let specs = self.specs.read().unwrap();
            if specs.contains_key(name) {
                return Some(name.to_string());
            }
        }

        // Then check aliases
        {
            let aliases = self.aliases.read().unwrap();
            if let Some(full_name) = aliases.get(name) {
                return Some(full_name.clone());
            }
        }

        None
    }

    /// Get an option specification by name (supports aliases).
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn get_spec(&self, name: &str) -> Option<OptionSpec> {
        let full_name = self.resolve_name(name)?;
        let specs = self.specs.read().unwrap();
        specs.get(&full_name).cloned()
    }

    /// Get the current value of an option (or default if not set).
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<OptionValue> {
        let full_name = self.resolve_name(name)?;

        // Check for override value
        {
            let values = self.values.read().unwrap();
            if let Some(value) = values.get(&full_name) {
                return Some(value.clone());
            }
        }

        // Fall back to default
        let specs = self.specs.read().unwrap();
        specs.get(&full_name).map(|s| s.default.clone())
    }

    /// Set an option value with validation.
    ///
    /// Returns the old value if the option existed and had a value set.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Option not found
    /// - Value fails validation
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn set(&self, name: &str, value: OptionValue) -> Result<Option<OptionValue>, OptionError> {
        let full_name = self
            .resolve_name(name)
            .ok_or_else(|| OptionError::NotFound(name.to_string()))?;

        // Get spec for validation
        let spec = {
            let specs = self.specs.read().unwrap();
            specs
                .get(&full_name)
                .cloned()
                .ok_or_else(|| OptionError::NotFound(full_name.clone()))?
        };

        // Validate
        spec.validate(&value)
            .map_err(OptionError::ValidationFailed)?;

        // Set the value
        let old = self.values.write().unwrap().insert(full_name, value);
        Ok(old)
    }

    /// Reset an option to its default value.
    ///
    /// Returns the old value if one was set.
    ///
    /// # Errors
    ///
    /// Returns error if option not found.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn reset(&self, name: &str) -> Result<Option<OptionValue>, OptionError> {
        let full_name = self
            .resolve_name(name)
            .ok_or_else(|| OptionError::NotFound(name.to_string()))?;

        let mut values = self.values.write().unwrap();
        Ok(values.remove(&full_name))
    }

    /// Toggle a boolean option.
    ///
    /// # Errors
    ///
    /// Returns error if option is not a boolean.
    pub fn toggle(&self, name: &str) -> Result<bool, OptionError> {
        let current = self
            .get(name)
            .ok_or_else(|| OptionError::NotFound(name.to_string()))?;

        match current {
            OptionValue::Bool(b) => {
                let new_value = !b;
                self.set(name, OptionValue::Bool(new_value))?;
                Ok(new_value)
            }
            other => Err(OptionError::TypeMismatch {
                expected: "boolean",
                got: other.type_name(),
            }),
        }
    }

    /// List all registered option names.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn list_all(&self) -> Vec<String> {
        let specs = self.specs.read().unwrap();
        specs.keys().cloned().collect()
    }

    /// List options matching a prefix (for tab completion).
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    #[allow(clippy::significant_drop_tightening)]
    pub fn list_matching(&self, prefix: &str) -> Vec<OptionSpec> {
        let specs = self.specs.read().unwrap();
        let aliases = self.aliases.read().unwrap();

        let mut results: Vec<OptionSpec> = specs
            .iter()
            .filter(|(name, spec)| {
                // Match full name
                name.starts_with(prefix)
                    // Or match short alias
                    || spec.short.as_ref().is_some_and(|s| s.starts_with(prefix))
            })
            .map(|(_, spec)| spec.clone())
            .collect();

        // Also check aliases that match but weren't covered
        for (alias, full_name) in aliases.iter() {
            if alias.starts_with(prefix)
                && !full_name.starts_with(prefix)
                && let Some(spec) = specs.get(full_name)
                && !results.iter().any(|r| r.name == spec.name)
            {
                results.push(spec.clone());
            }
        }

        // Sort by name for consistent ordering
        results.sort_by(|a, b| a.name.cmp(&b.name));
        results
    }

    /// List options by category.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn list_by_category(&self, category: &OptionCategory) -> Vec<OptionSpec> {
        let specs = self.specs.read().unwrap();
        specs
            .values()
            .filter(|spec| &spec.category == category)
            .cloned()
            .collect()
    }

    /// List all plugin IDs that have registered options.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn list_plugin_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .specs
            .read()
            .unwrap()
            .values()
            .filter_map(|spec| spec.plugin_id().map(String::from))
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Export plugin options to TOML-compatible format.
    ///
    /// Returns a map of option names (without plugin prefix) to their values.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    #[allow(clippy::significant_drop_tightening)]
    pub fn export_plugin_options(&self, plugin_id: &str) -> HashMap<String, toml::Value> {
        let specs = self.specs.read().unwrap();
        let values = self.values.read().unwrap();

        let prefix = format!("plugin.{plugin_id}.");
        let mut result = HashMap::new();

        for (name, spec) in specs.iter() {
            if let Some(stripped) = name.strip_prefix(&prefix) {
                // Get current value or default
                let value = values.get(name).unwrap_or(&spec.default);
                result.insert(stripped.to_string(), option_value_to_toml(value));
            }
        }

        result
    }

    /// Import plugin options from TOML values.
    ///
    /// The `values` map should contain option names without the plugin prefix.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    pub fn import_plugin_options(
        &self,
        plugin_id: &str,
        values: &HashMap<String, toml::Value>,
    ) -> Vec<OptionError> {
        let prefix = format!("plugin.{plugin_id}.");
        let mut errors = Vec::new();

        for (name, toml_value) in values {
            let full_name = format!("{prefix}{name}");

            // Get spec for this option
            let Some(spec) = self.get_spec(&full_name) else {
                errors.push(OptionError::NotFound(full_name));
                continue;
            };

            // Convert TOML value to OptionValue
            let Some(option_value) = toml_to_option_value(toml_value, &spec.default) else {
                errors.push(OptionError::ValidationFailed(format!(
                    "cannot convert TOML value for {full_name}"
                )));
                continue;
            };

            // Set the value
            if let Err(e) = self.set(&full_name, option_value) {
                errors.push(e);
            }
        }

        errors
    }

    /// Check if an option exists.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.resolve_name(name).is_some()
    }

    /// Get the number of registered options.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.specs.read().unwrap().len()
    }

    /// Check if the registry is empty.
    ///
    /// # Panics
    ///
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.specs.read().unwrap().is_empty()
    }
}

/// Convert an `OptionValue` to a TOML value.
fn option_value_to_toml(value: &OptionValue) -> toml::Value {
    match value {
        OptionValue::Bool(b) => toml::Value::Boolean(*b),
        OptionValue::Integer(i) => toml::Value::Integer(*i),
        OptionValue::String(s) => toml::Value::String(s.clone()),
        OptionValue::Choice { value, .. } => toml::Value::String(value.clone()),
    }
}

/// Convert a TOML value to an `OptionValue`, using the default for type hints.
fn toml_to_option_value(toml: &toml::Value, default: &OptionValue) -> Option<OptionValue> {
    match (toml, default) {
        (toml::Value::Boolean(b), OptionValue::Bool(_)) => Some(OptionValue::Bool(*b)),
        (toml::Value::Integer(i), OptionValue::Integer(_)) => Some(OptionValue::Integer(*i)),
        (toml::Value::String(s), OptionValue::String(_)) => Some(OptionValue::String(s.clone())),
        (toml::Value::String(s), OptionValue::Choice { choices, .. }) => {
            Some(OptionValue::Choice {
                value: s.clone(),
                choices: choices.clone(),
            })
        }
        _ => None,
    }
}

/// Builder for creating option specifications with a fluent API.
///
/// This is used by `PluginContext` to provide a convenient way
/// for plugins to register options.
pub struct OptionBuilder<F>
where
    F: FnOnce(OptionSpec) -> Result<(), OptionError>,
{
    spec: OptionSpec,
    register_fn: F,
}

impl<F> OptionBuilder<F>
where
    F: FnOnce(OptionSpec) -> Result<(), OptionError>,
{
    /// Create a new option builder.
    pub fn new(
        name: impl Into<Cow<'static, str>>,
        plugin_id: impl Into<Cow<'static, str>>,
        register_fn: F,
    ) -> Self {
        let plugin_id = plugin_id.into();
        let name = name.into();
        let full_name: Cow<'static, str> = format!("plugin.{plugin_id}.{name}").into();

        Self {
            spec: OptionSpec {
                name: full_name,
                short: None,
                description: Cow::Borrowed(""),
                category: OptionCategory::Plugin(plugin_id),
                default: OptionValue::Bool(false), // Placeholder
                constraint: super::spec::OptionConstraint::none(),
                depends_on: Vec::new(),
                scope: super::spec::OptionScope::Global,
                section: None,
                display_order: 100,
                show_in_menu: true,
            },
            register_fn,
        }
    }

    /// Set the short alias for this option.
    #[must_use]
    pub fn short(mut self, alias: impl Into<Cow<'static, str>>) -> Self {
        self.spec.short = Some(alias.into());
        self
    }

    /// Set the description for this option.
    #[must_use]
    pub fn description(mut self, desc: impl Into<Cow<'static, str>>) -> Self {
        self.spec.description = desc.into();
        self
    }

    /// Set the default value to a boolean.
    #[must_use]
    pub fn default_bool(mut self, value: bool) -> Self {
        self.spec.default = OptionValue::Bool(value);
        self
    }

    /// Set the default value to an integer.
    #[must_use]
    pub fn default_int(mut self, value: i64) -> Self {
        self.spec.default = OptionValue::Integer(value);
        self
    }

    /// Set the default value to a string.
    #[must_use]
    pub fn default_string(mut self, value: impl Into<String>) -> Self {
        self.spec.default = OptionValue::String(value.into());
        self
    }

    /// Set the default value to a choice.
    #[must_use]
    pub fn default_choice(mut self, value: impl Into<String>, choices: &[&str]) -> Self {
        self.spec.default = OptionValue::Choice {
            value: value.into(),
            choices: choices.iter().map(|s| (*s).to_string()).collect(),
        };
        self
    }

    /// Set a minimum value constraint (for integers).
    #[must_use]
    pub const fn min(mut self, value: i64) -> Self {
        self.spec.constraint.min = Some(value);
        self
    }

    /// Set a maximum value constraint (for integers).
    #[must_use]
    pub const fn max(mut self, value: i64) -> Self {
        self.spec.constraint.max = Some(value);
        self
    }

    /// Set the scope for this option.
    #[must_use]
    pub const fn scope(mut self, scope: super::spec::OptionScope) -> Self {
        self.spec.scope = scope;
        self
    }

    /// Register the option with the registry.
    ///
    /// # Errors
    ///
    /// Returns error if registration fails.
    pub fn register(self) -> Result<(), OptionError> {
        (self.register_fn)(self.spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let registry = OptionRegistry::new();

        let spec = OptionSpec::new("test.option", "A test option", OptionValue::Bool(true));
        registry.register(spec).unwrap();

        assert!(registry.contains("test.option"));
        assert_eq!(registry.get("test.option"), Some(OptionValue::Bool(true)));
    }

    #[test]
    fn test_alias_resolution() {
        let registry = OptionRegistry::new();

        let spec = OptionSpec::new("number", "Show line numbers", OptionValue::Bool(true))
            .with_short("nu");
        registry.register(spec).unwrap();

        assert_eq!(registry.resolve_name("number"), Some("number".to_string()));
        assert_eq!(registry.resolve_name("nu"), Some("number".to_string()));
        assert_eq!(registry.resolve_name("unknown"), None);
    }

    #[test]
    fn test_set_and_get() {
        let registry = OptionRegistry::new();

        let spec = OptionSpec::new("tabwidth", "Tab width", OptionValue::Integer(4))
            .with_constraint(super::super::spec::OptionConstraint::range(1, 8));
        registry.register(spec).unwrap();

        // Set valid value
        registry.set("tabwidth", OptionValue::Integer(2)).unwrap();
        assert_eq!(registry.get("tabwidth"), Some(OptionValue::Integer(2)));

        // Set invalid value
        assert!(registry.set("tabwidth", OptionValue::Integer(10)).is_err());
    }

    #[test]
    fn test_toggle() {
        let registry = OptionRegistry::new();

        let spec = OptionSpec::new("enabled", "Enable feature", OptionValue::Bool(false));
        registry.register(spec).unwrap();

        assert_eq!(registry.toggle("enabled"), Ok(true));
        assert_eq!(registry.get("enabled"), Some(OptionValue::Bool(true)));

        assert_eq!(registry.toggle("enabled"), Ok(false));
        assert_eq!(registry.get("enabled"), Some(OptionValue::Bool(false)));
    }

    #[test]
    fn test_list_matching() {
        let registry = OptionRegistry::new();

        registry
            .register(OptionSpec::new("plugin.treesitter.highlight", "", OptionValue::Bool(true)))
            .unwrap();
        registry
            .register(OptionSpec::new("plugin.treesitter.timeout", "", OptionValue::Integer(100)))
            .unwrap();
        registry
            .register(OptionSpec::new("plugin.fold.enabled", "", OptionValue::Bool(true)))
            .unwrap();

        let matches = registry.list_matching("plugin.treesitter");
        assert_eq!(matches.len(), 2);

        let all = registry.list_matching("plugin");
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_reset() {
        let registry = OptionRegistry::new();

        let spec = OptionSpec::new("option", "Test", OptionValue::Integer(42));
        registry.register(spec).unwrap();

        registry.set("option", OptionValue::Integer(100)).unwrap();
        assert_eq!(registry.get("option"), Some(OptionValue::Integer(100)));

        registry.reset("option").unwrap();
        assert_eq!(registry.get("option"), Some(OptionValue::Integer(42)));
    }
}
