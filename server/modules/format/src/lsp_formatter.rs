//! LSP-based formatter.
//!
//! Sends `textDocument/formatting` requests to the active LSP server and
//! applies the returned `TextEdit` list to produce formatted content.

use std::{path::Path, sync::Arc, time::Duration};

use {
    lsp_types::{FormattingOptions, TextEdit},
    reovim_driver_formatter::{FormatError, FormatterProvider},
    reovim_driver_text_lsp::{LspKey, LspProvider, LspProviderRegistry, LspRequest, recv_response},
    reovim_kernel::api::v1::ServiceRegistry,
    tracing::warn,
};

/// Timeout for LSP formatting requests.
const LSP_TIMEOUT: Duration = Duration::from_secs(5);

/// Detect language from file extension for LSP key lookup.
pub fn language_from_path(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("js" | "jsx") => "javascript",
        Some("ts" | "tsx") => "typescript",
        Some("c" | "h") => "c",
        Some("cpp" | "hpp" | "cc" | "cxx") => "cpp",
        Some("go") => "go",
        Some("lua") => "lua",
        Some("json") => "json",
        Some("toml") => "toml",
        Some("md") => "markdown",
        _ => "unknown",
    }
}

/// Find an active LSP provider for the given file path.
// Needs real LSP registry — tested by integration tests.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn find_provider(
    services: &Arc<ServiceRegistry>,
    file_path: &str,
) -> Option<Arc<dyn LspProvider>> {
    let registry = services.get::<LspProviderRegistry>()?;

    let lang = language_from_path(file_path);
    if let Some(provider) = registry.get(&LspKey::Language(lang.to_owned()))
        && provider.is_active()
    {
        return Some(provider);
    }

    let provider = registry.get(&LspKey::Default)?;
    if provider.is_active() {
        Some(provider)
    } else {
        None
    }
}

/// Check if an LSP provider supports document formatting.
pub fn has_formatting_capability(provider: &dyn LspProvider) -> bool {
    provider
        .capabilities()
        .is_some_and(|caps| caps.document_formatting_provider.is_some())
}

/// Default formatting options (4-space indent, insert spaces).
#[must_use]
pub fn default_formatting_options() -> FormattingOptions {
    FormattingOptions {
        tab_size: 4,
        insert_spaces: true,
        ..FormattingOptions::default()
    }
}

/// Format buffer content using the LSP server.
///
/// Returns `None` if no active LSP provider supports formatting, or if the
/// request fails/times out. Failures are logged but do not propagate.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn format_with_lsp(
    content: &str,
    path: &str,
    services: &Arc<ServiceRegistry>,
) -> Option<String> {
    let provider = find_provider(services, path)?;
    if !has_formatting_capability(&*provider) {
        return None;
    }

    let uri = reovim_driver_text_lsp::uri_from_path(Path::new(path));
    let options = default_formatting_options();
    let (tx, rx) = reovim_kernel::api::v1::oneshot();

    if !provider.send_request(LspRequest::Formatting {
        uri,
        options,
        response_tx: tx,
    }) {
        warn!("format: LSP send_request failed");
        return None;
    }

    match recv_response(&rx, LSP_TIMEOUT) {
        Ok(Ok(Some(edits))) => {
            if edits.is_empty() {
                return None;
            }
            Some(apply_text_edits(content, &edits))
        }
        Ok(Ok(None)) => None,
        Ok(Err(e)) => {
            warn!(error = %e, "format: LSP formatting error");
            None
        }
        Err(e) => {
            warn!(error = %e, "format: LSP recv_response error");
            None
        }
    }
}

/// Apply LSP `TextEdit` list to produce formatted content.
///
/// Edits are sorted by position descending and applied in reverse order
/// to preserve byte offsets for earlier edits.
#[must_use]
pub fn apply_text_edits(content: &str, edits: &[TextEdit]) -> String {
    let mut sorted: Vec<&TextEdit> = edits.iter().collect();
    sorted.sort_by(|a, b| {
        b.range
            .start
            .line
            .cmp(&a.range.start.line)
            .then(b.range.start.character.cmp(&a.range.start.character))
    });

    let mut result = content.to_string();
    for edit in &sorted {
        let start = offset_from_position(&result, edit.range.start);
        let end = offset_from_position(&result, edit.range.end);
        result.replace_range(start..end, &edit.new_text);
    }
    result
}

/// Convert an LSP `Position` (line, character) to a byte offset.
fn offset_from_position(content: &str, pos: lsp_types::Position) -> usize {
    let target_line = pos.line as usize;
    let target_char = pos.character as usize;
    let mut offset = 0;

    for (i, line) in content.split('\n').enumerate() {
        if i == target_line {
            return offset + target_char.min(line.len());
        }
        offset += line.len() + 1; // +1 for newline
    }

    content.len()
}

/// LSP-based formatter implementing `FormatterProvider`.
///
/// Wraps an LSP provider to format via `textDocument/formatting`.
/// Lives in the module layer (not the driver) because it depends on
/// the LSP driver (cross-driver prohibition).
pub struct LspFormatter {
    services: Arc<ServiceRegistry>,
}

impl LspFormatter {
    /// Create a new LSP formatter.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Arc::new is not const
    pub fn new(services: Arc<ServiceRegistry>) -> Self {
        Self { services }
    }
}

impl FormatterProvider for LspFormatter {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn format(&self, content: &str, path: &Path) -> Result<String, FormatError> {
        let path_str = path.display().to_string();
        format_with_lsp(content, &path_str, &self.services)
            .ok_or_else(|| FormatError::LspError("no LSP formatting available".to_string()))
    }

    // Trait requires `&str`, literal return triggers clippy::unnecessary_literal_bound.
    #[allow(clippy::unnecessary_literal_bound)]
    fn name(&self) -> &str {
        "lsp"
    }

    fn supports_range(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[path = "lsp_formatter_tests.rs"]
mod tests;
