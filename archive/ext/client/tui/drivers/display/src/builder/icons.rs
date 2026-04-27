//! Mode and component icon constants.
//!
//! Provides Nerd Font icons for editor modes and UI components.
//! These icons are used in the status line, tab bar, and other UI elements.

/// Core mode icons (Nerd Font).
///
/// These icons represent the fundamental editing modes.
pub mod mode {
    /// Normal mode icon
    pub const NORMAL: &str = "󰆾 ";

    /// Insert mode icon
    pub const INSERT: &str = "󰙏 ";

    /// Visual mode icon
    pub const VISUAL: &str = "󰒉 ";

    /// Command mode icon
    pub const COMMAND: &str = "󰘳 ";

    /// Operator pending mode icon
    pub const OPERATOR: &str = "󰦒 ";

    /// Replace mode icon
    pub const REPLACE: &str = "󰛔 ";

    /// Select mode icon (visual block)
    pub const SELECT: &str = "󰒅 ";
}

/// Sub-mode icons for specialized editing states.
pub mod submode {
    /// Search mode icon
    pub const SEARCH: &str = "󰍉 ";

    /// Filter mode icon
    pub const FILTER: &str = "󰈲 ";

    /// Input/prompt mode icon
    pub const INPUT: &str = "󰗧 ";

    /// Window navigation mode icon
    pub const WINDOW: &str = "󱂬 ";

    /// Leap/jump mode icon
    pub const LEAP: &str = "󱐋 ";
}

/// Fallback icons when specific icons are not available.
pub mod fallback {
    /// Generic interactor/component icon
    pub const INTERACTOR: &str = "󰆾 ";

    /// Empty/no icon placeholder
    pub const NONE: &str = " ";

    /// Unknown component icon
    pub const UNKNOWN: &str = "? ";
}

/// UI component icons.
pub mod ui {
    /// File icon
    pub const FILE: &str = "󰈙 ";

    /// Folder icon
    pub const FOLDER: &str = "󰉋 ";

    /// Folder open icon
    pub const FOLDER_OPEN: &str = "󰝰 ";

    /// Git branch icon
    pub const GIT_BRANCH: &str = "󰊢 ";

    /// Error/diagnostic icon
    pub const ERROR: &str = "󰅚 ";

    /// Warning icon
    pub const WARNING: &str = "󰀪 ";

    /// Info icon
    pub const INFO: &str = "󰋽 ";

    /// Hint icon
    pub const HINT: &str = "󰌶 ";

    /// Modified indicator
    pub const MODIFIED: &str = "● ";

    /// Readonly indicator
    pub const READONLY: &str = "󰌾 ";
}

#[cfg(test)]
#[path = "icons_tests.rs"]
mod tests;
