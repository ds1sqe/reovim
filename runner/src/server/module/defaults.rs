//! Default modules loaded on server startup.
//!
//! This module defines the hardcoded default module list that provides
//! core editing functionality out of the box. Users can override this
//! via config file or CLI flags.
//!
//! # Precedence (highest to lowest)
//!
//! 1. CLI `--load` flags
//! 2. Config file `[modules].autoload`
//! 3. Config file `[modules].extra` (adds to defaults)
//! 4. Config file `[modules].skip` (removes from defaults)
//! 5. This `DEFAULT_MODULES` constant
//!
//! # Example
//!
//! ```toml
//! # ~/.config/reovim/config.toml
//! [modules]
//! extra = ["my-custom-module"]  # Add to defaults
//! skip = ["operators"]          # Remove from defaults
//! ```

/// Default modules to load when server starts.
///
/// These provide core editing functionality:
///
/// - `editor` - Cursor movement, mode switching, basic editing
/// - `keymap` - Vim keybindings and key sequence handling
/// - `operators` - Vim operators (d, y, c, etc.)
/// - `motions` - Vim motions (w, e, b, $, ^, G, etc.)
/// - `commands` - Ex commands (:w, :q, :wq, etc.)
/// - `mode-manager` - Mode transitions and mode stack management
///
/// # Override
///
/// - Use `--no-defaults` CLI flag to skip all defaults
/// - Use `[modules].autoload` in config to replace this list
/// - Use `[modules].skip` in config to remove specific modules
/// - Use `[modules].extra` in config to add modules
pub const DEFAULT_MODULES: &[&str] = &[
    "editor",
    "keymap",
    "operators",
    "motions",
    "commands",
    "mode-manager",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_modules_not_empty() {
        assert!(
            !DEFAULT_MODULES.is_empty(),
            "DEFAULT_MODULES should contain at least one module"
        );
    }

    #[test]
    fn test_default_modules_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for module in DEFAULT_MODULES {
            assert!(seen.insert(*module), "Duplicate module found in DEFAULT_MODULES: {module}");
        }
    }

    #[test]
    fn test_default_modules_contains_core() {
        // Verify the essential modules are present
        assert!(DEFAULT_MODULES.contains(&"editor"), "DEFAULT_MODULES should contain 'editor'");
        assert!(DEFAULT_MODULES.contains(&"keymap"), "DEFAULT_MODULES should contain 'keymap'");
    }

    #[test]
    fn test_default_modules_no_empty_strings() {
        for module in DEFAULT_MODULES {
            assert!(!module.is_empty(), "DEFAULT_MODULES should not contain empty strings");
        }
    }
}
