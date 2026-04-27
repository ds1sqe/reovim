#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Text-domain capability identifiers.
//!
//! These constants are the single source of truth for capability strings
//! advertised by modules that implement text-domain trait surfaces (buffer
//! management, undo, search, syntax highlighting, snippets, completion,
//! formatting, LSP, git).
//!
//! Consumers import via
//! `reovim_domain_text_capabilities::<CONST>`.

/// Buffer lifecycle management (create, close, switch).
pub const BUFFER_MANAGER: &str = "buffer-manager";

/// Completion engine (popup, source aggregation).
pub const COMPLETION_PROVIDER: &str = "completion-provider";

/// File explorer sidebar. Reserved for future picker module; no current provider.
pub const FILE_EXPLORER: &str = "file-explorer";

/// Code formatting (format-on-save, external/LSP formatters).
pub const FORMATTER_PROVIDER: &str = "formatter-provider";

/// Fuzzy finder / picker orchestration. Reserved for future picker module; no current provider.
pub const FUZZY_FINDER: &str = "fuzzy-finder";

/// Git repository integration.
pub const GIT_PROVIDER: &str = "git-provider";

/// Language Server Protocol client.
pub const LSP_PROVIDER: &str = "lsp-provider";

/// Search and replace engine.
pub const SEARCH_PROVIDER: &str = "search-provider";

/// Snippet expansion engine.
pub const SNIPPET_PROVIDER: &str = "snippet-provider";

/// Syntax highlighting.
pub const SYNTAX_HIGHLIGHTING: &str = "syntax-highlighting";

/// Undo/redo history management.
pub const UNDO_PROVIDER: &str = "undo-provider";

/// All known text-domain capabilities, alphabetically sorted.
pub const ALL: &[&str] = &[
    BUFFER_MANAGER,
    COMPLETION_PROVIDER,
    FILE_EXPLORER,
    FORMATTER_PROVIDER,
    FUZZY_FINDER,
    GIT_PROVIDER,
    LSP_PROVIDER,
    SEARCH_PROVIDER,
    SNIPPET_PROVIDER,
    SYNTAX_HIGHLIGHTING,
    UNDO_PROVIDER,
];

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
