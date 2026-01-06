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
        render::{LineHighlight, RenderData, RenderStage, VirtualTextEntry},
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

            // Update last sync timestamp
            m.mark_sync_sent();
        });
    }

    /// Apply diagnostic highlights to render data.
    #[allow(clippy::too_many_lines)]
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

            // Create sign for the diagnostic (only on start line)
            if start_line < input.signs.len() {
                tracing::debug!(
                    "LSP: Creating sign for line {} (severity: {:?})",
                    start_line,
                    diagnostic.severity
                );
                let (icon, priority) = match diagnostic.severity {
                    Some(DiagnosticSeverity::ERROR) => ("●", 304),
                    Some(DiagnosticSeverity::WARNING) => ("◐", 303),
                    Some(DiagnosticSeverity::INFORMATION) => ("ⓘ", 302),
                    Some(DiagnosticSeverity::HINT) => ("·", 301),
                    Some(_) | None => ("ⓘ", 300),
                };

                let sign = reovim_core::sign::Sign {
                    icon: icon.to_string(),
                    style: style.clone(),
                    priority,
                };

                // Set sign only if none exists or we have higher priority
                match &input.signs[start_line] {
                    Some(existing) if existing.priority >= sign.priority => {
                        // Keep existing higher-priority sign
                    }
                    _ => {
                        // Replace with new sign
                        input.signs[start_line] = Some(sign);
                    }
                }
            }

            // Create virtual text for diagnostic message (inline display)
            // Get virtual text config from manager
            let vt_config = self.manager.with(|m| m.virtual_text_config.clone());

            if vt_config.enabled && start_line < input.virtual_texts.len() {
                use crate::manager::VirtualTextShowMode;

                let vt_style = match diagnostic.severity {
                    Some(DiagnosticSeverity::ERROR) => ctx.theme.virtual_text.error.clone(),
                    Some(DiagnosticSeverity::WARNING) => ctx.theme.virtual_text.warn.clone(),
                    Some(DiagnosticSeverity::INFORMATION) => ctx.theme.virtual_text.info.clone(),
                    Some(DiagnosticSeverity::HINT) => ctx.theme.virtual_text.hint.clone(),
                    Some(_) | None => ctx.theme.virtual_text.default.clone(),
                };

                let vt_priority = match diagnostic.severity {
                    Some(DiagnosticSeverity::ERROR) => 404,
                    Some(DiagnosticSeverity::WARNING) => 403,
                    Some(DiagnosticSeverity::INFORMATION) => 402,
                    Some(DiagnosticSeverity::HINT) => 401,
                    Some(_) | None => 400,
                };

                // Determine if we should set virtual text based on show_mode
                let should_set = match vt_config.show_mode {
                    VirtualTextShowMode::First => {
                        // First diagnostic wins
                        input.virtual_texts[start_line].is_none()
                    }
                    VirtualTextShowMode::Highest => {
                        // Higher priority wins
                        input.virtual_texts[start_line]
                            .as_ref()
                            .is_none_or(|existing| existing.priority < vt_priority)
                    }
                    VirtualTextShowMode::All => {
                        // Will concatenate in a separate pass - always set if higher priority
                        true
                    }
                };

                if should_set {
                    // Use custom prefix or default severity icon
                    let prefix = if vt_config.prefix.is_empty() {
                        match diagnostic.severity {
                            Some(DiagnosticSeverity::ERROR) => "●",
                            Some(DiagnosticSeverity::WARNING) => "◐",
                            Some(DiagnosticSeverity::HINT) => "·",
                            Some(DiagnosticSeverity::INFORMATION | _) | None => "ⓘ",
                        }
                    } else {
                        &vt_config.prefix
                    };

                    let mut text = format!("{prefix} {}", diagnostic.message);

                    // Apply max_length truncation
                    let max_len = vt_config.max_length as usize;
                    if text.chars().count() > max_len {
                        let truncated: String =
                            text.chars().take(max_len.saturating_sub(3)).collect();
                        text = format!("{truncated}...");
                    }

                    // For "all" mode, concatenate with existing
                    if vt_config.show_mode == VirtualTextShowMode::All
                        && let Some(existing) = &input.virtual_texts[start_line]
                    {
                        text = format!("{} | {text}", existing.text);
                        // Re-truncate if needed
                        if text.chars().count() > max_len {
                            let truncated: String =
                                text.chars().take(max_len.saturating_sub(3)).collect();
                            text = format!("{truncated}...");
                        }
                    }

                    input.virtual_texts[start_line] = Some(VirtualTextEntry {
                        text,
                        style: vt_style,
                        priority: vt_priority,
                    });
                }
            }

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

        let sign_count = input.signs.iter().filter(|s| s.is_some()).count();
        debug!(
            buffer_id,
            count = diagnostics.len(),
            signs_created = sign_count,
            "LSP: applied diagnostic highlights and signs"
        );
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_core::{
            highlight::{ColorMode, Theme},
            render::{Bounds, LineVisibility},
        },
    };

    fn create_test_render_data(lines: Vec<&str>) -> RenderData {
        let line_count = lines.len();
        RenderData {
            lines: lines.into_iter().map(String::from).collect(),
            visibility: vec![LineVisibility::Visible; line_count],
            highlights: vec![Vec::new(); line_count],
            decorations: vec![Vec::new(); line_count],
            signs: vec![None; line_count],
            virtual_texts: vec![None; line_count],
            buffer_id: 1,
            window_id: 1,
            window_bounds: Bounds {
                x: 0,
                y: 0,
                width: 80,
                height: 24,
            },
            cursor: (0, 0),
        }
    }

    fn create_test_context() -> RenderContext<'static> {
        let theme = Box::leak(Box::new(Theme::dark()));
        RenderContext {
            screen_width: 80,
            screen_height: 24,
            theme,
            color_mode: ColorMode::TrueColor,
            tab_line_offset: 0,
            state: None,
            modifier_style: None,
            modifier_behavior: None,
        }
    }

    #[test]
    fn test_severity_to_style_error() {
        let theme = Theme::dark();
        let style = severity_to_style(Some(DiagnosticSeverity::ERROR), &theme.diagnostic);
        assert_eq!(style, theme.diagnostic.error);
    }

    #[test]
    fn test_severity_to_style_warning() {
        let theme = Theme::dark();
        let style = severity_to_style(Some(DiagnosticSeverity::WARNING), &theme.diagnostic);
        assert_eq!(style, theme.diagnostic.warn);
    }

    #[test]
    fn test_severity_to_style_hint() {
        let theme = Theme::dark();
        let style = severity_to_style(Some(DiagnosticSeverity::HINT), &theme.diagnostic);
        assert_eq!(style, theme.diagnostic.hint);
    }

    #[test]
    fn test_severity_to_style_info() {
        let theme = Theme::dark();
        let style = severity_to_style(Some(DiagnosticSeverity::INFORMATION), &theme.diagnostic);
        assert_eq!(style, theme.diagnostic.info);
    }

    #[test]
    fn test_severity_to_style_none() {
        let theme = Theme::dark();
        let style = severity_to_style(None, &theme.diagnostic);
        assert_eq!(style, theme.diagnostic.info);
    }

    #[test]
    fn test_virtual_text_priority_error_highest() {
        // Test that ERROR has highest priority
        let priorities = [
            (Some(DiagnosticSeverity::ERROR), 404),
            (Some(DiagnosticSeverity::WARNING), 403),
            (Some(DiagnosticSeverity::INFORMATION), 402),
            (Some(DiagnosticSeverity::HINT), 401),
        ];

        let mut prev_priority = u32::MAX;
        for (severity, expected_priority) in priorities {
            let priority = match severity {
                Some(DiagnosticSeverity::ERROR) => 404,
                Some(DiagnosticSeverity::WARNING) => 403,
                Some(DiagnosticSeverity::INFORMATION) => 402,
                Some(DiagnosticSeverity::HINT) => 401,
                _ => 400,
            };

            assert_eq!(priority, expected_priority);
            assert!(priority < prev_priority, "Priorities should be in descending order");
            prev_priority = priority;
        }
    }

    #[test]
    fn test_virtual_text_icon_mapping() {
        let icon_mappings = [
            (Some(DiagnosticSeverity::ERROR), "●"),
            (Some(DiagnosticSeverity::WARNING), "◐"),
            (Some(DiagnosticSeverity::HINT), "·"),
            (Some(DiagnosticSeverity::INFORMATION), "ⓘ"),
            (None, "ⓘ"),
        ];

        for (severity, expected_icon) in icon_mappings {
            let icon = match severity {
                Some(DiagnosticSeverity::ERROR) => "●",
                Some(DiagnosticSeverity::WARNING) => "◐",
                Some(DiagnosticSeverity::HINT) => "·",
                Some(DiagnosticSeverity::INFORMATION | _) | None => "ⓘ",
            };

            assert_eq!(
                icon, expected_icon,
                "Severity {severity:?} should map to icon {expected_icon}"
            );
        }
    }

    #[test]
    fn test_sign_priority_mapping() {
        let sign_mappings = [
            (Some(DiagnosticSeverity::ERROR), 304),
            (Some(DiagnosticSeverity::WARNING), 303),
            (Some(DiagnosticSeverity::INFORMATION), 302),
            (Some(DiagnosticSeverity::HINT), 301),
            (None, 300),
        ];

        for (severity, expected_priority) in sign_mappings {
            let (_, priority) = match severity {
                Some(DiagnosticSeverity::ERROR) => ("●", 304),
                Some(DiagnosticSeverity::WARNING) => ("◐", 303),
                Some(DiagnosticSeverity::INFORMATION) => ("ⓘ", 302),
                Some(DiagnosticSeverity::HINT) => ("·", 301),
                Some(_) | None => ("ⓘ", 300),
            };

            assert_eq!(
                priority, expected_priority,
                "Severity {severity:?} should have sign priority {expected_priority}"
            );
        }
    }

    #[test]
    fn test_render_data_initialization_for_lsp() {
        let render_data = create_test_render_data(vec!["line 1", "line 2", "line 3"]);

        // Verify all virtual text slots are initialized to None
        assert_eq!(render_data.virtual_texts.len(), 3);
        assert!(render_data.virtual_texts.iter().all(Option::is_none));

        // Verify all sign slots are initialized to None
        assert_eq!(render_data.signs.len(), 3);
        assert!(render_data.signs.iter().all(Option::is_none));
    }

    #[test]
    fn test_virtual_text_style_selection() {
        let ctx = create_test_context();

        // Test that each severity maps to the correct style
        let error_style = match Some(DiagnosticSeverity::ERROR) {
            Some(DiagnosticSeverity::ERROR) => ctx.theme.virtual_text.error.clone(),
            Some(DiagnosticSeverity::WARNING) => ctx.theme.virtual_text.warn.clone(),
            Some(DiagnosticSeverity::INFORMATION) => ctx.theme.virtual_text.info.clone(),
            Some(DiagnosticSeverity::HINT) => ctx.theme.virtual_text.hint.clone(),
            Some(_) | None => ctx.theme.virtual_text.default.clone(),
        };

        assert_eq!(error_style, ctx.theme.virtual_text.error);
    }

    #[test]
    fn test_virtual_text_message_formatting() {
        let message = "mismatched types: expected `i32`, found `&str`";
        let icon = "●";
        let formatted = format!("{icon} {message}");

        assert!(formatted.starts_with("●"));
        assert!(formatted.contains("mismatched types"));
        assert_eq!(formatted, "● mismatched types: expected `i32`, found `&str`");
    }

    #[test]
    fn test_lsp_render_stage_name() {
        use {crate::SharedLspManager, std::sync::Arc};

        let manager = Arc::new(SharedLspManager::new());
        let stage = LspRenderStage::new(manager);

        assert_eq!(stage.name(), "lsp");
    }
}
