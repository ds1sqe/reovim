//! Statusline context — immutable editor state snapshot.

use super::DiagnosticCounts;

/// Immutable snapshot of editor state for component rendering.
///
/// Created once per render cycle, shared by all data providers.
/// Providers should not perform I/O or blocking operations.
///
/// This is the display-agnostic counterpart of `ComponentContext`
/// in the display driver. No `Style` or `Color` types here.
#[derive(Debug, Clone, Default)]
pub struct ComponentDataContext {
    // === Mode ===
    /// Current mode name (e.g., "NORMAL", "INSERT", "VISUAL").
    pub mode: String,
    /// Mode subtype for visual modes (e.g., "CHAR", "LINE", "BLOCK").
    pub mode_subtype: Option<String>,

    // === Buffer ===
    /// Active buffer filename (basename only, e.g., "main.rs").
    pub filename: Option<String>,
    /// Full file path (e.g., "/home/user/project/src/main.rs").
    pub filepath: Option<String>,
    /// Buffer modified flag.
    pub modified: bool,
    /// Buffer readonly flag.
    pub readonly: bool,
    /// Filetype (e.g., "rust", "python", "markdown").
    pub filetype: Option<String>,

    // === Cursor ===
    /// Cursor line (1-indexed).
    pub line: usize,
    /// Cursor column (1-indexed, byte offset).
    pub column: usize,
    /// Total lines in buffer.
    pub total_lines: usize,

    // === Encoding ===
    /// File encoding (e.g., "utf-8", "latin1").
    pub encoding: String,
    /// Line ending style (e.g., "unix", "dos", "mac").
    pub line_ending: String,

    // === Window ===
    /// Terminal width.
    pub terminal_width: u16,
    /// Terminal height.
    pub terminal_height: u16,

    // === Extended (optional, provided by other modules) ===
    /// Git branch name (if git module provides).
    pub git_branch: Option<String>,
    /// Scope breadcrumb (if context module provides).
    pub breadcrumb: Option<String>,
    /// Diagnostic counts (if LSP module provides).
    pub diagnostics: Option<DiagnosticCounts>,
}
