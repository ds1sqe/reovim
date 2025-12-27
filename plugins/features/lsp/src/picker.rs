//! LSP Pickers.
//!
//! Displays LSP results (references, definitions) in microscope pickers for navigation.

use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

use {
    arc_swap::ArcSwap,
    reovim_lsp::Location,
    reovim_plugin_microscope::{
        MicroscopeAction, MicroscopeData, MicroscopeItem, Picker, PickerContext, PreviewContent,
    },
};

// ============================================================================
// Definitions Picker
// ============================================================================

/// Picker for displaying LSP definitions.
///
/// When goto definition returns multiple locations, this picker
/// allows the user to choose which definition to navigate to.
pub struct LspDefinitionsPicker {
    /// Cached definitions from LSP response.
    definitions: ArcSwap<Vec<Location>>,
}

impl LspDefinitionsPicker {
    /// Create a new LSP definitions picker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            definitions: ArcSwap::new(Arc::new(Vec::new())),
        }
    }

    /// Set definitions from LSP response.
    pub fn set_definitions(&self, locations: Vec<Location>) {
        self.definitions.store(Arc::new(locations));
    }

    /// Clear stored definitions.
    #[allow(dead_code)]
    pub fn clear(&self) {
        self.definitions.store(Arc::new(Vec::new()));
    }
}

impl Default for LspDefinitionsPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for LspDefinitionsPicker {
    fn name(&self) -> &'static str {
        "lsp_definitions"
    }

    fn title(&self) -> &'static str {
        "Definitions"
    }

    fn prompt(&self) -> &'static str {
        "Definition> "
    }

    fn fetch(
        &self,
        _ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Vec<MicroscopeItem>> + Send + '_>> {
        let defs = Arc::clone(&self.definitions.load());

        Box::pin(async move {
            defs.iter()
                .enumerate()
                .filter_map(|(idx, loc)| {
                    let path = uri_to_path(&loc.uri)?;
                    let filename = path
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("unknown");
                    let line = loc.range.start.line as usize;
                    let col = loc.range.start.character as usize;

                    Some(
                        MicroscopeItem::new(
                            format!("def_{idx}"),
                            format!("{filename}:{}", line + 1),
                            MicroscopeData::GrepMatch {
                                path: path.clone(),
                                line,
                                col,
                            },
                            "lsp_definitions",
                        )
                        .with_detail(path.to_string_lossy().to_string())
                        .with_icon('󰊕'),
                    )
                })
                .collect()
        })
    }

    fn on_select(&self, item: &MicroscopeItem) -> MicroscopeAction {
        match &item.data {
            MicroscopeData::GrepMatch { path, line, col } => MicroscopeAction::GotoLocation {
                path: path.clone(),
                line: *line,
                col: *col,
            },
            _ => MicroscopeAction::Nothing,
        }
    }

    fn preview(
        &self,
        item: &MicroscopeItem,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        let data = item.data.clone();

        Box::pin(async move {
            let MicroscopeData::GrepMatch { path, line, .. } = data else {
                return None;
            };

            // Read file and extract context around the definition
            let Ok(content) = std::fs::read_to_string(&path) else {
                return None;
            };

            let lines: Vec<String> = content.lines().map(String::from).collect();
            let total_lines = lines.len();

            // Calculate preview window (5 lines before and after)
            let start = line.saturating_sub(5);
            let end = (line + 6).min(total_lines);
            let preview_lines: Vec<String> = lines[start..end].to_vec();

            // Calculate highlight line (relative to preview window)
            let highlight_line = if line >= 5 { 5 } else { line };

            Some(
                PreviewContent::new(preview_lines)
                    .with_highlight_line(highlight_line)
                    .with_title(path.to_string_lossy().to_string()),
            )
        })
    }
}

// ============================================================================
// References Picker
// ============================================================================

/// Picker for displaying LSP references.
///
/// Stores references from the last `textDocument/references` response
/// and displays them in the microscope picker.
pub struct LspReferencesPicker {
    /// Cached references from LSP response.
    references: ArcSwap<Vec<Location>>,
}

impl LspReferencesPicker {
    /// Create a new LSP references picker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            references: ArcSwap::new(Arc::new(Vec::new())),
        }
    }

    /// Set references from LSP response.
    pub fn set_references(&self, locations: Vec<Location>) {
        self.references.store(Arc::new(locations));
    }

    /// Clear stored references.
    #[allow(dead_code)]
    pub fn clear(&self) {
        self.references.store(Arc::new(Vec::new()));
    }
}

impl Default for LspReferencesPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for LspReferencesPicker {
    fn name(&self) -> &'static str {
        "lsp_references"
    }

    fn title(&self) -> &'static str {
        "References"
    }

    fn prompt(&self) -> &'static str {
        "References> "
    }

    fn fetch(
        &self,
        _ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Vec<MicroscopeItem>> + Send + '_>> {
        let refs = Arc::clone(&self.references.load());

        Box::pin(async move {
            refs.iter()
                .enumerate()
                .filter_map(|(idx, loc)| {
                    let path = uri_to_path(&loc.uri)?;
                    let filename = path
                        .file_name()
                        .and_then(|f| f.to_str())
                        .unwrap_or("unknown");
                    let line = loc.range.start.line as usize;
                    let col = loc.range.start.character as usize;

                    Some(
                        MicroscopeItem::new(
                            format!("ref_{idx}"),
                            format!("{filename}:{}", line + 1),
                            MicroscopeData::GrepMatch {
                                path: path.clone(),
                                line,
                                col,
                            },
                            "lsp_references",
                        )
                        .with_detail(path.to_string_lossy().to_string())
                        .with_icon('󰏫'),
                    )
                })
                .collect()
        })
    }

    fn on_select(&self, item: &MicroscopeItem) -> MicroscopeAction {
        match &item.data {
            MicroscopeData::GrepMatch { path, line, col } => MicroscopeAction::GotoLocation {
                path: path.clone(),
                line: *line,
                col: *col,
            },
            _ => MicroscopeAction::Nothing,
        }
    }

    fn preview(
        &self,
        item: &MicroscopeItem,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        let data = item.data.clone();

        Box::pin(async move {
            let MicroscopeData::GrepMatch { path, line, .. } = data else {
                return None;
            };

            // Read file and extract context around the reference
            let Ok(content) = std::fs::read_to_string(&path) else {
                return None;
            };

            let lines: Vec<String> = content.lines().map(String::from).collect();
            let total_lines = lines.len();

            // Calculate preview window (5 lines before and after)
            let start = line.saturating_sub(5);
            let end = (line + 6).min(total_lines);
            let preview_lines: Vec<String> = lines[start..end].to_vec();

            // Calculate highlight line (relative to preview window)
            let highlight_line = if line >= 5 { 5 } else { line };

            Some(
                PreviewContent::new(preview_lines)
                    .with_highlight_line(highlight_line)
                    .with_title(path.to_string_lossy().to_string()),
            )
        })
    }
}

/// Convert a file:// URI to a filesystem path.
fn uri_to_path(uri: &reovim_lsp::Uri) -> Option<PathBuf> {
    let uri_str = uri.as_str();
    uri_str.strip_prefix("file://").map(|path_str| {
        // Handle percent-encoded characters (basic: %20 -> space)
        let decoded = path_str.replace("%20", " ");
        PathBuf::from(decoded)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_location() -> Location {
        let uri: reovim_lsp::Uri = "file:///test/file.rs"
            .parse()
            .expect("test URI should parse");
        Location {
            uri,
            range: reovim_lsp::Range {
                start: reovim_lsp::Position {
                    line: 10,
                    character: 5,
                },
                end: reovim_lsp::Position {
                    line: 10,
                    character: 10,
                },
            },
        }
    }

    // ========================================================================
    // References Picker Tests
    // ========================================================================

    #[test]
    fn test_references_picker_name() {
        let picker = LspReferencesPicker::new();
        assert_eq!(picker.name(), "lsp_references");
        assert_eq!(picker.title(), "References");
    }

    #[test]
    fn test_set_and_clear_references() {
        let picker = LspReferencesPicker::new();
        assert!(picker.references.load().is_empty());

        picker.set_references(vec![test_location()]);
        assert_eq!(picker.references.load().len(), 1);

        picker.clear();
        assert!(picker.references.load().is_empty());
    }

    // ========================================================================
    // Definitions Picker Tests
    // ========================================================================

    #[test]
    fn test_definitions_picker_name() {
        let picker = LspDefinitionsPicker::new();
        assert_eq!(picker.name(), "lsp_definitions");
        assert_eq!(picker.title(), "Definitions");
    }

    #[test]
    fn test_set_and_clear_definitions() {
        let picker = LspDefinitionsPicker::new();
        assert!(picker.definitions.load().is_empty());

        picker.set_definitions(vec![test_location()]);
        assert_eq!(picker.definitions.load().len(), 1);

        picker.clear();
        assert!(picker.definitions.load().is_empty());
    }
}
