//! Vim keybindings.
//!
//! This module contains all Vim-style keybindings organized by mode:
//! - `normal` - Normal mode (hjkl, operators, etc.)
//! - `insert` - Insert mode (Escape, etc.)
//! - `visual` - Visual mode (selection commands)
//! - `operator_modes` - Dedicated operator modes: delete, yank, change (Epic #415)
//! - `commandline` - Command-line mode (: commands)
//! - `window` - Window mode (`<C-w>` commands) (Epic #438)

pub mod commandline;
pub mod insert;
pub mod normal;
pub mod operator_modes;
pub mod visual;
pub mod window;

use reovim_kernel::api::v1::KeybindingRegistration;

/// Returns all Vim keybindings.
///
/// This collects bindings from all modes into a single vector.
#[must_use]
pub fn all() -> Vec<KeybindingRegistration> {
    let mut bindings = Vec::new();
    bindings.extend(normal::bindings());
    bindings.extend(insert::bindings());
    bindings.extend(visual::bindings());
    bindings.extend(operator_modes::all_operator_bindings());
    bindings.extend(commandline::bindings());
    bindings.extend(window::bindings());
    bindings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_bindings_not_empty() {
        let bindings = all();
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_all_bindings_combines_all_modes() {
        let all_bindings = all();
        let normal_count = normal::bindings().len();
        let insert_count = insert::bindings().len();
        let visual_count = visual::bindings().len();
        let operator_count = operator_modes::all_operator_bindings().len();
        let commandline_count = commandline::bindings().len();
        let window_count = window::bindings().len();

        let expected = normal_count
            + insert_count
            + visual_count
            + operator_count
            + commandline_count
            + window_count;
        assert_eq!(all_bindings.len(), expected);
    }

    #[test]
    fn test_all_bindings_has_description() {
        for binding in all() {
            assert!(
                !binding.description.is_empty(),
                "Binding '{}' should have a description",
                binding.keys
            );
        }
    }

    #[test]
    fn test_all_bindings_has_category() {
        for binding in all() {
            assert!(
                binding.category.is_some(),
                "Binding '{}' should have a category",
                binding.keys
            );
        }
    }

    #[test]
    fn test_all_bindings_has_modes() {
        for binding in all() {
            assert!(
                !binding.modes.is_empty(),
                "Binding '{}' should have at least one mode",
                binding.keys
            );
        }
    }

    // ========================================================================
    // Mode-specific binding count tests
    // ========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_normal_bindings_count() {
        let bindings = normal::bindings();
        // Should have a reasonable number of normal mode bindings
        assert!(
            bindings.len() > 40,
            "Normal mode should have many bindings, got {}",
            bindings.len()
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_insert_bindings_count() {
        let bindings = insert::bindings();
        // Should have insert mode bindings
        assert!(
            bindings.len() > 10,
            "Insert mode should have several bindings, got {}",
            bindings.len()
        );
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_visual_bindings_count() {
        let bindings = visual::bindings();
        // Should have visual mode bindings
        assert!(
            bindings.len() > 20,
            "Visual mode should have many bindings, got {}",
            bindings.len()
        );
    }

    #[test]
    fn test_commandline_bindings_count() {
        let bindings = commandline::bindings();
        assert_eq!(bindings.len(), 3, "Commandline mode should have 3 bindings");
    }

    #[test]
    fn test_window_bindings_count() {
        let bindings = window::bindings();
        assert_eq!(bindings.len(), 26, "Window mode should have 26 bindings");
    }

    #[test]
    fn test_operator_bindings_symmetric_structure() {
        // All three operator modes should have the same basic structure
        let delete_len = operator_modes::delete_bindings().len();
        let yank_len = operator_modes::yank_bindings().len();
        let change_len = operator_modes::change_bindings().len();

        // All should have equal lengths (same motions + text objects)
        assert_eq!(delete_len, yank_len, "delete and yank should have same binding count");
        assert_eq!(yank_len, change_len, "yank and change should have same binding count");
    }

    // ========================================================================
    // Mode names consistency tests
    // ========================================================================

    #[test]
    fn test_all_normal_bindings_use_correct_mode_string() {
        for binding in normal::bindings() {
            assert!(
                binding.modes.contains(&"vim:normal"),
                "Normal binding '{}' should use 'vim:normal'",
                binding.keys
            );
        }
    }

    #[test]
    fn test_all_insert_bindings_use_correct_mode_string() {
        for binding in insert::bindings() {
            assert!(
                binding.modes.contains(&"vim:insert"),
                "Insert binding '{}' should use 'vim:insert'",
                binding.keys
            );
        }
    }

    #[test]
    fn test_all_window_bindings_use_correct_mode_string() {
        for binding in window::bindings() {
            assert!(
                binding.modes.contains(&"vim:window"),
                "Window binding '{}' should use 'vim:window'",
                binding.keys
            );
        }
    }

    #[test]
    fn test_all_commandline_bindings_use_correct_mode_string() {
        for binding in commandline::bindings() {
            assert!(
                binding.modes.contains(&"vim:command"),
                "Commandline binding '{}' should use 'vim:command'",
                binding.keys
            );
        }
    }

    // ========================================================================
    // Cross-mode binding uniqueness tests
    // ========================================================================

    #[test]
    fn test_all_bindings_have_valid_modes() {
        let valid_modes = [
            "vim:normal",
            "vim:insert",
            "vim:visual",
            "vim:visual-line",
            "vim:visual-block",
            "vim:command",
            "vim:window",
            "vim:delete",
            "vim:yank",
            "vim:change",
        ];
        for binding in all() {
            for mode in binding.modes {
                assert!(
                    valid_modes.contains(mode),
                    "Binding '{}' has invalid mode '{}'",
                    binding.keys,
                    mode
                );
            }
        }
    }

    #[test]
    fn test_all_bindings_non_empty_keys() {
        for binding in all() {
            assert!(!binding.keys.is_empty(), "Binding should have non-empty keys");
        }
    }
}
