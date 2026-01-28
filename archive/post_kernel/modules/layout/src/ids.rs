//! Command ID constants for the layout module.
//!
//! These constants enable compile-time verification of command IDs
//! referenced in keybindings.

use reovim_kernel::api::v1::{CommandId, ModuleId};

/// Layout module ID.
pub const MODULE: ModuleId = ModuleId::new("layout");

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
// Float Zone Operations (#398)
// =============================================================================

/// Toggle window between tiled and floating zones.
pub const TOGGLE_FLOAT: CommandId = CommandId::new(MODULE, "toggle-float");

/// Raise floating window to front of float zone.
pub const RAISE_FLOAT: CommandId = CommandId::new(MODULE, "raise-float");

/// Lower floating window to back of float zone.
pub const LOWER_FLOAT: CommandId = CommandId::new(MODULE, "lower-float");
