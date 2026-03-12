#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Shared module capability identifiers.
//!
//! These constants are the single source of truth for capability strings
//! used by server modules via [`Module::provides()`] and [`Module::requires()`].
//!
//! Capabilities enable abstract dependency matching: a module that `requires`
//! `"syntax-highlighting"` is satisfied by any module that `provides` it,
//! without hard-coding a specific module ID. This enables substitutability
//! (e.g., swapping one syntax provider for another).
//!
//! # Adding a new capability
//!
//! 1. Add a constant here
//! 2. Add it to the [`ALL`] array (maintain alphabetical order)
//! 3. Use it in module `provides()` / `requires()` overrides

/// Buffer lifecycle management (create, close, switch).
pub const BUFFER_MANAGER: &str = "buffer-manager";

/// System clipboard integration.
pub const CLIPBOARD_PROVIDER: &str = "clipboard-provider";

/// Command dispatch and registration.
pub const COMMAND_DISPATCH: &str = "command-dispatch";

/// Completion engine (popup, source aggregation).
pub const COMPLETION_PROVIDER: &str = "completion-provider";

/// File explorer sidebar.
pub const FILE_EXPLORER: &str = "file-explorer";

/// Fuzzy finder / picker orchestration.
pub const FUZZY_FINDER: &str = "fuzzy-finder";

/// Git repository integration.
pub const GIT_PROVIDER: &str = "git-provider";

/// Language Server Protocol client.
pub const LSP_PROVIDER: &str = "lsp-provider";

/// Mode management and keybinding personality (vim, emacs, kakoune).
pub const MODE_MANAGEMENT: &str = "mode-management";

/// Motion commands (cursor movement).
pub const MOTION_COMMANDS: &str = "motion-commands";

/// Search and replace engine.
pub const SEARCH_PROVIDER: &str = "search-provider";

/// Snippet expansion engine.
pub const SNIPPET_PROVIDER: &str = "snippet-provider";

/// Syntax highlighting.
pub const SYNTAX_HIGHLIGHTING: &str = "syntax-highlighting";

/// Undo/redo history management.
pub const UNDO_PROVIDER: &str = "undo-provider";

/// Virtual filesystem abstraction.
pub const VFS_PROVIDER: &str = "vfs-provider";

/// All known capabilities, alphabetically sorted.
///
/// Useful for validation and debugging.
pub const ALL: &[&str] = &[
    BUFFER_MANAGER,
    CLIPBOARD_PROVIDER,
    COMMAND_DISPATCH,
    COMPLETION_PROVIDER,
    FILE_EXPLORER,
    FUZZY_FINDER,
    GIT_PROVIDER,
    LSP_PROVIDER,
    MODE_MANAGEMENT,
    MOTION_COMMANDS,
    SEARCH_PROVIDER,
    SNIPPET_PROVIDER,
    SYNTAX_HIGHLIGHTING,
    UNDO_PROVIDER,
    VFS_PROVIDER,
];

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
