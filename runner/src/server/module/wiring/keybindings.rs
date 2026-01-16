//! Keybinding wiring for modules.
//!
//! Provides functions to wire module keybindings to the session registries.

use std::fmt;

use {
    reovim_driver_input::KeySequence,
    reovim_kernel::api::v1::{CommandId, KeybindingRegistration, ModeId, ModuleId},
};

use crate::server::registry::KeymapRegistry;

/// Result of a wiring operation.
pub type WiringResult = Result<WiringStats, WiringError>;

/// Statistics from a wiring operation.
#[derive(Debug, Default, Clone)]
pub struct WiringStats {
    /// Number of keybindings successfully wired.
    pub keybindings_wired: usize,
    /// Number of keybindings skipped (disabled or invalid).
    pub keybindings_skipped: usize,
}

impl WiringStats {
    /// Create empty stats.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            keybindings_wired: 0,
            keybindings_skipped: 0,
        }
    }

    /// Merge another stats into this one.
    pub const fn merge(&mut self, other: &Self) {
        self.keybindings_wired += other.keybindings_wired;
        self.keybindings_skipped += other.keybindings_skipped;
    }
}

/// Error during handler wiring.
#[derive(Debug, Clone)]
pub enum WiringError {
    /// Failed to parse a key sequence.
    InvalidKeySequence {
        /// The keys string that couldn't be parsed.
        keys: String,
        /// The module that provided the invalid keybinding.
        module: ModuleId,
    },
    /// A required keybinding couldn't be registered.
    RequiredBindingFailed {
        /// The keys string for the failed binding.
        keys: String,
        /// The module that provided the binding.
        module: ModuleId,
        /// The reason for failure.
        reason: String,
    },
}

impl fmt::Display for WiringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKeySequence { keys, module } => {
                write!(f, "invalid key sequence '{}' from module '{}'", keys, module.as_str())
            }
            Self::RequiredBindingFailed {
                keys,
                module,
                reason,
            } => {
                write!(
                    f,
                    "required keybinding '{}' from module '{}' failed: {}",
                    keys,
                    module.as_str(),
                    reason
                )
            }
        }
    }
}

impl std::error::Error for WiringError {}

/// Wire a module's keybindings to the keymap registry.
///
/// Processes the module's `KeybindingRegistration` entries and registers
/// them with ownership tracking.
///
/// # Arguments
///
/// * `module_id` - The ID of the module providing the keybindings
/// * `keybindings` - The keybinding registrations from the module
/// * `keymap_registry` - The registry to wire keybindings to
///
/// # Returns
///
/// - `Ok(WiringStats)` with the number of keybindings wired/skipped
/// - `Err(WiringError)` if a required keybinding fails
///
/// # Errors
///
/// Returns `WiringError::InvalidKeySequence` if a required keybinding has an
/// invalid key sequence. Returns `WiringError::RequiredBindingFailed` if a
/// required keybinding fails for other reasons (e.g., no modes specified).
///
/// # Note
///
/// Disabled keybindings (where `enabled == false`) are skipped.
/// Non-required keybindings that fail to parse are skipped with a warning.
/// Required keybindings that fail to parse return an error.
pub fn wire_module_keybindings(
    module_id: &ModuleId,
    keybindings: &[KeybindingRegistration],
    keymap_registry: &mut KeymapRegistry,
) -> WiringResult {
    let mut stats = WiringStats::new();

    for registration in keybindings {
        // Skip disabled keybindings
        if !registration.enabled {
            stats.keybindings_skipped += 1;
            continue;
        }

        // Parse the key sequence
        let Some(keys) = KeySequence::parse(registration.keys) else {
            if registration.flags.is_required() {
                return Err(WiringError::InvalidKeySequence {
                    keys: registration.keys.to_string(),
                    module: module_id.clone(),
                });
            }
            // Non-required binding with invalid keys - skip
            tracing::warn!(
                module = %module_id,
                keys = registration.keys,
                "skipping keybinding with invalid key sequence"
            );
            stats.keybindings_skipped += 1;
            continue;
        };

        // Build the command ID
        // Convention: if command_id doesn't contain ':', it's prefixed with module ID
        let command_id = if registration.command_id.contains(':') {
            // Already fully qualified (e.g., "editor:cursor-down")
            let parts: Vec<&str> = registration.command_id.splitn(2, ':').collect();
            if parts.len() == 2 {
                CommandId::new(ModuleId::from_string(parts[0].to_string()), parts[1])
            } else {
                CommandId::new(module_id.clone(), registration.command_id)
            }
        } else {
            // Use the registering module as the command owner
            CommandId::new(module_id.clone(), registration.command_id)
        };

        // Determine modes to register in
        let modes: Vec<ModeId> = if registration.modes.is_empty() {
            // Empty modes means all modes - but we need at least one mode to register
            // The caller should handle this case by providing default modes
            // For now, skip if no modes specified and it's not required
            if registration.flags.is_required() {
                return Err(WiringError::RequiredBindingFailed {
                    keys: registration.keys.to_string(),
                    module: module_id.clone(),
                    reason: "no modes specified for keybinding".to_string(),
                });
            }
            tracing::warn!(
                module = %module_id,
                keys = registration.keys,
                "skipping keybinding with no modes specified"
            );
            stats.keybindings_skipped += 1;
            continue;
        } else {
            registration
                .modes
                .iter()
                .map(|mode_name| {
                    // Mode names are typically "module:name" or just "name"
                    // If just "name", assume it's from the same module
                    if mode_name.contains(':') {
                        let parts: Vec<&str> = mode_name.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            ModeId::new(ModuleId::from_string(parts[0].to_string()), parts[1])
                        } else {
                            ModeId::new(module_id.clone(), mode_name)
                        }
                    } else {
                        // No explicit module prefix - use the registering module
                        ModeId::new(module_id.clone(), mode_name)
                    }
                })
                .collect()
        };

        // Register the keybinding in each mode
        for mode in modes {
            keymap_registry.register_for_module(
                mode,
                keys.clone(),
                command_id.clone(),
                module_id.clone(),
            );
        }

        stats.keybindings_wired += 1;
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_kernel::api::v1::{KeybindingRegistration, RegistrationFlags},
    };

    #[test]
    fn test_wiring_stats_new() {
        let stats = WiringStats::new();
        assert_eq!(stats.keybindings_wired, 0);
        assert_eq!(stats.keybindings_skipped, 0);
    }

    #[test]
    fn test_wiring_stats_merge() {
        let mut stats1 = WiringStats {
            keybindings_wired: 5,
            keybindings_skipped: 2,
        };
        let stats2 = WiringStats {
            keybindings_wired: 3,
            keybindings_skipped: 1,
        };

        stats1.merge(&stats2);

        assert_eq!(stats1.keybindings_wired, 8);
        assert_eq!(stats1.keybindings_skipped, 3);
    }

    #[test]
    fn test_wiring_error_display() {
        let err = WiringError::InvalidKeySequence {
            keys: "invalid".to_string(),
            module: ModuleId::new("test"),
        };
        assert!(err.to_string().contains("invalid key sequence"));
        assert!(err.to_string().contains("test"));

        let err = WiringError::RequiredBindingFailed {
            keys: "j".to_string(),
            module: ModuleId::new("test"),
            reason: "no modes".to_string(),
        };
        assert!(err.to_string().contains("required keybinding"));
    }

    #[test]
    fn test_wire_module_keybindings_simple() {
        let mut registry = KeymapRegistry::new();
        let module_id = ModuleId::new("my-module");

        let keybindings = vec![
            KeybindingRegistration::new("j", "cursor-down").with_modes(&["normal"]),
            KeybindingRegistration::new("k", "cursor-up").with_modes(&["normal"]),
        ];

        let result = wire_module_keybindings(&module_id, &keybindings, &mut registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.keybindings_wired, 2);
        assert_eq!(stats.keybindings_skipped, 0);
    }

    #[test]
    fn test_wire_module_keybindings_skips_disabled() {
        let mut registry = KeymapRegistry::new();
        let module_id = ModuleId::new("my-module");

        let keybindings = vec![
            KeybindingRegistration::new("j", "cursor-down")
                .with_modes(&["normal"])
                .with_disabled(),
        ];

        let result = wire_module_keybindings(&module_id, &keybindings, &mut registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.keybindings_wired, 0);
        assert_eq!(stats.keybindings_skipped, 1);
    }

    #[test]
    fn test_wire_module_keybindings_invalid_keys_non_required() {
        let mut registry = KeymapRegistry::new();
        let module_id = ModuleId::new("my-module");

        // Invalid key sequence but not required
        let keybindings =
            vec![KeybindingRegistration::new("<INVALID_KEY>", "some-cmd").with_modes(&["normal"])];

        let result = wire_module_keybindings(&module_id, &keybindings, &mut registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.keybindings_wired, 0);
        assert_eq!(stats.keybindings_skipped, 1);
    }

    #[test]
    fn test_wire_module_keybindings_invalid_keys_required() {
        let mut registry = KeymapRegistry::new();
        let module_id = ModuleId::new("my-module");

        // Invalid key sequence and required
        let keybindings = vec![
            KeybindingRegistration::new("<INVALID_KEY>", "some-cmd")
                .with_modes(&["normal"])
                .with_flags(RegistrationFlags::required()),
        ];

        let result = wire_module_keybindings(&module_id, &keybindings, &mut registry);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, WiringError::InvalidKeySequence { .. }));
    }

    #[test]
    fn test_wire_module_keybindings_no_modes_non_required() {
        let mut registry = KeymapRegistry::new();
        let module_id = ModuleId::new("my-module");

        // No modes specified (empty slice is the default)
        let keybindings = vec![KeybindingRegistration::new("j", "some-cmd")];

        let result = wire_module_keybindings(&module_id, &keybindings, &mut registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.keybindings_wired, 0);
        assert_eq!(stats.keybindings_skipped, 1);
    }

    #[test]
    fn test_wire_module_keybindings_no_modes_required() {
        let mut registry = KeymapRegistry::new();
        let module_id = ModuleId::new("my-module");

        // No modes specified and required
        let keybindings = vec![
            KeybindingRegistration::new("j", "some-cmd").with_flags(RegistrationFlags::required()),
        ];

        let result = wire_module_keybindings(&module_id, &keybindings, &mut registry);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, WiringError::RequiredBindingFailed { .. }));
    }

    #[test]
    fn test_wire_module_keybindings_qualified_command_id() {
        let mut registry = KeymapRegistry::new();
        let module_id = ModuleId::new("my-module");

        // Command ID with explicit module prefix
        let keybindings =
            vec![KeybindingRegistration::new("j", "editor:cursor-down").with_modes(&["normal"])];

        let result = wire_module_keybindings(&module_id, &keybindings, &mut registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.keybindings_wired, 1);
    }

    #[test]
    fn test_wire_module_keybindings_multiple_modes() {
        let mut registry = KeymapRegistry::new();
        let module_id = ModuleId::new("my-module");

        // Same key in multiple modes
        let keybindings = vec![
            KeybindingRegistration::new("<Esc>", "enter-normal").with_modes(&["insert", "visual"]),
        ];

        let result = wire_module_keybindings(&module_id, &keybindings, &mut registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.keybindings_wired, 1);

        // Both modes should have the binding - using the registering module, not hardcoded "editor"
        let insert_mode = ModeId::new(module_id.clone(), "insert");
        let visual_mode = ModeId::new(module_id.clone(), "visual");

        assert_eq!(registry.binding_count(&insert_mode), 1);
        assert_eq!(registry.binding_count(&visual_mode), 1);
    }
}
