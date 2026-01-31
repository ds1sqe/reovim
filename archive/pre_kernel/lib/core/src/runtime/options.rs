//! Dynamic option handler methods for the Runtime
//!
//! This module contains handlers for dynamic option manipulation via :set commands:
//! - Enable/disable boolean options
//! - Set option values with type validation
//! - Query current option values
//! - Reset options to default values

use crate::option::{ChangeSource, OptionChanged, OptionValue};

use super::Runtime;

impl Runtime {
    /// Handle dynamic option enable (`:set optionname`)
    pub(crate) fn handle_dynamic_option_enable(&self, name: &str) {
        if let Some(old_value) = self.option_registry.get(name) {
            let new_value = OptionValue::Bool(true);
            match self.option_registry.set(name, new_value.clone()) {
                Ok(_) => {
                    tracing::info!(option = %name, "Option enabled");
                    // Emit change event
                    self.emit_event(OptionChanged::new(
                        name,
                        old_value,
                        new_value,
                        ChangeSource::UserCommand,
                    ));
                }
                Err(e) => {
                    tracing::warn!(option = %name, error = %e, "Failed to enable option");
                }
            }
        } else {
            tracing::warn!(option = %name, "E518: Unknown option");
        }
    }

    /// Handle dynamic option disable (`:set nooptionname`)
    pub(crate) fn handle_dynamic_option_disable(&self, name: &str) {
        if let Some(old_value) = self.option_registry.get(name) {
            let new_value = OptionValue::Bool(false);
            match self.option_registry.set(name, new_value.clone()) {
                Ok(_) => {
                    tracing::info!(option = %name, "Option disabled");
                    // Emit change event
                    self.emit_event(OptionChanged::new(
                        name,
                        old_value,
                        new_value,
                        ChangeSource::UserCommand,
                    ));
                }
                Err(e) => {
                    tracing::warn!(option = %name, error = %e, "Failed to disable option");
                }
            }
        } else {
            tracing::warn!(option = %name, "E518: Unknown option");
        }
    }

    /// Handle dynamic option set (`:set optionname=value`)
    pub(crate) fn handle_dynamic_option_set(&self, name: &str, value_str: &str) {
        // Get the spec to determine the expected type
        let Some(spec) = self.option_registry.get_spec(name) else {
            tracing::warn!(option = %name, "E518: Unknown option");
            return;
        };

        // Parse value based on expected type
        let new_value = match &spec.default {
            OptionValue::Bool(_) => {
                // For booleans, accept "true"/"false", "1"/"0", "yes"/"no"
                match value_str.to_lowercase().as_str() {
                    "true" | "1" | "yes" | "on" => OptionValue::Bool(true),
                    "false" | "0" | "no" | "off" => OptionValue::Bool(false),
                    _ => {
                        tracing::warn!(option = %name, value = %value_str, "E474: Invalid boolean argument");
                        return;
                    }
                }
            }
            OptionValue::Integer(_) => {
                if let Ok(i) = value_str.parse::<i64>() {
                    OptionValue::Integer(i)
                } else {
                    tracing::warn!(option = %name, value = %value_str, "E521: Number required");
                    return;
                }
            }
            OptionValue::String(_) => OptionValue::String(value_str.to_string()),
            OptionValue::Choice { choices, .. } => {
                if choices.contains(&value_str.to_string()) {
                    OptionValue::Choice {
                        value: value_str.to_string(),
                        choices: choices.clone(),
                    }
                } else {
                    tracing::warn!(
                        option = %name,
                        value = %value_str,
                        valid = %choices.join(", "),
                        "E474: Invalid choice"
                    );
                    return;
                }
            }
        };

        // Get old value for change event
        let old_value = self
            .option_registry
            .get(name)
            .unwrap_or_else(|| spec.default.clone());

        // Set the value
        match self.option_registry.set(name, new_value.clone()) {
            Ok(_) => {
                tracing::info!(option = %name, value = %value_str, "Option set");
                // Emit change event
                self.emit_event(OptionChanged::new(
                    name,
                    old_value,
                    new_value,
                    ChangeSource::UserCommand,
                ));
            }
            Err(e) => {
                tracing::warn!(option = %name, value = %value_str, error = %e, "E474: Invalid value");
            }
        }
    }

    /// Handle dynamic option query (`:set optionname?`)
    pub(crate) fn handle_dynamic_option_query(&self, name: &str) {
        if let Some(value) = self.option_registry.get(name) {
            tracing::info!(option = %name, value = %value, "Option value: {name}={value}");
        } else {
            tracing::warn!(option = %name, "E518: Unknown option");
        }
    }

    /// Handle dynamic option reset (`:set optionname&`)
    pub(crate) fn handle_dynamic_option_reset(&self, name: &str) {
        // Get the spec to find the default value
        let Some(spec) = self.option_registry.get_spec(name) else {
            tracing::warn!(option = %name, "E518: Unknown option");
            return;
        };

        let old_value = self
            .option_registry
            .get(name)
            .unwrap_or_else(|| spec.default.clone());
        let default_value = spec.default;

        match self.option_registry.reset(name) {
            Ok(_) => {
                tracing::info!(option = %name, "Option reset to default");
                // Emit change event
                self.emit_event(OptionChanged::new(
                    name,
                    old_value,
                    default_value,
                    ChangeSource::UserCommand,
                ));
            }
            Err(e) => {
                tracing::warn!(option = %name, error = %e, "Failed to reset option");
            }
        }
    }
}
