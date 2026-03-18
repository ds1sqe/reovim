//! Command handlers for LSP navigation.
//!
//! - `GotoDefinition`: Jump to definition (single result) or open picker (multiple).
//! - `References`: Find all references and open in picker.

use std::{path::Path, sync::Arc, time::Duration};

use {
    lsp_types::{GotoDefinitionResponse, HoverContents, MarkedString},
    reovim_driver_command::CommandHandler,
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_lsp::{
        LspKey, LspProvider, LspProviderRegistry, LspRequest, recv_response, uri_from_path,
    },
    reovim_driver_picker::{PickerData, PickerItem, push_items},
    reovim_driver_session::{
        BufferApi, ChangeTracker, ExtensionApi, ModeApi, SessionRuntime, TickSchedulerHandle,
        TransitionContext, WindowApi,
    },
    reovim_kernel::api::v1::{CommandId, ServiceRegistry},
    reovim_module_microscope::{MicroscopeState, modes::MicroscopeMode},
    reovim_module_notification::{NotificationLevel, NotificationState},
    tracing::{debug, info, warn},
};

use crate::{
    hover_state::{HoverCache, HoverContentType, HoverSnapshot},
    ids,
    signature_help_state::SignatureHelpState,
};

/// Timeout for LSP request/response.
const LSP_TIMEOUT: Duration = Duration::from_secs(5);

// ============================================================================
// GotoDefinition command
// ============================================================================

/// Go to definition (LSP).
pub struct GotoDefinition;

impl reovim_driver_command::Command for GotoDefinition {
    fn id(&self) -> CommandId {
        ids::GOTO_DEFINITION
    }

    fn description(&self) -> &'static str {
        "Go to definition (LSP)"
    }
}

impl CommandHandler for GotoDefinition {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(buf_id) = runtime.active_buffer() else {
            debug!("gd: no active buffer");
            return CommandResult::Success;
        };
        let Some(file_path) = runtime.buffer_file_path(buf_id) else {
            notify_info(runtime, "No file associated with buffer");
            return CommandResult::Success;
        };
        let Some(cursor) = runtime.cursor_position() else {
            debug!("gd: no cursor position");
            return CommandResult::Success;
        };

        info!(file = %file_path, line = cursor.line, col = cursor.column, "gd: goto definition");

        let services = runtime.kernel().services.clone();
        let Some(provider) = find_provider(&services, &file_path) else {
            notify_info(runtime, "No LSP server active");
            return CommandResult::Success;
        };

        if !has_definition_capability(&*provider) {
            notify_info(runtime, "LSP server does not support goto definition");
            return CommandResult::Success;
        }

        let uri = uri_from_path(Path::new(&file_path));
        // Cursor positions are bounded by terminal/buffer size, well within u32 range.
        #[allow(clippy::cast_possible_truncation)]
        let lsp_pos = lsp_types::Position::new(cursor.line as u32, cursor.column as u32);
        info!(uri = %uri.as_str(), line = lsp_pos.line, character = lsp_pos.character, "gd: sending request");
        let (tx, rx) = reovim_kernel::api::v1::oneshot();

        if !provider.send_request(LspRequest::GotoDefinition {
            uri,
            position: lsp_pos,
            response_tx: tx,
        }) {
            warn!("gd: send_request returned false (server busy)");
            notify_info(runtime, "LSP server busy");
            return CommandResult::Success;
        }

        debug!("gd: request sent, waiting for response (timeout={}s)", LSP_TIMEOUT.as_secs());
        let response = match recv_response(&rx, LSP_TIMEOUT) {
            Ok(Ok(Some(resp))) => {
                info!("gd: received definition response");
                resp
            }
            Ok(Ok(None)) => {
                info!("gd: server returned None (no definition)");
                notify_info(runtime, "No definition found");
                return CommandResult::Success;
            }
            Ok(Err(e)) => {
                warn!("gd: LSP error: {e}");
                notify_info(runtime, "LSP request failed");
                return CommandResult::Success;
            }
            Err(e) => {
                warn!("gd: recv_timeout error: {e}");
                notify_info(runtime, "LSP request failed");
                return CommandResult::Success;
            }
        };

        let locations = definition_to_locations(response);
        if locations.is_empty() {
            notify_info(runtime, "No definition found");
            return CommandResult::Success;
        }

        if locations.len() == 1 {
            jump_to_location(runtime, &locations[0]);
        } else {
            open_locations_picker(runtime, &services, &locations, "Definitions");
        }

        CommandResult::Success
    }
}

// ============================================================================
// References command
// ============================================================================

/// Find references (LSP).
pub struct References;

impl reovim_driver_command::Command for References {
    fn id(&self) -> CommandId {
        ids::REFERENCES
    }

    fn description(&self) -> &'static str {
        "Find references (LSP)"
    }
}

impl CommandHandler for References {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(buf_id) = runtime.active_buffer() else {
            return CommandResult::Success;
        };
        let Some(file_path) = runtime.buffer_file_path(buf_id) else {
            notify_info(runtime, "No file associated with buffer");
            return CommandResult::Success;
        };
        let Some(cursor) = runtime.cursor_position() else {
            return CommandResult::Success;
        };

        info!(file = %file_path, line = cursor.line, col = cursor.column, "gr: find references");

        let services = runtime.kernel().services.clone();
        let Some(provider) = find_provider(&services, &file_path) else {
            notify_info(runtime, "No LSP server active");
            return CommandResult::Success;
        };

        if !has_references_capability(&*provider) {
            notify_info(runtime, "LSP server does not support find references");
            return CommandResult::Success;
        }

        let uri = uri_from_path(Path::new(&file_path));
        // Cursor positions are bounded by terminal/buffer size, well within u32 range.
        #[allow(clippy::cast_possible_truncation)]
        let lsp_pos = lsp_types::Position::new(cursor.line as u32, cursor.column as u32);
        info!(uri = %uri.as_str(), line = lsp_pos.line, character = lsp_pos.character, "gr: sending request");
        let (tx, rx) = reovim_kernel::api::v1::oneshot();

        if !provider.send_request(LspRequest::References {
            uri,
            position: lsp_pos,
            include_declaration: true,
            response_tx: tx,
        }) {
            warn!("gr: send_request returned false");
            notify_info(runtime, "LSP server busy");
            return CommandResult::Success;
        }

        debug!("gr: request sent, waiting for response");
        let locations = match recv_response(&rx, LSP_TIMEOUT) {
            Ok(Ok(Some(locs))) => {
                info!(count = locs.len(), "gr: received references");
                locs
            }
            Ok(Ok(None)) => {
                info!("gr: server returned None");
                notify_info(runtime, "No references found");
                return CommandResult::Success;
            }
            Ok(Err(e)) => {
                warn!("gr: LSP error: {e}");
                notify_info(runtime, "LSP request failed");
                return CommandResult::Success;
            }
            Err(e) => {
                warn!("gr: recv_timeout error: {e}");
                notify_info(runtime, "LSP request failed");
                return CommandResult::Success;
            }
        };

        if locations.is_empty() {
            notify_info(runtime, "No references found");
            return CommandResult::Success;
        }

        open_locations_picker(runtime, &services, &locations, "References");
        CommandResult::Success
    }
}

// ============================================================================
// Hover command
// ============================================================================

/// Show hover information (LSP).
pub struct HoverCommand;

impl reovim_driver_command::Command for HoverCommand {
    fn id(&self) -> CommandId {
        ids::HOVER
    }

    fn description(&self) -> &'static str {
        "Show hover information (LSP)"
    }
}

/// Tick interval for hover bridge polling.
const HOVER_TICK_INTERVAL: Duration = Duration::from_millis(50);

impl CommandHandler for HoverCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(buf_id) = runtime.active_buffer() else {
            debug!("K: no active buffer");
            return CommandResult::Success;
        };
        let Some(file_path) = runtime.buffer_file_path(buf_id) else {
            notify_info(runtime, "No file associated with buffer");
            return CommandResult::Success;
        };
        let Some(cursor) = runtime.cursor_position() else {
            debug!("K: no cursor position");
            return CommandResult::Success;
        };

        info!(file = %file_path, line = cursor.line, col = cursor.column, "K: hover");

        let services = runtime.kernel().services.clone();
        let Some(provider) = find_provider(&services, &file_path) else {
            notify_info(runtime, "No LSP server active");
            return CommandResult::Success;
        };

        if !has_hover_capability(&*provider) {
            notify_info(runtime, "LSP server does not support hover");
            return CommandResult::Success;
        }

        let uri = uri_from_path(Path::new(&file_path));
        #[allow(clippy::cast_possible_truncation)]
        let lsp_pos = lsp_types::Position::new(cursor.line as u32, cursor.column as u32);
        let (tx, rx) = reovim_kernel::api::v1::oneshot();

        if !provider.send_request(LspRequest::Hover {
            uri,
            position: lsp_pos,
            response_tx: tx,
        }) {
            warn!("K: send_request returned false");
            notify_info(runtime, "LSP server busy");
            return CommandResult::Success;
        }

        // Fire-and-forget: spawn async task to await LSP response (#662).
        // The task stores the result in HoverCache; HoverBridge::tick()
        // picks it up and pushes to the client.
        let cache = services.get_or_create::<HoverCache>();
        let cache_arc = cache.shared();
        #[allow(clippy::cast_possible_truncation)]
        let origin_buffer_id = buf_id.as_usize() as u64;
        #[allow(clippy::cast_possible_truncation)]
        let origin_line = cursor.line as u32;
        #[allow(clippy::cast_possible_truncation)]
        let origin_col = cursor.column as u32;

        // Start tick so HoverBridge::tick() polls for the result.
        if let Some(client_id) = runtime.owner()
            && let Some(tick_handle) = services.get::<TickSchedulerHandle>()
        {
            tick_handle.start(client_id, "hover", HOVER_TICK_INTERVAL);
        }

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let timeout = LSP_TIMEOUT;
            handle.spawn(async move {
                // OneshotReceiver is crossbeam-based (not a Future), so use
                // spawn_blocking to await it without holding a tokio thread.
                let result = tokio::task::spawn_blocking(move || rx.recv_timeout(timeout)).await;
                match result {
                    Ok(Ok(Ok(Some(hover)))) => {
                        let text = format_hover_content(&hover);
                        if text.is_empty() {
                            debug!("K: hover returned empty content");
                        } else {
                            let content_type = hover_content_type(&hover.contents);
                            cache_arc.store(Arc::new(Some(HoverSnapshot {
                                content: text,
                                content_type,
                                buffer_id: origin_buffer_id,
                                line: origin_line,
                                col: origin_col,
                            })));
                            info!("K: hover result cached for tick");
                        }
                    }
                    Ok(Ok(Ok(None))) => {
                        debug!("K: server returned None (no hover)");
                    }
                    Ok(Ok(Err(e))) => {
                        warn!("K: LSP error: {e}");
                    }
                    Ok(Err(_recv_err)) => {
                        warn!("K: oneshot channel closed or timed out");
                    }
                    Err(join_err) => {
                        warn!("K: spawn_blocking panicked: {join_err}");
                    }
                }
            });
        } else {
            debug!("K: no tokio runtime, skipping async hover");
        }

        debug!("K: hover request sent (non-blocking)");
        CommandResult::Success
    }
}

// ============================================================================
// SignatureHelp command
// ============================================================================

/// Show signature help (LSP).
pub struct SignatureHelpCommand;

impl reovim_driver_command::Command for SignatureHelpCommand {
    fn id(&self) -> CommandId {
        ids::SIGNATURE_HELP
    }

    fn description(&self) -> &'static str {
        "Show signature help (LSP)"
    }
}

impl CommandHandler for SignatureHelpCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(buf_id) = runtime.active_buffer() else {
            debug!("sig-help: no active buffer");
            return CommandResult::Success;
        };
        let Some(file_path) = runtime.buffer_file_path(buf_id) else {
            notify_info(runtime, "No file associated with buffer");
            return CommandResult::Success;
        };
        let Some(cursor) = runtime.cursor_position() else {
            debug!("sig-help: no cursor position");
            return CommandResult::Success;
        };

        info!(file = %file_path, line = cursor.line, col = cursor.column, "sig-help: request");

        let services = runtime.kernel().services.clone();
        let Some(provider) = find_provider(&services, &file_path) else {
            notify_info(runtime, "No LSP server active");
            return CommandResult::Success;
        };

        if !has_signature_help_capability(&*provider) {
            notify_info(runtime, "LSP server does not support signature help");
            return CommandResult::Success;
        }

        let uri = uri_from_path(Path::new(&file_path));
        #[allow(clippy::cast_possible_truncation)]
        let lsp_pos = lsp_types::Position::new(cursor.line as u32, cursor.column as u32);
        let (tx, rx) = reovim_kernel::api::v1::oneshot();

        if !provider.send_request(LspRequest::SignatureHelp {
            uri,
            position: lsp_pos,
            response_tx: tx,
        }) {
            warn!("sig-help: send_request returned false");
            notify_info(runtime, "LSP server busy");
            return CommandResult::Success;
        }

        debug!("sig-help: request sent, waiting for response");
        match recv_response(&rx, LSP_TIMEOUT) {
            Ok(Ok(Some(help))) => {
                let text = format_signature_help(&help);
                if text.is_empty() {
                    notify_info(runtime, "No signature help available");
                } else {
                    let state = runtime.ext_mut::<SignatureHelpState>();
                    // Cursor positions and buffer IDs are bounded well within u32/u64.
                    #[allow(clippy::cast_possible_truncation)]
                    state.show(
                        text,
                        buf_id.as_usize() as u64,
                        cursor.line as u32,
                        cursor.column as u32,
                    );
                    runtime
                        .take_changes()
                        .record_extension_change("signature-help".into());
                }
            }
            Ok(Ok(None)) => {
                notify_info(runtime, "No signature help available");
            }
            Ok(Err(e)) => {
                warn!("sig-help: LSP error: {e}");
                notify_info(runtime, "LSP request failed");
            }
            Err(e) => {
                warn!("sig-help: recv_timeout error: {e}");
                notify_info(runtime, "LSP request failed");
            }
        }

        CommandResult::Success
    }
}

// ============================================================================
// Pure helper functions (fully testable)
// ============================================================================

/// Convert `GotoDefinitionResponse` to a flat list of locations.
pub fn definition_to_locations(response: GotoDefinitionResponse) -> Vec<lsp_types::Location> {
    match response {
        GotoDefinitionResponse::Scalar(loc) => vec![loc],
        GotoDefinitionResponse::Array(locs) => locs,
        GotoDefinitionResponse::Link(links) => links
            .into_iter()
            .map(|link| lsp_types::Location {
                uri: link.target_uri,
                range: link.target_selection_range,
            })
            .collect(),
    }
}

/// Extract a file path from an LSP URI.
///
/// Strips the `file://` scheme prefix. Returns an empty `PathBuf` for
/// non-file URIs.
pub fn path_from_uri(uri: &lsp_types::Uri) -> std::path::PathBuf {
    uri.as_str()
        .strip_prefix("file://")
        .map_or_else(std::path::PathBuf::new, std::path::PathBuf::from)
}

/// Convert an LSP `Location` to a `PickerItem`.
///
/// LSP positions are 0-indexed; picker positions are 1-indexed.
pub fn location_to_picker_item(loc: &lsp_types::Location) -> PickerItem {
    let path = path_from_uri(&loc.uri);
    let line = loc.range.start.line as usize + 1;
    let col = loc.range.start.character as usize + 1;
    let display =
        format!("{}:{}:{}", path.file_name().and_then(|n| n.to_str()).unwrap_or("?"), line, col);
    let detail = Some(path.to_string_lossy().to_string());
    PickerItem {
        display,
        detail,
        data: PickerData::GotoLocation { path, line, col },
        icon: None,
    }
}

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
        _ => "unknown",
    }
}

/// Find an active LSP provider for the given file path.
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

/// Check if the provider supports goto definition.
pub fn has_definition_capability(provider: &dyn LspProvider) -> bool {
    provider
        .capabilities()
        .is_some_and(|caps| caps.definition_provider.is_some())
}

/// Check if the provider supports find references.
pub fn has_references_capability(provider: &dyn LspProvider) -> bool {
    provider
        .capabilities()
        .is_some_and(|caps| caps.references_provider.is_some())
}

/// Check if the provider supports hover.
pub fn has_hover_capability(provider: &dyn LspProvider) -> bool {
    provider
        .capabilities()
        .is_some_and(|caps| caps.hover_provider.is_some())
}

/// Check if the provider supports signature help.
pub fn has_signature_help_capability(provider: &dyn LspProvider) -> bool {
    provider
        .capabilities()
        .is_some_and(|caps| caps.signature_help_provider.is_some())
}

/// Determine the content type from hover contents.
///
/// - `Scalar(String)`: plain text
/// - `Markup(PlainText)`: plain text
/// - `Scalar(LanguageString)`, `Array`, `Markup(Markdown)`: markdown
#[must_use]
pub fn hover_content_type(contents: &HoverContents) -> HoverContentType {
    match contents {
        HoverContents::Scalar(MarkedString::String(_)) => HoverContentType::PlainText,
        HoverContents::Markup(markup) if markup.kind == lsp_types::MarkupKind::PlainText => {
            HoverContentType::PlainText
        }
        _ => HoverContentType::Markdown,
    }
}

/// Format hover content for display.
///
/// Extracts text from all `HoverContents` variants:
/// - `Scalar(String)`: plain text
/// - `Scalar(LanguageString)`: code block with language tag
/// - `Array`: multiple items joined with newlines
/// - `Markup`: markdown content
#[must_use]
pub fn format_hover_content(hover: &lsp_types::Hover) -> String {
    match &hover.contents {
        HoverContents::Scalar(MarkedString::String(s)) => s.clone(),
        HoverContents::Scalar(MarkedString::LanguageString(ls)) => {
            format!("```{}\n{}\n```", ls.language, ls.value)
        }
        HoverContents::Array(items) => items
            .iter()
            .map(|item| match item {
                MarkedString::String(s) => s.clone(),
                MarkedString::LanguageString(ls) => {
                    format!("```{}\n{}\n```", ls.language, ls.value)
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
        HoverContents::Markup(markup) => markup.value.clone(),
    }
}

/// Format signature help for display.
///
/// Shows the active signature label. If `active_signature` is out of range,
/// returns an empty string and logs a warning.
pub fn format_signature_help(help: &lsp_types::SignatureHelp) -> String {
    let active_idx = help.active_signature.unwrap_or(0) as usize;
    if let Some(sig) = help.signatures.get(active_idx) {
        return sig.label.clone();
    }
    if !help.signatures.is_empty() {
        warn!(active_idx, count = help.signatures.len(), "active_signature index out of range");
    }
    String::new()
}

// ============================================================================
// Runtime helpers (coverage(off) - require real SessionRuntime)
// ============================================================================

/// Push an info notification.
#[cfg_attr(coverage_nightly, coverage(off))]
fn notify_info(runtime: &mut SessionRuntime<'_>, message: &str) {
    let state = runtime.ext_mut::<NotificationState>();
    state.push(NotificationLevel::Info, message);
    runtime
        .take_changes()
        .record_extension_change("notification".into());
}

/// Jump to an LSP location (open file + set cursor).
#[cfg_attr(coverage_nightly, coverage(off))]
fn jump_to_location(runtime: &mut SessionRuntime<'_>, location: &lsp_types::Location) {
    let path = path_from_uri(&location.uri);
    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
    let path_str = canonical.to_string_lossy();

    let existing = runtime.kernel().buffers.list().into_iter().find(|&id| {
        runtime
            .buffer_file_path(id)
            .is_some_and(|p| Path::new(&p) == canonical)
    });

    let buf_id = existing.unwrap_or_else(|| {
        use reovim_driver_vfs::VfsInstance;
        let content = runtime
            .kernel()
            .services
            .get::<VfsInstance>()
            .and_then(|vfs| vfs.driver().read_to_string(&canonical).ok())
            .unwrap_or_default();
        let id = runtime.create_buffer(Some(&path_str), &content);
        runtime.set_buffer_modified(id, false);
        id
    });

    if let Some(win) = runtime.active_window() {
        let _ = runtime.set_window_buffer(win, buf_id);
    }

    if let Some(window) = runtime.windows_mut().active_mut() {
        window.cursor.line = location.range.start.line as usize;
        window.cursor.column = location.range.start.character as usize;
    }

    runtime.record_cursor_move(buf_id);
}

/// Open the locations picker with injected items.
#[cfg_attr(coverage_nightly, coverage(off))]
fn open_locations_picker(
    runtime: &mut SessionRuntime<'_>,
    services: &Arc<ServiceRegistry>,
    locations: &[lsp_types::Location],
    title: &str,
) {
    let items: Vec<PickerItem> = locations.iter().map(location_to_picker_item).collect();

    let state = runtime.ext_mut::<MicroscopeState>();
    state.active = true;
    state.query.clear();
    state.cursor = 0;
    state.selected = 0;
    state.scroll_offset = 0;
    "lsp-locations".clone_into(&mut state.picker_name);
    title.clone_into(&mut state.picker_title);
    "> ".clone_into(&mut state.prompt);
    state.preview = None;
    state.services = Some(Arc::clone(services));

    state.engine.restart();
    if !items.is_empty() {
        let injector = state.engine.injector();
        push_items(&injector, items);
    }
    state.refresh_engine();

    runtime.set_mode(MicroscopeMode::PICKER_ID, TransitionContext::new());
}

/// Collect all command handlers for registration.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(GotoDefinition),
        Box::new(References),
        Box::new(HoverCommand),
        Box::new(SignatureHelpCommand),
    ]
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
