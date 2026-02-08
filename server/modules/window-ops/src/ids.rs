//! Command ID constants for window operations.
//!
//! These constants enable compile-time verification of command IDs
//! referenced in keybindings. Previously in `layout` module, now
//! server-side as window operations are data operations.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Window-ops module ID.
pub const MODULE: ModuleId = ModuleId::new("window-ops");

// =============================================================================
// Focus Navigation
// =============================================================================

/// Move focus to left window (h).
pub const FOCUS_LEFT: CommandId = CommandId::new(MODULE, "focus-left");

/// Move focus to window below (j).
pub const FOCUS_DOWN: CommandId = CommandId::new(MODULE, "focus-down");

/// Move focus to window above (k).
pub const FOCUS_UP: CommandId = CommandId::new(MODULE, "focus-up");

/// Move focus to right window (l).
pub const FOCUS_RIGHT: CommandId = CommandId::new(MODULE, "focus-right");

// =============================================================================
// Focus Cycling
// =============================================================================

/// Cycle focus to next window (w).
pub const FOCUS_NEXT: CommandId = CommandId::new(MODULE, "focus-next");

/// Cycle focus to previous window (W, p).
pub const FOCUS_PREV: CommandId = CommandId::new(MODULE, "focus-prev");

// =============================================================================
// Window Splitting
// =============================================================================

/// Split window horizontally (s, :split).
pub const SPLIT_HORIZONTAL: CommandId = CommandId::new(MODULE, "split-horizontal");

/// Split window vertically (v, :vsplit).
pub const SPLIT_VERTICAL: CommandId = CommandId::new(MODULE, "split-vertical");

/// Create new window with empty buffer (n).
pub const SPLIT_NEW: CommandId = CommandId::new(MODULE, "split-new");

// =============================================================================
// Window Closing
// =============================================================================

/// Close current window (c, q, :close).
pub const CLOSE_WINDOW: CommandId = CommandId::new(MODULE, "close-window");

/// Close all other windows (o, :only).
pub const CLOSE_OTHERS: CommandId = CommandId::new(MODULE, "close-others");

// =============================================================================
// Window Resizing
// =============================================================================

/// Increase window height (+).
pub const RESIZE_HEIGHT_INCREASE: CommandId = CommandId::new(MODULE, "resize-height-increase");

/// Decrease window height (-).
pub const RESIZE_HEIGHT_DECREASE: CommandId = CommandId::new(MODULE, "resize-height-decrease");

/// Increase window width (>).
pub const RESIZE_WIDTH_INCREASE: CommandId = CommandId::new(MODULE, "resize-width-increase");

/// Decrease window width (<).
pub const RESIZE_WIDTH_DECREASE: CommandId = CommandId::new(MODULE, "resize-width-decrease");

/// Make all windows equal size (=).
pub const RESIZE_EQUAL: CommandId = CommandId::new(MODULE, "resize-equal");

/// Maximize window height (_).
pub const RESIZE_MAX_HEIGHT: CommandId = CommandId::new(MODULE, "resize-max-height");

/// Maximize window width (|).
pub const RESIZE_MAX_WIDTH: CommandId = CommandId::new(MODULE, "resize-max-width");

// =============================================================================
// Window Movement
// =============================================================================

/// Move window to far left (H).
pub const MOVE_WINDOW_LEFT: CommandId = CommandId::new(MODULE, "move-window-left");

/// Move window to bottom (J).
pub const MOVE_WINDOW_DOWN: CommandId = CommandId::new(MODULE, "move-window-down");

/// Move window to top (K).
pub const MOVE_WINDOW_UP: CommandId = CommandId::new(MODULE, "move-window-up");

/// Move window to far right (L).
pub const MOVE_WINDOW_RIGHT: CommandId = CommandId::new(MODULE, "move-window-right");

/// Rotate windows downwards (r).
pub const ROTATE_WINDOWS: CommandId = CommandId::new(MODULE, "rotate-windows");

/// Rotate windows upwards (R).
pub const ROTATE_WINDOWS_REVERSE: CommandId = CommandId::new(MODULE, "rotate-windows-reverse");

/// Exchange window with next (x).
pub const SWAP_WINDOW: CommandId = CommandId::new(MODULE, "swap-window");

// =============================================================================
// Tab Operations
// =============================================================================

/// Move current window to new tab (T).
pub const MOVE_TO_NEW_TAB: CommandId = CommandId::new(MODULE, "move-to-new-tab");

// =============================================================================
// Float Zone Operations
// =============================================================================

/// Toggle window between tiled and floating zones.
pub const TOGGLE_FLOAT: CommandId = CommandId::new(MODULE, "toggle-float");

/// Raise floating window to front of float zone.
pub const RAISE_FLOAT: CommandId = CommandId::new(MODULE, "raise-float");

/// Lower floating window to back of float zone.
pub const LOWER_FLOAT: CommandId = CommandId::new(MODULE, "lower-float");

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Module ID
    // =========================================================================

    #[test]
    fn test_module_id() {
        assert_eq!(MODULE.as_str(), "window-ops");
    }

    // =========================================================================
    // Focus Navigation IDs
    // =========================================================================

    #[test]
    fn test_focus_left_id() {
        assert_eq!(FOCUS_LEFT.module(), &MODULE);
        assert_eq!(FOCUS_LEFT.name(), "focus-left");
    }

    #[test]
    fn test_focus_down_id() {
        assert_eq!(FOCUS_DOWN.module(), &MODULE);
        assert_eq!(FOCUS_DOWN.name(), "focus-down");
    }

    #[test]
    fn test_focus_up_id() {
        assert_eq!(FOCUS_UP.module(), &MODULE);
        assert_eq!(FOCUS_UP.name(), "focus-up");
    }

    #[test]
    fn test_focus_right_id() {
        assert_eq!(FOCUS_RIGHT.module(), &MODULE);
        assert_eq!(FOCUS_RIGHT.name(), "focus-right");
    }

    // =========================================================================
    // Focus Cycling IDs
    // =========================================================================

    #[test]
    fn test_focus_next_id() {
        assert_eq!(FOCUS_NEXT.module(), &MODULE);
        assert_eq!(FOCUS_NEXT.name(), "focus-next");
    }

    #[test]
    fn test_focus_prev_id() {
        assert_eq!(FOCUS_PREV.module(), &MODULE);
        assert_eq!(FOCUS_PREV.name(), "focus-prev");
    }

    // =========================================================================
    // Window Splitting IDs
    // =========================================================================

    #[test]
    fn test_split_horizontal_id() {
        assert_eq!(SPLIT_HORIZONTAL.module(), &MODULE);
        assert_eq!(SPLIT_HORIZONTAL.name(), "split-horizontal");
    }

    #[test]
    fn test_split_vertical_id() {
        assert_eq!(SPLIT_VERTICAL.module(), &MODULE);
        assert_eq!(SPLIT_VERTICAL.name(), "split-vertical");
    }

    #[test]
    fn test_split_new_id() {
        assert_eq!(SPLIT_NEW.module(), &MODULE);
        assert_eq!(SPLIT_NEW.name(), "split-new");
    }

    // =========================================================================
    // Window Closing IDs
    // =========================================================================

    #[test]
    fn test_close_window_id() {
        assert_eq!(CLOSE_WINDOW.module(), &MODULE);
        assert_eq!(CLOSE_WINDOW.name(), "close-window");
    }

    #[test]
    fn test_close_others_id() {
        assert_eq!(CLOSE_OTHERS.module(), &MODULE);
        assert_eq!(CLOSE_OTHERS.name(), "close-others");
    }

    // =========================================================================
    // Window Resizing IDs
    // =========================================================================

    #[test]
    fn test_resize_height_increase_id() {
        assert_eq!(RESIZE_HEIGHT_INCREASE.module(), &MODULE);
        assert_eq!(RESIZE_HEIGHT_INCREASE.name(), "resize-height-increase");
    }

    #[test]
    fn test_resize_height_decrease_id() {
        assert_eq!(RESIZE_HEIGHT_DECREASE.module(), &MODULE);
        assert_eq!(RESIZE_HEIGHT_DECREASE.name(), "resize-height-decrease");
    }

    #[test]
    fn test_resize_width_increase_id() {
        assert_eq!(RESIZE_WIDTH_INCREASE.module(), &MODULE);
        assert_eq!(RESIZE_WIDTH_INCREASE.name(), "resize-width-increase");
    }

    #[test]
    fn test_resize_width_decrease_id() {
        assert_eq!(RESIZE_WIDTH_DECREASE.module(), &MODULE);
        assert_eq!(RESIZE_WIDTH_DECREASE.name(), "resize-width-decrease");
    }

    #[test]
    fn test_resize_equal_id() {
        assert_eq!(RESIZE_EQUAL.module(), &MODULE);
        assert_eq!(RESIZE_EQUAL.name(), "resize-equal");
    }

    #[test]
    fn test_resize_max_height_id() {
        assert_eq!(RESIZE_MAX_HEIGHT.module(), &MODULE);
        assert_eq!(RESIZE_MAX_HEIGHT.name(), "resize-max-height");
    }

    #[test]
    fn test_resize_max_width_id() {
        assert_eq!(RESIZE_MAX_WIDTH.module(), &MODULE);
        assert_eq!(RESIZE_MAX_WIDTH.name(), "resize-max-width");
    }

    // =========================================================================
    // Window Movement IDs
    // =========================================================================

    #[test]
    fn test_move_window_left_id() {
        assert_eq!(MOVE_WINDOW_LEFT.module(), &MODULE);
        assert_eq!(MOVE_WINDOW_LEFT.name(), "move-window-left");
    }

    #[test]
    fn test_move_window_down_id() {
        assert_eq!(MOVE_WINDOW_DOWN.module(), &MODULE);
        assert_eq!(MOVE_WINDOW_DOWN.name(), "move-window-down");
    }

    #[test]
    fn test_move_window_up_id() {
        assert_eq!(MOVE_WINDOW_UP.module(), &MODULE);
        assert_eq!(MOVE_WINDOW_UP.name(), "move-window-up");
    }

    #[test]
    fn test_move_window_right_id() {
        assert_eq!(MOVE_WINDOW_RIGHT.module(), &MODULE);
        assert_eq!(MOVE_WINDOW_RIGHT.name(), "move-window-right");
    }

    #[test]
    fn test_rotate_windows_id() {
        assert_eq!(ROTATE_WINDOWS.module(), &MODULE);
        assert_eq!(ROTATE_WINDOWS.name(), "rotate-windows");
    }

    #[test]
    fn test_rotate_windows_reverse_id() {
        assert_eq!(ROTATE_WINDOWS_REVERSE.module(), &MODULE);
        assert_eq!(ROTATE_WINDOWS_REVERSE.name(), "rotate-windows-reverse");
    }

    #[test]
    fn test_swap_window_id() {
        assert_eq!(SWAP_WINDOW.module(), &MODULE);
        assert_eq!(SWAP_WINDOW.name(), "swap-window");
    }

    // =========================================================================
    // Tab Operations IDs
    // =========================================================================

    #[test]
    fn test_move_to_new_tab_id() {
        assert_eq!(MOVE_TO_NEW_TAB.module(), &MODULE);
        assert_eq!(MOVE_TO_NEW_TAB.name(), "move-to-new-tab");
    }

    // =========================================================================
    // Float Zone Operations IDs
    // =========================================================================

    #[test]
    fn test_toggle_float_id() {
        assert_eq!(TOGGLE_FLOAT.module(), &MODULE);
        assert_eq!(TOGGLE_FLOAT.name(), "toggle-float");
    }

    #[test]
    fn test_raise_float_id() {
        assert_eq!(RAISE_FLOAT.module(), &MODULE);
        assert_eq!(RAISE_FLOAT.name(), "raise-float");
    }

    #[test]
    fn test_lower_float_id() {
        assert_eq!(LOWER_FLOAT.module(), &MODULE);
        assert_eq!(LOWER_FLOAT.name(), "lower-float");
    }

    // =========================================================================
    // Uniqueness and equality
    // =========================================================================

    #[test]
    fn test_all_command_ids_are_unique() {
        let all_ids = [
            &FOCUS_LEFT,
            &FOCUS_DOWN,
            &FOCUS_UP,
            &FOCUS_RIGHT,
            &FOCUS_NEXT,
            &FOCUS_PREV,
            &SPLIT_HORIZONTAL,
            &SPLIT_VERTICAL,
            &SPLIT_NEW,
            &CLOSE_WINDOW,
            &CLOSE_OTHERS,
            &RESIZE_HEIGHT_INCREASE,
            &RESIZE_HEIGHT_DECREASE,
            &RESIZE_WIDTH_INCREASE,
            &RESIZE_WIDTH_DECREASE,
            &RESIZE_EQUAL,
            &RESIZE_MAX_HEIGHT,
            &RESIZE_MAX_WIDTH,
            &MOVE_WINDOW_LEFT,
            &MOVE_WINDOW_DOWN,
            &MOVE_WINDOW_UP,
            &MOVE_WINDOW_RIGHT,
            &ROTATE_WINDOWS,
            &ROTATE_WINDOWS_REVERSE,
            &SWAP_WINDOW,
            &MOVE_TO_NEW_TAB,
            &TOGGLE_FLOAT,
            &RAISE_FLOAT,
            &LOWER_FLOAT,
        ];
        // Check that all IDs are unique (no duplicates)
        for (i, a) in all_ids.iter().enumerate() {
            for (j, b) in all_ids.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "IDs at positions {i} and {j} are duplicates");
                }
            }
        }
    }

    #[test]
    fn test_all_command_ids_belong_to_window_ops_module() {
        let all_ids = [
            &FOCUS_LEFT,
            &FOCUS_DOWN,
            &FOCUS_UP,
            &FOCUS_RIGHT,
            &FOCUS_NEXT,
            &FOCUS_PREV,
            &SPLIT_HORIZONTAL,
            &SPLIT_VERTICAL,
            &SPLIT_NEW,
            &CLOSE_WINDOW,
            &CLOSE_OTHERS,
            &RESIZE_HEIGHT_INCREASE,
            &RESIZE_HEIGHT_DECREASE,
            &RESIZE_WIDTH_INCREASE,
            &RESIZE_WIDTH_DECREASE,
            &RESIZE_EQUAL,
            &RESIZE_MAX_HEIGHT,
            &RESIZE_MAX_WIDTH,
            &MOVE_WINDOW_LEFT,
            &MOVE_WINDOW_DOWN,
            &MOVE_WINDOW_UP,
            &MOVE_WINDOW_RIGHT,
            &ROTATE_WINDOWS,
            &ROTATE_WINDOWS_REVERSE,
            &SWAP_WINDOW,
            &MOVE_TO_NEW_TAB,
            &TOGGLE_FLOAT,
            &RAISE_FLOAT,
            &LOWER_FLOAT,
        ];
        for id in &all_ids {
            assert_eq!(
                id.module(),
                &MODULE,
                "Command '{}' does not belong to window-ops module",
                id.name()
            );
        }
    }

    #[test]
    fn test_command_id_equality() {
        // Two identical CommandIds should be equal
        let dup = CommandId::new(MODULE, "focus-left");
        assert_eq!(FOCUS_LEFT, dup);
    }

    #[test]
    fn test_command_id_inequality_different_name() {
        assert_ne!(FOCUS_LEFT, FOCUS_RIGHT);
    }

    #[test]
    fn test_command_id_inequality_different_module() {
        let other_module = ModuleId::new("other-module");
        let other_cmd = CommandId::new(other_module, "focus-left");
        assert_ne!(FOCUS_LEFT, other_cmd);
    }
}
