#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Personality manifest driver for TOML-based keybinding, mode bridge, and option declarations.
//!
//! This driver provides the mechanism for parsing personality manifests (e.g., `vim.toml`)
//! that declare keybinding tables, mode bridges, and option specifications. It converts
//! manifest data into kernel registration types.
//!
//! # Architecture
//!
//! This is a **driver** (mechanism layer). It provides:
//! - TOML parsing for personality manifests
//! - Conversion from manifest data to kernel registration types
//! - Option spec conversion to kernel [`OptionSpec`] types (#610)
//! - A shared `ModeBridgeStore` service for feature modules to query
//!
//! Policy decisions (which bindings, which modes, which options) live in the personality
//! module (e.g., `vim`) and its TOML data file.

use std::fmt;

use {
    reovim_kernel::api::v1::{
        CommandId, KeybindingRegistration, ModuleId, OptionConstraint, OptionScope, OptionSpec,
        OptionValue, Service,
    },
    serde::Deserialize,
};

// ============================================================================
// Manifest Types
// ============================================================================

/// A single keybinding declared in a personality manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct ManifestKeybinding {
    /// Key sequence in vim notation (e.g., `"<C-y>"`, `"gd"`, `"<Space>f"`).
    pub key: String,
    /// Qualified command ID (e.g., `"completion:confirm"`, `"lsp-navigation:goto-definition"`).
    pub command: String,
    /// Modes where the binding is active (e.g., `["vim:normal"]`, `["vim:insert"]`).
    pub modes: Vec<String>,
    /// Category for which-key grouping (e.g., `"completion"`, `"lsp"`, `"picker"`).
    pub category: String,
    /// Human-readable description (e.g., `"Confirm completion"`).
    pub description: String,
}

/// A mode bridge declaration linking a feature mode to a personality parent mode.
#[derive(Debug, Clone, Deserialize)]
pub struct ManifestModeBridge {
    /// Feature mode in `"module:mode"` format (e.g., `"snippet:navigating"`).
    pub feature_mode: String,
    /// Parent mode in `"module:mode"` format (e.g., `"vim:insert"`).
    pub parent_mode: String,
}

/// Scope where a manifest option applies.
///
/// Defaults to `Global` if not specified in the manifest.
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ManifestOptionScope {
    /// Applies globally to the entire editor.
    #[default]
    Global,
    /// Per-buffer setting (e.g., `filetype`, `tabwidth`).
    Buffer,
    /// Per-window setting (e.g., `number`, `relativenumber`).
    Window,
}

/// Type-safe option value in a manifest.
///
/// Serde deserializes TOML values by type inference:
/// - `true`/`false` → `Bool`
/// - integers → `Integer`
/// - strings → `String`
/// - `{ value = "x", choices = ["x", "y"] }` → `Choice`
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum ManifestOptionValue {
    /// Boolean option value.
    Bool(bool),
    /// Integer option value.
    Integer(i64),
    /// Choice option with predefined valid values.
    Choice {
        /// Current selected value.
        value: String,
        /// All valid choices.
        choices: Vec<String>,
    },
    /// String option value.
    String(String),
}

/// Constraint for a manifest option value.
///
/// Maps directly to kernel [`OptionConstraint`] fields.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ManifestConstraint {
    /// For Integer: minimum value (inclusive).
    #[serde(default)]
    pub min: Option<i64>,
    /// For Integer: maximum value (inclusive).
    #[serde(default)]
    pub max: Option<i64>,
    /// For String: minimum length.
    #[serde(default)]
    pub min_length: Option<usize>,
    /// For String: maximum length.
    #[serde(default)]
    pub max_length: Option<usize>,
}

/// A single option specification declared in a personality manifest.
///
/// Format mirrors kernel [`OptionSpec`] exactly (#610): same fields for name,
/// scope, default, alias, description, and constraint.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ManifestOptionSpec {
    /// Full option name (e.g., `"scrolloff"`, `"number"`).
    pub name: String,
    /// Short alias (e.g., `"so"` for `"scrolloff"`).
    #[serde(default)]
    pub short: Option<String>,
    /// Human-readable description.
    pub description: String,
    /// Scope (global, buffer, window). Defaults to global.
    #[serde(default)]
    pub scope: ManifestOptionScope,
    /// Default value (also defines the type).
    pub default: ManifestOptionValue,
    /// Constraints for validation.
    #[serde(default)]
    pub constraint: ManifestConstraint,
}

impl ManifestOptionSpec {
    /// Convert to a kernel [`OptionSpec`].
    ///
    /// String fields are leaked to `&'static str` (bounded startup cost).
    #[must_use]
    pub fn to_option_spec(&self, owner: &ModuleId) -> OptionSpec {
        let name = leak_str(&self.name);
        let description = leak_str(&self.description);

        let default = match &self.default {
            ManifestOptionValue::Bool(b) => OptionValue::bool(*b),
            ManifestOptionValue::Integer(i) => OptionValue::int(*i),
            ManifestOptionValue::String(s) => OptionValue::string(s.clone()),
            ManifestOptionValue::Choice { value, choices } => {
                OptionValue::choice(value.clone(), choices.clone())
            }
        };

        let constraint = OptionConstraint {
            min: self.constraint.min,
            max: self.constraint.max,
            min_length: self.constraint.min_length,
            max_length: self.constraint.max_length,
        };

        let scope = match self.scope {
            ManifestOptionScope::Global => OptionScope::Global,
            ManifestOptionScope::Buffer => OptionScope::Buffer,
            ManifestOptionScope::Window => OptionScope::Window,
        };

        let mut spec = OptionSpec::new(name, description, default)
            .with_constraint(constraint)
            .with_scope(scope)
            .with_owner(owner.clone());

        if let Some(ref short) = self.short {
            spec = spec.with_short(leak_str(short));
        }

        spec
    }
}

/// Personality manifest metadata.
#[derive(Debug, Clone, Deserialize)]
pub struct PersonalityMeta {
    /// Personality name (e.g., `"vim"`).
    pub name: String,
}

/// Top-level personality manifest parsed from TOML.
///
/// A personality manifest declares keybinding tables and mode bridges for an
/// editor personality (e.g., vim, emacs). The manifest is parsed from TOML and
/// converted into kernel registration types.
#[derive(Debug, Clone, Deserialize)]
pub struct PersonalityManifest {
    /// Personality metadata.
    pub personality: PersonalityMeta,
    /// Keybinding declarations.
    #[serde(default, rename = "keybinding")]
    pub keybindings: Vec<ManifestKeybinding>,
    /// Mode bridge declarations.
    #[serde(default, rename = "mode-bridge")]
    pub mode_bridges: Vec<ManifestModeBridge>,
    /// Option declarations (#610).
    #[serde(default, rename = "option")]
    pub options: Vec<ManifestOptionSpec>,
}

// ============================================================================
// Error Types
// ============================================================================

/// Error parsing a personality manifest.
#[derive(Debug)]
pub enum ManifestError {
    /// TOML syntax or deserialization error.
    Parse(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(msg) => write!(f, "manifest parse error: {msg}"),
        }
    }
}

impl std::error::Error for ManifestError {}

/// Non-fatal warning from manifest validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestWarning {
    /// A mode referenced in a keybinding was not found.
    ModeNotFound {
        /// The binding key that references the missing mode.
        key: String,
        /// The mode string that was not found.
        mode: String,
    },
    /// A command's module is not loaded.
    ModuleNotLoaded {
        /// The binding key that references the unloaded module's command.
        key: String,
        /// The qualified command string.
        command: String,
        /// The module name extracted from the command.
        module: String,
    },
    /// Two bindings share the same key in the same mode.
    KeyConflict {
        /// The conflicting key.
        key: String,
        /// The conflicting mode.
        mode: String,
        /// The first command.
        command_a: String,
        /// The second command.
        command_b: String,
    },
}

impl fmt::Display for ManifestWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModeNotFound { key, mode } => {
                write!(f, "keybinding '{key}': mode '{mode}' not found")
            }
            Self::ModuleNotLoaded {
                key,
                command,
                module,
            } => {
                write!(
                    f,
                    "keybinding '{key}': command '{command}' references unloaded module '{module}'"
                )
            }
            Self::KeyConflict {
                key,
                mode,
                command_a,
                command_b,
            } => {
                write!(
                    f,
                    "key conflict: '{key}' in mode '{mode}' bound to both '{command_a}' and '{command_b}'"
                )
            }
        }
    }
}

// ============================================================================
// Parsing & Conversion
// ============================================================================

/// Leak a string to produce a `&'static str`.
///
/// This is used for converting runtime TOML strings into the `&'static str`
/// required by [`KeybindingRegistration`]. Called once at startup with bounded
/// data (typically ~20 bindings x ~5 fields = ~100 small allocations).
fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

/// Convert a single manifest keybinding to a kernel registration.
///
/// Leaks all string fields to `&'static str` (bounded startup cost).
fn convert_keybinding(kb: &ManifestKeybinding) -> KeybindingRegistration {
    let keys = leak_str(&kb.key);
    let command_id = CommandId::from_qualified_leaked(kb.command.clone());
    let description = leak_str(&kb.description);
    let category = leak_str(&kb.category);

    let modes_vec: Vec<&'static str> = kb.modes.iter().map(|m| leak_str(m)).collect();
    let modes: &'static [&'static str] = Box::leak(modes_vec.into_boxed_slice());

    KeybindingRegistration::new(keys, command_id)
        .with_modes(modes)
        .with_description(description)
        .with_category(category)
}

impl PersonalityManifest {
    /// Parse a personality manifest from TOML text.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::Parse`] if the TOML is invalid or missing
    /// required fields.
    pub fn parse(toml_str: &str) -> Result<Self, ManifestError> {
        toml::from_str(toml_str).map_err(|e| ManifestError::Parse(e.to_string()))
    }

    /// Convert manifest keybindings into kernel registration types.
    ///
    /// All string fields are leaked to `&'static str` since [`KeybindingRegistration`]
    /// requires static lifetimes. This is bounded startup cost.
    #[must_use]
    pub fn to_keybinding_registrations(&self) -> Vec<KeybindingRegistration> {
        self.keybindings.iter().map(convert_keybinding).collect()
    }

    /// Validate that all modes referenced in keybindings are known.
    ///
    /// Accepts a slice of `(module, name)` pairs representing known modes.
    /// Returns warnings for any mode not found.
    #[must_use]
    pub fn validate_modes(&self, known_modes: &[(&str, &str)]) -> Vec<ManifestWarning> {
        let mut warnings = Vec::new();
        for kb in &self.keybindings {
            for mode in &kb.modes {
                if let Some((module, name)) = mode.split_once(':') {
                    if !known_modes.iter().any(|(m, n)| *m == module && *n == name) {
                        warnings.push(ManifestWarning::ModeNotFound {
                            key: kb.key.clone(),
                            mode: mode.clone(),
                        });
                    }
                } else {
                    warnings.push(ManifestWarning::ModeNotFound {
                        key: kb.key.clone(),
                        mode: mode.clone(),
                    });
                }
            }
        }
        warnings
    }

    /// Validate that all command modules are in the loaded modules list.
    ///
    /// Returns warnings for commands whose module is not loaded (graceful
    /// degradation — these bindings should be skipped during registration).
    #[must_use]
    pub fn validate_commands(&self, loaded_modules: &[ModuleId]) -> Vec<ManifestWarning> {
        let mut warnings = Vec::new();
        for kb in &self.keybindings {
            let module_name = kb
                .command
                .split_once(':')
                .map_or(kb.command.as_str(), |(m, _)| m);
            if !loaded_modules.iter().any(|mid| mid.as_str() == module_name) {
                warnings.push(ManifestWarning::ModuleNotLoaded {
                    key: kb.key.clone(),
                    command: kb.command.clone(),
                    module: module_name.to_string(),
                });
            }
        }
        warnings
    }

    /// Detect key conflicts within the manifest (same key + same mode).
    #[must_use]
    pub fn detect_conflicts(&self) -> Vec<ManifestWarning> {
        let mut warnings = Vec::new();
        for (i, a) in self.keybindings.iter().enumerate() {
            for b in &self.keybindings[i + 1..] {
                if a.key == b.key {
                    for mode in &a.modes {
                        if b.modes.contains(mode) {
                            warnings.push(ManifestWarning::KeyConflict {
                                key: a.key.clone(),
                                mode: mode.clone(),
                                command_a: a.command.clone(),
                                command_b: b.command.clone(),
                            });
                        }
                    }
                }
            }
        }
        warnings
    }

    /// Convert manifest option declarations into kernel [`OptionSpec`] values.
    ///
    /// All string fields are leaked to `&'static str` since [`OptionSpec`]
    /// uses `Cow<'static, str>`. This is bounded startup cost (#610).
    #[must_use]
    pub fn to_option_specs(&self, owner: &ModuleId) -> Vec<OptionSpec> {
        self.options
            .iter()
            .map(|opt| opt.to_option_spec(owner))
            .collect()
    }

    /// Filter keybindings to only those whose command module is loaded.
    ///
    /// Returns registrations for bindings whose module is present in
    /// `loaded_modules`, skipping the rest.
    #[must_use]
    pub fn to_filtered_registrations(
        &self,
        loaded_modules: &[ModuleId],
    ) -> Vec<KeybindingRegistration> {
        self.keybindings
            .iter()
            .filter(|kb| {
                let module_name = kb
                    .command
                    .split_once(':')
                    .map_or(kb.command.as_str(), |(m, _)| m);
                loaded_modules.iter().any(|mid| mid.as_str() == module_name)
            })
            .map(convert_keybinding)
            .collect()
    }
}

// ============================================================================
// ModeBridgeStore
// ============================================================================

/// Shared store for mode bridge declarations.
///
/// Populated by the personality module (e.g., `VimModule`) during init.
/// Feature modules (e.g., `range-finder`, `snippet`) query this store to
/// discover their parent mode without depending on the personality module.
///
/// Registered in [`ServiceRegistry`] as a unique service.
pub struct ModeBridgeStore {
    bridges: Vec<ManifestModeBridge>,
}

impl ModeBridgeStore {
    /// Create a new store from manifest mode bridges.
    #[must_use]
    pub const fn new(bridges: Vec<ManifestModeBridge>) -> Self {
        Self { bridges }
    }

    /// Find the parent mode for a given feature mode.
    ///
    /// Returns the parent mode string (e.g., `"vim:insert"`) if a bridge
    /// exists for the given `feature_mode` (e.g., `"snippet:navigating"`).
    #[must_use]
    pub fn find_parent(&self, feature_mode: &str) -> Option<&str> {
        self.bridges
            .iter()
            .find(|b| b.feature_mode == feature_mode)
            .map(|b| b.parent_mode.as_str())
    }

    /// Get all stored bridges.
    #[must_use]
    pub fn bridges(&self) -> &[ManifestModeBridge] {
        &self.bridges
    }
}

impl Service for ModeBridgeStore {}

impl fmt::Debug for ModeBridgeStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModeBridgeStore")
            .field("count", &self.bridges.len())
            .finish()
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
