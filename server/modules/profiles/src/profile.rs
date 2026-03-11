//! Profile data model and TOML serialization.

use {
    reovim_kernel::api::v1::{OptionRegistry, OptionValue},
    serde::{Deserialize, Serialize},
    std::collections::BTreeMap,
};

/// A named configuration profile.
///
/// Captures global option overrides (values that differ from their registered
/// defaults) as a versioned TOML document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Profile {
    /// Schema version for forward compatibility.
    pub metadata: ProfileMetadata,
    /// Option overrides (name -> value). Only non-default values are stored.
    pub options: BTreeMap<String, ProfileOption>,
}

/// Profile metadata section.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileMetadata {
    /// Schema version (currently 1).
    pub version: u32,
}

/// A single option value in a profile.
///
/// Uses explicit type tags to avoid TOML ambiguity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum ProfileOption {
    /// Boolean option.
    #[serde(rename = "bool")]
    Bool {
        /// The value.
        value: bool,
    },
    /// Integer option.
    #[serde(rename = "integer")]
    Integer {
        /// The value.
        value: i64,
    },
    /// String option.
    #[serde(rename = "string")]
    Str {
        /// The value.
        value: String,
    },
    /// Choice option with predefined values.
    #[serde(rename = "choice")]
    Choice {
        /// The selected value.
        value: String,
        /// Valid choices (display context only; registry spec is authoritative on load).
        choices: Vec<String>,
    },
}

impl Profile {
    /// Create a new empty profile.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            metadata: ProfileMetadata { version: 1 },
            options: BTreeMap::new(),
        }
    }

    /// Snapshot current global option overrides from the registry.
    ///
    /// Only captures options whose effective global value differs from
    /// the registered default.
    #[must_use]
    pub fn from_option_registry(registry: &OptionRegistry) -> Self {
        let mut profile = Self::new();

        for name in registry.list_all() {
            if let Some((spec, current)) = registry.get_spec(&name).zip(registry.get_global(&name))
                && current != spec.default
            {
                profile
                    .options
                    .insert(name, ProfileOption::from_option_value(&current));
            }
        }

        profile
    }

    /// Apply this profile's options to the registry.
    ///
    /// Sets each option via `set_global()`. Unknown or invalid options
    /// are skipped with warnings (never aborts mid-load).
    ///
    /// Returns a list of warning messages for skipped options.
    pub fn apply_to_registry(&self, registry: &OptionRegistry) -> Vec<String> {
        let mut warnings = Vec::new();

        for (name, opt) in &self.options {
            let value = opt.to_option_value();

            if registry.get_spec(name).is_none() {
                warnings.push(format!("skipped unknown option '{name}'"));
                continue;
            }

            if let Err(e) = registry.set_global(name, value) {
                warnings.push(format!("failed to set '{name}': {e}"));
            }
        }

        warnings
    }

    /// Serialize this profile to a TOML string.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| format!("TOML serialization failed: {e}"))
    }

    /// Deserialize a profile from a TOML string.
    ///
    /// # Errors
    ///
    /// Returns an error if the TOML is invalid or has an unsupported version.
    pub fn from_toml(content: &str) -> Result<Self, String> {
        let profile: Self =
            toml::from_str(content).map_err(|e| format!("TOML parse failed: {e}"))?;

        if profile.metadata.version != 1 {
            return Err(format!(
                "unsupported profile version {} (expected 1)",
                profile.metadata.version
            ));
        }

        Ok(profile)
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileOption {
    /// Convert from kernel `OptionValue`.
    #[must_use]
    pub fn from_option_value(value: &OptionValue) -> Self {
        match value {
            OptionValue::Bool(b) => Self::Bool { value: *b },
            OptionValue::Integer(i) => Self::Integer { value: *i },
            OptionValue::String(s) => Self::Str { value: s.clone() },
            OptionValue::Choice { value, choices } => Self::Choice {
                value: value.clone(),
                choices: choices.clone(),
            },
        }
    }

    /// Convert to kernel `OptionValue`.
    #[must_use]
    pub fn to_option_value(&self) -> OptionValue {
        match self {
            Self::Bool { value } => OptionValue::Bool(*value),
            Self::Integer { value } => OptionValue::Integer(*value),
            Self::Str { value } => OptionValue::String(value.clone()),
            Self::Choice { value, choices } => OptionValue::Choice {
                value: value.clone(),
                choices: choices.clone(),
            },
        }
    }
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
