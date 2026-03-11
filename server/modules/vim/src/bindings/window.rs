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
