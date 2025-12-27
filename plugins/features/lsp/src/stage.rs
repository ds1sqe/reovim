//! LSP render stage for document synchronization and diagnostics display.
//!
//! This stage runs during the render pipeline and:
//! - Checks for pending document syncs (debounce elapsed)
//! - Accesses buffer content from `RenderContext`
//! - Sends didOpen/didChange to the LSP saturator
//! - Applies diagnostic highlights (underlines) to rendered lines

use std::{path::PathBuf, sync::Arc};

use {
    reovim_core::{
        component::RenderContext,
        highlight::Style,
        render::{LineHighlight, RenderData, RenderStage},
    },
    reovim_lsp::DiagnosticSeverity,
    tracing::{debug, info},
};

use crate::SharedLspManager;

/// LSP render stage - handles document synchronization.
///
/// Runs during rendering to check for ready syncs and send
/// buffer content to the LSP server.
pub struct LspRenderStage {
    manager: Arc<SharedLspManager>,
}

impl LspRenderStage {
    /// Create a new LSP render stage.
    pub const fn new(manager: Arc<SharedLspManager>) -> Self {
        Self { manager }
    }

    /// Ensure document is registered for LSP tracking.
    ///
    /// Called during render to handle race condition where `FileOpened` event
    /// may not have been processed yet by the async `EventBus`. This provides
    /// a synchronous fallback to register documents before user interaction.
    fn ensure_document_registered(&self, buffer_id: usize, ctx: &RenderContext<'_>) {
        // Skip if already registered
        if self.manager.with(|m| m.documents.has_document(buffer_id)) {
            return;
        }

        // Get buffer info from render context
        let Some(state) = ctx.state else { return };
        let Some(buffer) = state.buffers.get(&buffer_id) else {
            return;
        };
        let Some(ref path_str) = buffer.file_path else {
            return;
        };

        let path = PathBuf::from(path_str);

        self.manager.with_mut(|m| {
            if let Some(_doc) = m.documents.open_document(buffer_id, path) {
                // Schedule immediate sync - render stage will send didOpen
                m.documents.schedule_immediate_sync(buffer_id);
                info!(buffer_id, "LSP stage: registered document synchronously");
            }
        });
    }

    /// Process pending syncs and send to LSP server.
    fn process_syncs(&self, ctx: &RenderContext<'_>) {
        // Get ready syncs (debounce elapsed)
        let ready_syncs = self.manager.with_mut(|m| m.documents.get_ready_syncs());

        if ready_syncs.is_empty() {
            return;
        }

        debug!(count = ready_syncs.len(), "LSP stage: processing ready syncs");

        // Process each ready buffer
        for buffer_id in ready_syncs {
            // Try to get buffer content from state, or fall back to disk
            if let Some(state) = ctx.state {
                self.sync_buffer(buffer_id, Some(state.buffers));
            } else {
                // State not available - sync using file content from disk
                self.sync_buffer(buffer_id, None);
            }
        }
    }

    /// Sync a single buffer to the LSP server.
    fn sync_buffer(
        &self,
        buffer_id: usize,
        buffers: Option<&std::collections::BTreeMap<usize, reovim_core::buffer::Buffer>>,
    ) {
        // Get content: from buffer if available, or from disk as fallback
        let content = if let Some(buffers) = buffers {
            if let Some(buffer) = buffers.get(&buffer_id) {
                buffer
                    .contents
                    .iter()
                    .map(|line| line.inner.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                debug!(buffer_id, "LSP: buffer not found for sync");
                return;
            }
        } else {
            // No state - read from disk
            let path = self
                .manager
                .with(|m| m.documents.get(buffer_id).map(|d| d.path.clone()));

            let Some(path) = path else {
                debug!(buffer_id, "LSP: document not tracked");
                return;
            };

            match std::fs::read_to_string(&path) {
                Ok(content) => content,
                Err(e) => {
                    debug!(buffer_id, error = %e, "LSP: failed to read file for sync");
                    return;
                }
            }
        };

        self.manager.with_mut(|m| {
            let Some(doc) = m.documents.get(buffer_id) else {
                debug!(buffer_id, "LSP: document not tracked");
                return;
            };

            let Some(handle) = &m.handle else {
                debug!(buffer_id, "LSP: server not running, re-scheduling sync");
                // Re-schedule sync for next render cycle
                m.documents.schedule_immediate_sync(buffer_id);
                return;
            };

            let uri = doc.uri.clone();
            let version = doc.version;
            let language_id = doc.language_id.clone();

            if doc.opened {
                // Subsequent sync - send didChange
                debug!(
                    buffer_id,
                    version,
                    uri = %uri.as_str(),
                    "LSP: sending didChange"
                );
                handle.did_change(uri, version, content);
            } else {
                // First sync - send didOpen
                debug!(
                    buffer_id,
                    version,
                    language_id = %language_id,
                    uri = %uri.as_str(),
                    "LSP: sending didOpen"
                );
                handle.did_open(uri, language_id, version, content);
                // Mark as opened only after successfully sending didOpen
                m.documents.mark_opened(buffer_id);
            }
        });
    }

    /// Apply diagnostic highlights to render data.
    fn apply_diagnostics(&self, input: &mut RenderData, ctx: &RenderContext<'_>) {
        let buffer_id = input.buffer_id;

        // Get document URI for this buffer
        let uri = self
            .manager
            .with(|m| m.documents.get(buffer_id).map(|d| d.uri.clone()));

        let Some(uri) = uri else {
            return;
        };

        // Get diagnostics from cache
        let diagnostics = self.manager.with(|m| {
            m.cache
                .as_ref()
                .and_then(|cache| cache.get(&uri))
                .map(|bd| bd.diagnostics)
        });

        let Some(diagnostics) = diagnostics else {
            return;
        };

        if diagnostics.is_empty() {
            return;
        }

        // Get diagnostic styles from theme
        let diag_styles = &ctx.theme.diagnostic;

        // Apply each diagnostic as a highlight
        for diagnostic in &diagnostics {
            let start_line = diagnostic.range.start.line as usize;
            let end_line = diagnostic.range.end.line as usize;
            let start_col = diagnostic.range.start.character as usize;
            let end_col = diagnostic.range.end.character as usize;

            // Get style based on severity
            let style = severity_to_style(diagnostic.severity, diag_styles);

            // Handle single-line and multi-line diagnostics
            if start_line == end_line {
                // Single-line diagnostic
                if start_line < input.highlights.len() {
                    input.highlights[start_line].push(LineHighlight {
                        start_col,
                        end_col,
                        style,
                    });
                }
            } else {
                // Multi-line diagnostic - highlight each line
                for line in start_line..=end_line {
                    if line >= input.highlights.len() {
                        break;
                    }

                    let (s_col, e_col) = if line == start_line {
                        // First line: from start_col to end of line
                        let line_len = input.lines.get(line).map_or(0, String::len);
                        (start_col, line_len)
                    } else if line == end_line {
                        // Last line: from start to end_col
                        (0, end_col)
                    } else {
                        // Middle lines: entire line
                        let line_len = input.lines.get(line).map_or(0, String::len);
                        (0, line_len)
                    };

                    input.highlights[line].push(LineHighlight {
                        start_col: s_col,
                        end_col: e_col,
                        style: style.clone(),
                    });
                }
            }
        }

        debug!(buffer_id, count = diagnostics.len(), "LSP: applied diagnostic highlights");
    }
}

/// Convert diagnostic severity to a style.
fn severity_to_style(
    severity: Option<DiagnosticSeverity>,
    styles: &reovim_core::highlight::DiagnosticStyles,
) -> Style {
    match severity {
        Some(DiagnosticSeverity::ERROR) => styles.error.clone(),
        Some(DiagnosticSeverity::WARNING) => styles.warn.clone(),
        Some(DiagnosticSeverity::HINT) => styles.hint.clone(),
        // INFORMATION and unknown severities default to info style
        _ => styles.info.clone(),
    }
}

impl RenderStage for LspRenderStage {
    fn transform(&self, mut input: RenderData, ctx: &RenderContext<'_>) -> RenderData {
        let buffer_id = input.buffer_id;

        // Ensure document is registered (handles async event race condition)
        self.ensure_document_registered(buffer_id, ctx);

        // Check for pending syncs and send to LSP
        self.process_syncs(ctx);

        // Apply diagnostic highlights
        self.apply_diagnostics(&mut input, ctx);

        input
    }

    fn name(&self) -> &'static str {
        "lsp"
    }
}
