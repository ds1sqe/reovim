//! Window Layout Module
//!
//! This module provides window tiling and focus navigation for reovim.
//! It implements the `LayoutPolicy` and `FocusPolicy` traits from the display driver.
//!
//! # Architecture
//!
//! Following the kernel's "mechanism vs policy" principle:
//! - **Mechanism** (display driver): `LayoutPolicy`, `FocusPolicy` traits
//! - **Policy** (this module): `TilingLayout`, `VimFocusPolicy` implementations
//!
//! # Components
//!
//! - [`TilingLayout`]: Window arrangement using a split tree
//! - [`VimFocusPolicy`]: Vim-style directional navigation (hjkl)
//! - [`SplitTree`]: Binary tree for managing window splits
//!
//! # Keybindings
//!
//! This module registers keybindings for window mode (after `<C-w>`):
//! - `h/j/k/l`: Move focus left/down/up/right
//! - `w/W`: Cycle focus forward/backward
//! - `s`: Horizontal split (`:split`)
//! - `v`: Vertical split (`:vsplit`)
//! - `c`: Close window (`:close`)
//! - `o`: Close other windows (`:only`)
//! - `+/-/=`: Resize windows
//!
//! # Example
//!
//! ```ignore
//! use reovim_module_layout::{TilingLayout, VimFocusPolicy};
//! use reovim_driver_display::{LayoutPolicy, FocusPolicy, SplitDirection};
//!
//! let mut layout = TilingLayout::new();
//! let w1 = layout.add_first_window();
//! let w2 = layout.split_vertical(w1).unwrap();
//!
//! let views = layout.arrange((80, 24), &[w1, w2]);
//!
//! let focus = VimFocusPolicy::new();
//! // Navigate from w1 to w2 (right)
//! let next = focus.next(NavigateDirection::Right, w1, &views);
//! assert_eq!(next, Some(w2));
//! ```

pub mod commands;
mod focus;
mod split;
mod tiling;

pub use {
    commands::all_commands,
    focus::VimFocusPolicy,
    split::{SplitNode, SplitTree},
    tiling::TilingLayout,
};

use reovim_kernel::api::v1::{
    KeybindingRegistration, Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    pr_info,
};

/// Window layout module.
///
/// Manages window tiling, splits, and focus navigation.
/// Provides keybindings for window mode operations.
pub struct LayoutModule;

impl LayoutModule {
    /// Create a new layout module.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for LayoutModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for LayoutModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("layout")
    }

    fn name(&self) -> &'static str {
        "Window Layout"
    }

    fn version(&self) -> Version {
        Version::new(0, 9, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        pr_info!("Layout module initialized");
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        pr_info!("Layout module exiting");
        Ok(())
    }

    fn keybindings(&self) -> Vec<KeybindingRegistration> {
        vec![
            // ====================================================================
            // Focus Navigation (window mode, after <C-w>)
            // ====================================================================
            KeybindingRegistration::new("h", "focus-left")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Move focus to left window"),
            KeybindingRegistration::new("j", "focus-down")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Move focus to window below"),
            KeybindingRegistration::new("k", "focus-up")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Move focus to window above"),
            KeybindingRegistration::new("l", "focus-right")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Move focus to right window"),
            // Arrow keys as aliases
            KeybindingRegistration::new("<Left>", "focus-left")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Move focus to left window"),
            KeybindingRegistration::new("<Down>", "focus-down")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Move focus to window below"),
            KeybindingRegistration::new("<Up>", "focus-up")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Move focus to window above"),
            KeybindingRegistration::new("<Right>", "focus-right")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Move focus to right window"),
            // ====================================================================
            // Focus Cycling
            // ====================================================================
            KeybindingRegistration::new("w", "focus-next")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Cycle focus to next window"),
            KeybindingRegistration::new("W", "focus-prev")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Cycle focus to previous window"),
            KeybindingRegistration::new("p", "focus-prev")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Go to previous (last accessed) window"),
            // ====================================================================
            // Window Splitting
            // ====================================================================
            KeybindingRegistration::new("s", "split-horizontal")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Split window horizontally (:split)"),
            KeybindingRegistration::new("v", "split-vertical")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Split window vertically (:vsplit)"),
            KeybindingRegistration::new("n", "split-new")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Create new window with empty buffer"),
            // ====================================================================
            // Window Closing
            // ====================================================================
            KeybindingRegistration::new("c", "close-window")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Close current window (:close)"),
            KeybindingRegistration::new("q", "close-window")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Close current window"),
            KeybindingRegistration::new("o", "close-others")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Close all other windows (:only)"),
            // ====================================================================
            // Window Resizing
            // ====================================================================
            KeybindingRegistration::new("+", "resize-height-increase")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Increase window height"),
            KeybindingRegistration::new("-", "resize-height-decrease")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Decrease window height"),
            KeybindingRegistration::new(">", "resize-width-increase")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Increase window width"),
            KeybindingRegistration::new("<", "resize-width-decrease")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Decrease window width"),
            KeybindingRegistration::new("=", "resize-equal")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Make all windows equal size"),
            KeybindingRegistration::new("_", "resize-max-height")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Maximize window height"),
            KeybindingRegistration::new("|", "resize-max-width")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Maximize window width"),
            // ====================================================================
            // Window Movement
            // ====================================================================
            KeybindingRegistration::new("H", "move-window-left")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Move window to far left"),
            KeybindingRegistration::new("J", "move-window-down")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Move window to bottom"),
            KeybindingRegistration::new("K", "move-window-up")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Move window to top"),
            KeybindingRegistration::new("L", "move-window-right")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Move window to far right"),
            KeybindingRegistration::new("r", "rotate-windows")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Rotate windows downwards"),
            KeybindingRegistration::new("R", "rotate-windows-reverse")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Rotate windows upwards"),
            KeybindingRegistration::new("x", "swap-window")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Exchange window with next"),
            // ====================================================================
            // Tab Operations (for future tab support)
            // ====================================================================
            KeybindingRegistration::new("T", "move-to-new-tab")
                .with_modes(&["window"])
                .with_category("window")
                .with_description("Move current window to new tab"),
        ]
    }
}

// Note: declare_module! is NOT used here because this module is statically linked.
// The macro generates FFI entry points with fixed symbol names that conflict when
// multiple modules are linked into the same binary. It's only needed for dynamic
// loading via libloading.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = LayoutModule::new();
        assert_eq!(module.id().as_str(), "layout");
    }

    #[test]
    fn test_module_name() {
        let module = LayoutModule::new();
        assert_eq!(module.name(), "Window Layout");
    }

    #[test]
    fn test_module_version() {
        let module = LayoutModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 9);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn test_keybindings_not_empty() {
        let module = LayoutModule::new();
        let bindings = module.keybindings();
        assert!(!bindings.is_empty());
    }

    #[test]
    fn test_keybindings_have_window_mode() {
        let module = LayoutModule::new();
        let bindings = module.keybindings();

        // All bindings should be in window mode
        for binding in &bindings {
            assert!(
                binding.modes.contains(&"window"),
                "Binding '{}' should be in window mode",
                binding.keys
            );
        }
    }

    #[test]
    fn test_keybindings_have_category() {
        let module = LayoutModule::new();
        let bindings = module.keybindings();

        // All bindings should have window category
        for binding in &bindings {
            assert_eq!(
                binding.category,
                Some("window"),
                "Binding '{}' should have window category",
                binding.keys
            );
        }
    }

    #[test]
    fn test_essential_keybindings_present() {
        let module = LayoutModule::new();
        let bindings = module.keybindings();

        let keys: Vec<_> = bindings.iter().map(|b| b.keys).collect();

        // Navigation
        assert!(keys.contains(&"h"), "Missing 'h' binding");
        assert!(keys.contains(&"j"), "Missing 'j' binding");
        assert!(keys.contains(&"k"), "Missing 'k' binding");
        assert!(keys.contains(&"l"), "Missing 'l' binding");

        // Cycling
        assert!(keys.contains(&"w"), "Missing 'w' binding");
        assert!(keys.contains(&"W"), "Missing 'W' binding");

        // Splitting
        assert!(keys.contains(&"s"), "Missing 's' binding");
        assert!(keys.contains(&"v"), "Missing 'v' binding");

        // Closing
        assert!(keys.contains(&"c"), "Missing 'c' binding");
        assert!(keys.contains(&"o"), "Missing 'o' binding");
    }
}
