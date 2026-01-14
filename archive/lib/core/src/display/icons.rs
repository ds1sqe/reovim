//! Mode icons (Nerd Font) for status line display
//!
//! Provides icon constants for different editor modes.
//! These are centralized here so all display logic uses consistent icons.

/// Core mode icons for editor modes
pub mod core {
    /// Normal mode icon
    pub const NORMAL: &str = "󰆾 ";
    /// Insert mode icon
    pub const INSERT: &str = "󰙏 ";
    /// Visual mode icon
    pub const VISUAL: &str = "󰒉 ";
    /// Command mode icon
    pub const COMMAND: &str = "󰘳 ";
    /// Operator pending icon
    pub const OPERATOR: &str = "󰦒 ";
    /// Replace mode icon (for future use)
    #[allow(dead_code)]
    pub const REPLACE: &str = "󰛔 ";
}

/// Fallback icons for generic/unknown modes
pub mod fallback {
    /// Generic interactor icon (used for plugins and unknown sub-modes)
    pub const INTERACTOR: &str = "󰆾 ";
    /// Empty icon (used when no icon is registered)
    pub const NONE: &str = " ";
}
