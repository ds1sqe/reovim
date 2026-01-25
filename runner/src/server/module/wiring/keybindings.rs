//! Keybinding wiring for modules.
//!
//! Provides functions to wire module keybindings to the session registries.

use std::fmt;

use {
    reovim_driver_input::KeySequence,
    reovim_kernel::api::v1::{KeybindingRegistration, ModeId, ModuleId},
};

use crate::server::registry::{KeymapRegistry, ModeRegistry};

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
/// * `mode_registry` - The registry to look up mode IDs from names
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
///
/// Mode names are looked up in the mode registry to get the correct `ModeId`
/// with proper discriminant. If a mode is not found, it falls back to creating
/// a `ModeId` with discriminant 0 (for forward compatibility with new modes).
#[allow(deprecated)]
pub fn wire_module_keybindings(
    module_id: &ModuleId,
    keybindings: &[KeybindingRegistration],
    keymap_registry: &mut KeymapRegistry,
    mode_registry: &ModeRegistry,
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

        // Command ID is now a typed CommandId - use it directly
        // No string parsing needed since compile-time verification ensures correctness
        let command_id = registration.command_id.clone();

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
                .filter_map(|mode_name| {
                    // Mode names are typically "module:name" or just "name"
                    // If just "name", assume it's from the same module
                    let (mode_module, mode_local) = if mode_name.contains(':') {
                        let parts: Vec<&str> = mode_name.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            (parts[0], parts[1])
                        } else {
                            (module_id.as_str(), *mode_name)
                        }
                    } else {
                        // No explicit module prefix - use the registering module
                        (module_id.as_str(), *mode_name)
                    };

                    // Look up the mode in the registry to get the correct ModeId
                    // with proper discriminant
                    mode_registry
                        .find_by_name(mode_module, mode_local)
                        .map_or_else(
                            || {
                                // Mode not found - warn and skip this mode binding
                                // This is more conservative than creating a ModeId with
                                // discriminant 0, which would cause all modes to collide
                                tracing::warn!(
                                    module = %module_id,
                                    mode = %mode_name,
                                    "mode not found in registry, skipping binding"
                                );
                                None
                            },
                            |mode_id| Some(mode_id.clone()),
                        )
                })
                .collect()
        };

        // Register the keybinding in each mode
        for mode in modes {
            keymap_registry.register_for_module(
                &mode,
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
        reovim_kernel::api::v1::{
            CommandId, CursorStyle, KeybindingRegistration, Mode, RegistrationFlags,
        },
    };

    /// Test module ID for test modes.
    const TEST_MODULE: ModuleId = ModuleId::new("my-module");

    // Test command IDs
    const CMD_CURSOR_DOWN: CommandId = CommandId::new(TEST_MODULE, "cursor-down");
    const CMD_CURSOR_UP: CommandId = CommandId::new(TEST_MODULE, "cursor-up");
    const CMD_SOME: CommandId = CommandId::new(TEST_MODULE, "some-cmd");
    const CMD_ENTER_COMMAND: CommandId = CommandId::new(TEST_MODULE, "enter-command");
    const EDITOR_MODULE: ModuleId = ModuleId::new("editor");
    const CMD_EDITOR_CURSOR_DOWN: CommandId = CommandId::new(EDITOR_MODULE, "cursor-down");

    /// Test mode enum implementing the Mode trait for testing.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(u16)]
    enum TestMode {
        Command = 0,
        Input = 1,
        Selection = 2,
    }

    impl Mode for TestMode {
        fn module() -> ModuleId {
            TEST_MODULE
        }

        fn discriminant(&self) -> u16 {
            *self as u16
        }

        fn display_name(&self) -> &'static str {
            match self {
                Self::Command => "command",
                Self::Input => "input",
                Self::Selection => "selection",
            }
        }

        fn cursor_style(&self) -> CursorStyle {
            match self {
                Self::Input => CursorStyle::Bar,
                _ => CursorStyle::Block,
            }
        }

        fn accepts_char_input(&self) -> bool {
            matches!(self, Self::Input)
        }
    }

    /// Create a mode registry with test modes.
    fn test_mode_registry() -> ModeRegistry {
        let mut registry = ModeRegistry::new();
        registry.register_mode(TestMode::Command);
        registry.register_mode(TestMode::Input);
        registry.register_mode(TestMode::Selection);
        registry
    }

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
        let mut keymap_registry = KeymapRegistry::new();
        let mode_registry = test_mode_registry();
        let module_id = ModuleId::new("my-module");

        let keybindings = vec![
            KeybindingRegistration::new("j", CMD_CURSOR_DOWN).with_modes(&["command"]),
            KeybindingRegistration::new("k", CMD_CURSOR_UP).with_modes(&["command"]),
        ];

        let result =
            wire_module_keybindings(&module_id, &keybindings, &mut keymap_registry, &mode_registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.keybindings_wired, 2);
        assert_eq!(stats.keybindings_skipped, 0);
    }

    #[test]
    fn test_wire_module_keybindings_skips_disabled() {
        let mut keymap_registry = KeymapRegistry::new();
        let mode_registry = test_mode_registry();
        let module_id = ModuleId::new("my-module");

        let keybindings = vec![
            KeybindingRegistration::new("j", CMD_CURSOR_DOWN)
                .with_modes(&["command"])
                .with_disabled(),
        ];

        let result =
            wire_module_keybindings(&module_id, &keybindings, &mut keymap_registry, &mode_registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.keybindings_wired, 0);
        assert_eq!(stats.keybindings_skipped, 1);
    }

    #[test]
    fn test_wire_module_keybindings_invalid_keys_non_required() {
        let mut keymap_registry = KeymapRegistry::new();
        let mode_registry = test_mode_registry();
        let module_id = ModuleId::new("my-module");

        // Invalid key sequence but not required
        let keybindings =
            vec![KeybindingRegistration::new("<INVALID_KEY>", CMD_SOME).with_modes(&["command"])];

        let result =
            wire_module_keybindings(&module_id, &keybindings, &mut keymap_registry, &mode_registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.keybindings_wired, 0);
        assert_eq!(stats.keybindings_skipped, 1);
    }

    #[test]
    fn test_wire_module_keybindings_invalid_keys_required() {
        let mut keymap_registry = KeymapRegistry::new();
        let mode_registry = test_mode_registry();
        let module_id = ModuleId::new("my-module");

        // Invalid key sequence and required
        let keybindings = vec![
            KeybindingRegistration::new("<INVALID_KEY>", CMD_SOME)
                .with_modes(&["command"])
                .with_flags(RegistrationFlags::required()),
        ];

        let result =
            wire_module_keybindings(&module_id, &keybindings, &mut keymap_registry, &mode_registry);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, WiringError::InvalidKeySequence { .. }));
    }

    #[test]
    fn test_wire_module_keybindings_no_modes_non_required() {
        let mut keymap_registry = KeymapRegistry::new();
        let mode_registry = test_mode_registry();
        let module_id = ModuleId::new("my-module");

        // No modes specified (empty slice is the default)
        let keybindings = vec![KeybindingRegistration::new("j", CMD_SOME)];

        let result =
            wire_module_keybindings(&module_id, &keybindings, &mut keymap_registry, &mode_registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.keybindings_wired, 0);
        assert_eq!(stats.keybindings_skipped, 1);
    }

    #[test]
    fn test_wire_module_keybindings_no_modes_required() {
        let mut keymap_registry = KeymapRegistry::new();
        let mode_registry = test_mode_registry();
        let module_id = ModuleId::new("my-module");

        // No modes specified and required
        let keybindings = vec![
            KeybindingRegistration::new("j", CMD_SOME).with_flags(RegistrationFlags::required()),
        ];

        let result =
            wire_module_keybindings(&module_id, &keybindings, &mut keymap_registry, &mode_registry);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, WiringError::RequiredBindingFailed { .. }));
    }

    #[test]
    fn test_wire_module_keybindings_qualified_command_id() {
        let mut keymap_registry = KeymapRegistry::new();
        let mode_registry = test_mode_registry();
        let module_id = ModuleId::new("my-module");

        // Command ID with explicit module prefix (now a typed CommandId)
        let keybindings =
            vec![KeybindingRegistration::new("j", CMD_EDITOR_CURSOR_DOWN).with_modes(&["command"])];

        let result =
            wire_module_keybindings(&module_id, &keybindings, &mut keymap_registry, &mode_registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.keybindings_wired, 1);
    }

    #[test]
    fn test_wire_module_keybindings_multiple_modes() {
        let mut keymap_registry = KeymapRegistry::new();
        let mode_registry = test_mode_registry();
        let module_id = ModuleId::new("my-module");

        // Same key in multiple modes
        let keybindings = vec![
            KeybindingRegistration::new("<Esc>", CMD_ENTER_COMMAND)
                .with_modes(&["input", "selection"]),
        ];

        let result =
            wire_module_keybindings(&module_id, &keybindings, &mut keymap_registry, &mode_registry);

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.keybindings_wired, 1);

        // Both modes should have the binding - look up the correct mode IDs from registry
        let input_mode = mode_registry.find_by_name("my-module", "input").unwrap();
        let selection_mode = mode_registry
            .find_by_name("my-module", "selection")
            .unwrap();

        assert_eq!(keymap_registry.binding_count(input_mode), 1);
        assert_eq!(keymap_registry.binding_count(selection_mode), 1);
    }
}
