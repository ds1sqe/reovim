//! Window mode keybindings (`<C-w>` prefix).
//!
//! Window mode is entered via `<C-w>` from normal mode. Each key executes
//! a window command and returns to normal mode automatically.
//!
//! # Navigation (h/j/k/l)
//!
//! - `h` / `<Left>` - Focus window to the left
//! - `j` / `<Down>` - Focus window below
//! - `k` / `<Up>` - Focus window above
//! - `l` / `<Right>` - Focus window to the right
//!
//! # Focus Cycling (w/W/p)
//!
//! - `w` - Cycle to next window
//! - `W` / `p` - Cycle to previous window
//!
//! # Splitting (s/v/n)
//!
//! - `s` - Split window horizontally
//! - `v` - Split window vertically
//! - `n` - New window with empty buffer
//!
//! # Closing (c/q/o)
//!
//! - `c` / `q` - Close current window
//! - `o` - Close all other windows (`:only`)
//!
//! # Resizing (+/-/>/</=)
//!
//! - `+` - Increase window height
//! - `-` - Decrease window height
//! - `>` - Increase window width
//! - `<` - Decrease window width
//! - `=` - Equalize all window sizes
//!
//! # Float Zone (f/]/[)
//!
//! - `f` - Toggle between tiled and floating
//! - `]` - Raise float to front
//! - `[` - Lower float to back
//!
//! # Tabs (T)
//!
//! - `T` - Move current window to new tab

use {reovim_kernel::api::v1::KeybindingRegistration, reovim_module_window_ops::ids as window_ops};

/// Returns all window mode keybindings.
///
/// These bindings are active when in `vim:window` mode (entered via `<C-w>`).
/// Each command automatically returns to normal mode after execution.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn bindings() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Focus Navigation (h/j/k/l)
        // ====================================================================
        KeybindingRegistration::new("h", window_ops::FOCUS_LEFT)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window to the left"),
        KeybindingRegistration::new("j", window_ops::FOCUS_DOWN)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window below"),
        KeybindingRegistration::new("k", window_ops::FOCUS_UP)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window above"),
        KeybindingRegistration::new("l", window_ops::FOCUS_RIGHT)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window to the right"),
        // ====================================================================
        // Arrow Key Alternatives
        // ====================================================================
        KeybindingRegistration::new("<Left>", window_ops::FOCUS_LEFT)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window to the left"),
        KeybindingRegistration::new("<Down>", window_ops::FOCUS_DOWN)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window below"),
        KeybindingRegistration::new("<Up>", window_ops::FOCUS_UP)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window above"),
        KeybindingRegistration::new("<Right>", window_ops::FOCUS_RIGHT)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window to the right"),
        // ====================================================================
        // Focus Cycling (w/W/p)
        // ====================================================================
        KeybindingRegistration::new("w", window_ops::FOCUS_NEXT)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Cycle to next window"),
        KeybindingRegistration::new("W", window_ops::FOCUS_PREV)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Cycle to previous window"),
        KeybindingRegistration::new("p", window_ops::FOCUS_PREV)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Cycle to previous window"),
        // ====================================================================
        // Splitting (s/v/n)
        // ====================================================================
        KeybindingRegistration::new("s", window_ops::SPLIT_HORIZONTAL)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Split window horizontally"),
        KeybindingRegistration::new("v", window_ops::SPLIT_VERTICAL)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Split window vertically"),
        KeybindingRegistration::new("n", window_ops::SPLIT_NEW)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("New window with empty buffer"),
        // ====================================================================
        // Closing (c/q/o)
        // ====================================================================
        KeybindingRegistration::new("c", window_ops::CLOSE_WINDOW)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Close current window"),
        KeybindingRegistration::new("q", window_ops::CLOSE_WINDOW)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Close current window"),
        KeybindingRegistration::new("o", window_ops::CLOSE_OTHERS)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Close all other windows"),
        // ====================================================================
        // Resizing (+/-/>/</=)
        // ====================================================================
        KeybindingRegistration::new("+", window_ops::RESIZE_HEIGHT_INCREASE)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Increase window height"),
        KeybindingRegistration::new("-", window_ops::RESIZE_HEIGHT_DECREASE)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Decrease window height"),
        KeybindingRegistration::new(">", window_ops::RESIZE_WIDTH_INCREASE)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Increase window width"),
        KeybindingRegistration::new("<lt>", window_ops::RESIZE_WIDTH_DECREASE)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Decrease window width"),
        KeybindingRegistration::new("=", window_ops::RESIZE_EQUAL)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Equalize all window sizes"),
        // ====================================================================
        // Float Zone (#398)
        // ====================================================================
        KeybindingRegistration::new("f", window_ops::TOGGLE_FLOAT)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Toggle between tiled and floating"),
        KeybindingRegistration::new("]", window_ops::RAISE_FLOAT)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Raise float to front"),
        KeybindingRegistration::new("[", window_ops::LOWER_FLOAT)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Lower float to back"),
        // ====================================================================
        // Tabs (#401)
        // ====================================================================
        KeybindingRegistration::new("T", window_ops::MOVE_TO_NEW_TAB)
            .with_modes(&["vim:window"])
            .with_category("tab")
            .with_description("Move current window to new tab"),
        // ====================================================================
        // Escape back to normal mode
        // ====================================================================
        KeybindingRegistration::new("<Esc>", crate::ids::CANCEL_TO_NORMAL)
            .with_modes(&["vim:window"])
            .with_category("mode")
            .with_description("Return to normal mode"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bindings_not_empty() {
        let bindings = bindings();
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_bindings_all_in_window_mode() {
        for binding in bindings() {
            assert!(
                binding.modes.contains(&"vim:window"),
                "Binding '{}' should be in vim:window mode",
                binding.keys
            );
        }
    }

    #[test]
    fn test_navigation_bindings_exist() {
        let bindings = bindings();
        let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"h"), "Should have 'h' for focus left");
        assert!(keys.contains(&"j"), "Should have 'j' for focus down");
        assert!(keys.contains(&"k"), "Should have 'k' for focus up");
        assert!(keys.contains(&"l"), "Should have 'l' for focus right");
    }

    #[test]
    fn test_split_bindings_exist() {
        let bindings = bindings();
        let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"s"), "Should have 's' for horizontal split");
        assert!(keys.contains(&"v"), "Should have 'v' for vertical split");
    }

    #[test]
    fn test_close_bindings_exist() {
        let bindings = bindings();
        let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"c"), "Should have 'c' for close window");
        assert!(keys.contains(&"o"), "Should have 'o' for close others");
    }

    #[test]
    fn test_resize_bindings_exist() {
        let bindings = bindings();
        let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"+"), "Should have '+' for increase height");
        assert!(keys.contains(&"-"), "Should have '-' for decrease height");
        assert!(keys.contains(&">"), "Should have '>' for increase width");
        assert!(keys.contains(&"<lt>"), "Should have '<lt>' for decrease width");
        assert!(keys.contains(&"="), "Should have '=' for equalize");
    }

    #[test]
    fn test_arrow_key_alternatives_exist() {
        let bindings = bindings();
        let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"<Left>"), "Should have '<Left>' for focus left");
        assert!(keys.contains(&"<Down>"), "Should have '<Down>' for focus down");
        assert!(keys.contains(&"<Up>"), "Should have '<Up>' for focus up");
        assert!(keys.contains(&"<Right>"), "Should have '<Right>' for focus right");
    }

    #[test]
    fn test_focus_cycling_bindings_exist() {
        let bindings = bindings();
        let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"w"), "Should have 'w' for next window");
        assert!(keys.contains(&"W"), "Should have 'W' for previous window");
        assert!(keys.contains(&"p"), "Should have 'p' for previous window");
    }

    #[test]
    fn test_new_window_binding_exists() {
        let bindings = bindings();
        assert!(bindings.iter().any(|b| b.keys == "n"), "Should have 'n' for new window");
    }

    #[test]
    fn test_close_others_binding_exists() {
        let bindings = bindings();
        assert!(bindings.iter().any(|b| b.keys == "q"), "Should have 'q' for close window");
    }

    #[test]
    fn test_float_zone_bindings_exist() {
        let bindings = bindings();
        let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();
        assert!(keys.contains(&"f"), "Should have 'f' for toggle float");
        assert!(keys.contains(&"]"), "Should have ']' for raise float");
        assert!(keys.contains(&"["), "Should have '[' for lower float");
    }

    #[test]
    fn test_escape_binding_exists() {
        let bindings = bindings();
        assert!(bindings.iter().any(|b| b.keys == "<Esc>"), "Should have '<Esc>' for escape");
    }

    #[test]
    fn test_all_bindings_have_description() {
        for binding in bindings() {
            assert!(
                !binding.description.is_empty(),
                "Binding '{}' should have a description",
                binding.keys
            );
        }
    }

    #[test]
    fn test_all_bindings_have_category() {
        for binding in bindings() {
            assert!(
                binding.category.is_some(),
                "Binding '{}' should have a category",
                binding.keys
            );
        }
    }

    #[test]
    fn test_bindings_count() {
        let b = bindings();
        // Navigation (4) + Arrows (4) + Cycling (3) + Split (3) + Close (3) +
        // Resize (5) + Float (3) + Tabs (1) + Escape (1) = 27
        assert_eq!(b.len(), 27);
    }
}
