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

#[cfg(test)]
mod tests {
    use super::*;

    // ========== OptionScope tests ==========

    #[test]
    fn test_option_scope_default() {
        assert_eq!(OptionScope::default(), OptionScope::Global);
    }

    #[test]
    fn test_option_scope_display_names() {
        assert_eq!(OptionScope::Global.display_name(), "global");
        assert_eq!(OptionScope::Buffer.display_name(), "buffer");
        assert_eq!(OptionScope::Window.display_name(), "window");
    }

    #[test]
    fn test_option_scope_display() {
        assert_eq!(format!("{}", OptionScope::Global), "global");
        assert_eq!(format!("{}", OptionScope::Buffer), "buffer");
        assert_eq!(format!("{}", OptionScope::Window), "window");
    }

    #[test]
    fn test_option_scope_debug() {
        let debug = format!("{:?}", OptionScope::Global);
        assert_eq!(debug, "Global");
    }

    #[test]
    fn test_option_scope_clone() {
        let scope = OptionScope::Buffer;
        let cloned = scope;
        assert_eq!(scope, cloned);
    }

    #[test]
    fn test_option_scope_eq() {
        assert_eq!(OptionScope::Global, OptionScope::Global);
        assert_ne!(OptionScope::Global, OptionScope::Buffer);
        assert_ne!(OptionScope::Buffer, OptionScope::Window);
    }

    #[test]
    fn test_option_scope_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(OptionScope::Global);
        set.insert(OptionScope::Buffer);
        set.insert(OptionScope::Window);
        set.insert(OptionScope::Global); // duplicate
        assert_eq!(set.len(), 3);
    }

    // ========== OptionScopeId tests ==========

    #[test]
    fn test_option_scope_id_default() {
        assert_eq!(OptionScopeId::default(), OptionScopeId::Global);
    }

    #[test]
    fn test_option_scope_id_display_global() {
        assert_eq!(format!("{}", OptionScopeId::Global), "global");
    }

    #[test]
    fn test_option_scope_id_display_buffer() {
        let id = BufferId::from_raw(42);
        let display = format!("{}", OptionScopeId::Buffer(id));
        assert!(display.contains("buffer"));
    }

    #[test]
    fn test_option_scope_id_display_window() {
        let id = WindowId::from_raw(7);
        let display = format!("{}", OptionScopeId::Window(id));
        assert!(display.contains("window"));
    }

    #[test]
    fn test_option_scope_id_debug() {
        let scope_id = OptionScopeId::Global;
        let debug = format!("{scope_id:?}");
        assert_eq!(debug, "Global");
    }

    #[test]
    fn test_option_scope_id_eq() {
        let id = BufferId::from_raw(1);
        assert_eq!(OptionScopeId::Global, OptionScopeId::Global);
        assert_eq!(OptionScopeId::Buffer(id), OptionScopeId::Buffer(id));
        assert_ne!(OptionScopeId::Global, OptionScopeId::Buffer(id));
    }

    #[test]
    fn test_option_scope_id_hash() {
        use std::collections::HashSet;
        let id1 = BufferId::from_raw(1);
        let id2 = BufferId::from_raw(2);
        let mut set = HashSet::new();
        set.insert(OptionScopeId::Global);
        set.insert(OptionScopeId::Buffer(id1));
        set.insert(OptionScopeId::Buffer(id2));
        set.insert(OptionScopeId::Global); // duplicate
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_option_scope_id_clone() {
        let id = BufferId::from_raw(5);
        let scope_id = OptionScopeId::Buffer(id);
        let cloned = scope_id;
        assert_eq!(scope_id, cloned);
    }
}
