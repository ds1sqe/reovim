//! Option scope types.
//!
//! Defines where an option applies (global, buffer, window).

use std::fmt;

use crate::mm::{BufferId, WindowId};

/// Scope where an option applies.
///
/// Determines the granularity of option storage:
/// - `Global`: Single value for the entire editor
/// - `Buffer`: Per-buffer values (e.g., `tabwidth`)
/// - `Window`: Per-window values (e.g., `number`)
///
/// Window-scoped options can also have buffer-local defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OptionScope {
    /// Applies globally to the entire editor.
    #[default]
    Global,

    /// Per-buffer setting (e.g., `filetype`, `tabwidth`, `expandtab`).
    Buffer,

    /// Per-window setting (e.g., `number`, `relativenumber`, `wrap`).
    Window,
}

impl OptionScope {
    /// Get display name for this scope.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Buffer => "buffer",
            Self::Window => "window",
        }
    }
}

impl fmt::Display for OptionScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Runtime scope identifier for option access.
///
/// Used when getting or setting option values to specify the exact
/// scope context (which buffer, which window, or global).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OptionScopeId {
    /// Global scope (no buffer/window context).
    #[default]
    Global,
    /// Buffer-local scope with specific buffer ID.
    Buffer(BufferId),
    /// Window-local scope with specific window ID.
    Window(WindowId),
}

impl fmt::Display for OptionScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Global => write!(f, "global"),
            Self::Buffer(id) => write!(f, "buffer({id:?})"),
            Self::Window(id) => write!(f, "window({id:?})"),
        }
    }
}

