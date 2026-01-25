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

use {reovim_kernel::api::v1::KeybindingRegistration, reovim_module_layout::ids as layout};

/// Returns all window mode keybindings.
///
/// These bindings are active when in `vim:window` mode (entered via `<C-w>`).
/// Each command automatically returns to normal mode after execution.
#[must_use]
pub fn bindings() -> Vec<KeybindingRegistration> {
    vec![
        // ====================================================================
        // Focus Navigation (h/j/k/l)
        // ====================================================================
        KeybindingRegistration::new("h", layout::FOCUS_LEFT)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window to the left"),
        KeybindingRegistration::new("j", layout::FOCUS_DOWN)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window below"),
        KeybindingRegistration::new("k", layout::FOCUS_UP)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window above"),
        KeybindingRegistration::new("l", layout::FOCUS_RIGHT)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window to the right"),
        // ====================================================================
        // Arrow Key Alternatives
        // ====================================================================
        KeybindingRegistration::new("<Left>", layout::FOCUS_LEFT)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window to the left"),
        KeybindingRegistration::new("<Down>", layout::FOCUS_DOWN)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window below"),
        KeybindingRegistration::new("<Up>", layout::FOCUS_UP)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window above"),
        KeybindingRegistration::new("<Right>", layout::FOCUS_RIGHT)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Focus window to the right"),
        // ====================================================================
        // Focus Cycling (w/W/p)
        // ====================================================================
        KeybindingRegistration::new("w", layout::FOCUS_NEXT)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Cycle to next window"),
        KeybindingRegistration::new("W", layout::FOCUS_PREV)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Cycle to previous window"),
        KeybindingRegistration::new("p", layout::FOCUS_PREV)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Cycle to previous window"),
        // ====================================================================
        // Splitting (s/v/n)
        // ====================================================================
        KeybindingRegistration::new("s", layout::SPLIT_HORIZONTAL)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Split window horizontally"),
        KeybindingRegistration::new("v", layout::SPLIT_VERTICAL)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Split window vertically"),
        KeybindingRegistration::new("n", layout::SPLIT_NEW)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("New window with empty buffer"),
        // ====================================================================
        // Closing (c/q/o)
        // ====================================================================
        KeybindingRegistration::new("c", layout::CLOSE_WINDOW)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Close current window"),
        KeybindingRegistration::new("q", layout::CLOSE_WINDOW)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Close current window"),
        KeybindingRegistration::new("o", layout::CLOSE_OTHERS)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Close all other windows"),
        // ====================================================================
        // Resizing (+/-/>/</=)
        // ====================================================================
        KeybindingRegistration::new("+", layout::RESIZE_HEIGHT_INCREASE)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Increase window height"),
        KeybindingRegistration::new("-", layout::RESIZE_HEIGHT_DECREASE)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Decrease window height"),
        KeybindingRegistration::new(">", layout::RESIZE_WIDTH_INCREASE)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Increase window width"),
        KeybindingRegistration::new("<", layout::RESIZE_WIDTH_DECREASE)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Decrease window width"),
        KeybindingRegistration::new("=", layout::RESIZE_EQUAL)
            .with_modes(&["vim:window"])
            .with_category("window")
            .with_description("Equalize all window sizes"),
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
        assert!(keys.contains(&"<"), "Should have '<' for decrease width");
        assert!(keys.contains(&"="), "Should have '=' for equalize");
    }
}
