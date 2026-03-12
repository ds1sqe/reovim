use std::path::PathBuf;

/// Preview content for the selected item.
#[derive(Debug, Clone, Default)]
pub struct PreviewContent {
    /// Lines of the preview.
    pub lines: Vec<String>,
    /// Line to highlight (e.g. grep match line, 0-indexed).
    pub highlight_line: Option<usize>,
    /// File path hint for syntax detection.
    pub file_path: Option<PathBuf>,
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
