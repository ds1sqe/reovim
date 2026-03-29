use std::path::PathBuf;

/// A syntax highlight span in preview content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewHighlight {
    /// Line number (0-indexed).
    pub line: u16,
    /// Start column (0-indexed, byte offset within line).
    pub col_start: u16,
    /// End column (exclusive, byte offset within line).
    pub col_end: u16,
    /// Syntax category (e.g. "keyword", "string", "comment").
    pub category: String,
}

/// Preview content for the selected item.
#[derive(Debug, Clone, Default)]
pub struct PreviewContent {
    /// Lines of the preview.
    pub lines: Vec<String>,
    /// Line to highlight (e.g. grep match line, 0-indexed).
    pub highlight_line: Option<usize>,
    /// File path hint for syntax detection.
    pub file_path: Option<PathBuf>,
    /// Syntax highlight spans.
    pub highlights: Vec<PreviewHighlight>,
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
