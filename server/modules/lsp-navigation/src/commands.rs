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
        BufferApi, ChangeTracker, ExtensionApi, ModeApi, SessionRuntime, TransitionContext,
        WindowApi,
    },
    reovim_kernel::api::v1::{CommandId, ServiceRegistry},
    reovim_module_microscope::{MicroscopeState, modes::MicroscopeMode},
    reovim_module_notification::{NotificationLevel, NotificationState},
    tracing::{debug, info, warn},
};

use crate::ids;

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

        debug!("K: request sent, waiting for response");
        match recv_response(&rx, LSP_TIMEOUT) {
            Ok(Ok(Some(hover))) => {
                let text = format_hover_content(&hover);
                if text.is_empty() {
                    notify_info(runtime, "No hover information");
                } else {
                    notify_info(runtime, &text);
                }
            }
            Ok(Ok(None)) => {
                notify_info(runtime, "No hover information");
            }
            Ok(Err(e)) => {
                warn!("K: LSP error: {e}");
                notify_info(runtime, "LSP request failed");
            }
            Err(e) => {
                warn!("K: recv_timeout error: {e}");
                notify_info(runtime, "LSP request failed");
            }
        }

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
                    notify_info(runtime, &text);
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
/// Shows the active signature label.
pub fn format_signature_help(help: &lsp_types::SignatureHelp) -> String {
    let active_idx = help.active_signature.unwrap_or(0) as usize;
    help.signatures
        .get(active_idx)
        .map_or_else(String::new, |sig| sig.label.clone())
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
mod tests {
    use std::sync::Arc;

    use {lsp_types::LocationLink, reovim_driver_lsp::DiagnosticCache};

    use super::*;

    // ========================================================================
    // Mock LspProvider for pure function tests
    // ========================================================================

    struct MockProvider {
        active: bool,
        capabilities: Option<lsp_types::ServerCapabilities>,
        accept_request: bool,
    }

    impl MockProvider {
        const fn active_with_caps(caps: lsp_types::ServerCapabilities) -> Self {
            Self {
                active: true,
                capabilities: Some(caps),
                accept_request: true,
            }
        }

        const fn inactive() -> Self {
            Self {
                active: false,
                capabilities: None,
                accept_request: false,
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[allow(clippy::unnecessary_literal_bound)]
    impl LspProvider for MockProvider {
        fn send_request(&self, _request: LspRequest) -> bool {
            self.accept_request
        }

        fn diagnostics(&self) -> &DiagnosticCache {
            static CACHE: std::sync::OnceLock<DiagnosticCache> = std::sync::OnceLock::new();
            CACHE.get_or_init(DiagnosticCache::new)
        }

        fn is_active(&self) -> bool {
            self.active
        }

        fn capabilities(&self) -> Option<std::sync::Arc<lsp_types::ServerCapabilities>> {
            self.capabilities
                .as_ref()
                .map(|c| std::sync::Arc::new(c.clone()))
        }

        fn root_path(&self) -> &Path {
            Path::new("/mock")
        }

        fn language_id(&self) -> &str {
            "mock"
        }

        fn server_info(&self) -> Option<&lsp_types::ServerInfo> {
            None
        }
    }

    // ========================================================================
    // Command metadata tests
    // ========================================================================

    #[test]
    fn goto_definition_metadata() {
        use reovim_driver_command::Command;
        let cmd = GotoDefinition;
        assert_eq!(cmd.id(), ids::GOTO_DEFINITION);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn references_metadata() {
        use reovim_driver_command::Command;
        let cmd = References;
        assert_eq!(cmd.id(), ids::REFERENCES);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn command_handlers_unique_ids() {
        let handlers = command_handlers();
        let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
        // All IDs must be unique
        for (i, id) in ids.iter().enumerate() {
            for (j, other) in ids.iter().enumerate() {
                if i != j {
                    assert_ne!(id, other, "duplicate command ID at {i} and {j}");
                }
            }
        }
    }

    // ========================================================================
    // definition_to_locations tests
    // ========================================================================

    fn make_uri(path: &str) -> lsp_types::Uri {
        path.parse().expect("test URI should parse")
    }

    fn make_location(uri: &str, line: u32, col: u32) -> lsp_types::Location {
        lsp_types::Location {
            uri: make_uri(uri),
            range: lsp_types::Range {
                start: lsp_types::Position::new(line, col),
                end: lsp_types::Position::new(line, col + 1),
            },
        }
    }

    #[test]
    fn definition_scalar() {
        let loc = make_location("file:///test.rs", 10, 5);
        let response = GotoDefinitionResponse::Scalar(loc.clone());
        let result = definition_to_locations(response);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].uri, loc.uri);
    }

    #[test]
    fn definition_array() {
        let locs = vec![
            make_location("file:///a.rs", 1, 0),
            make_location("file:///b.rs", 2, 0),
        ];
        let response = GotoDefinitionResponse::Array(locs.clone());
        let result = definition_to_locations(response);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].uri, locs[0].uri);
        assert_eq!(result[1].uri, locs[1].uri);
    }

    #[test]
    fn definition_link() {
        let link = LocationLink {
            origin_selection_range: None,
            target_uri: make_uri("file:///target.rs"),
            target_range: lsp_types::Range {
                start: lsp_types::Position::new(0, 0),
                end: lsp_types::Position::new(10, 0),
            },
            target_selection_range: lsp_types::Range {
                start: lsp_types::Position::new(5, 3),
                end: lsp_types::Position::new(5, 10),
            },
        };
        let response = GotoDefinitionResponse::Link(vec![link]);
        let result = definition_to_locations(response);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].range.start.line, 5);
        assert_eq!(result[0].range.start.character, 3);
    }

    #[test]
    fn definition_empty_array() {
        let response = GotoDefinitionResponse::Array(vec![]);
        let result = definition_to_locations(response);
        assert!(result.is_empty());
    }

    #[test]
    fn definition_empty_link() {
        let response = GotoDefinitionResponse::Link(vec![]);
        let result = definition_to_locations(response);
        assert!(result.is_empty());
    }

    // ========================================================================
    // path_from_uri tests
    // ========================================================================

    #[test]
    fn path_from_file_uri() {
        let uri = make_uri("file:///project/src/main.rs");
        let path = path_from_uri(&uri);
        assert_eq!(path, std::path::Path::new("/project/src/main.rs"));
    }

    #[test]
    fn path_from_non_file_uri() {
        let uri = make_uri("https://example.com/file.rs");
        let path = path_from_uri(&uri);
        assert_eq!(path, std::path::PathBuf::new());
    }

    // ========================================================================
    // location_to_picker_item tests
    // ========================================================================

    #[test]
    fn location_to_item_basic() {
        let loc = make_location("file:///project/src/main.rs", 9, 4);
        let item = location_to_picker_item(&loc);
        assert_eq!(item.display, "main.rs:10:5");
        assert!(item.detail.is_some());
        assert!(matches!(
            &item.data,
            PickerData::GotoLocation {
                line: 10,
                col: 5,
                ..
            }
        ));
    }

    #[test]
    fn location_to_item_zero_position() {
        let loc = make_location("file:///test.rs", 0, 0);
        let item = location_to_picker_item(&loc);
        assert_eq!(item.display, "test.rs:1:1");
    }

    // ========================================================================
    // language_from_path tests
    // ========================================================================

    #[test]
    fn language_rust() {
        assert_eq!(language_from_path("src/main.rs"), "rust");
    }

    #[test]
    fn language_python() {
        assert_eq!(language_from_path("script.py"), "python");
    }

    #[test]
    fn language_javascript() {
        assert_eq!(language_from_path("app.js"), "javascript");
        assert_eq!(language_from_path("component.jsx"), "javascript");
    }

    #[test]
    fn language_typescript() {
        assert_eq!(language_from_path("app.ts"), "typescript");
        assert_eq!(language_from_path("component.tsx"), "typescript");
    }

    #[test]
    fn language_c() {
        assert_eq!(language_from_path("main.c"), "c");
        assert_eq!(language_from_path("header.h"), "c");
    }

    #[test]
    fn language_cpp() {
        assert_eq!(language_from_path("main.cpp"), "cpp");
        assert_eq!(language_from_path("header.hpp"), "cpp");
        assert_eq!(language_from_path("main.cc"), "cpp");
        assert_eq!(language_from_path("main.cxx"), "cpp");
    }

    #[test]
    fn language_go() {
        assert_eq!(language_from_path("main.go"), "go");
    }

    #[test]
    fn language_unknown() {
        assert_eq!(language_from_path("Makefile"), "unknown");
        assert_eq!(language_from_path("file.xyz"), "unknown");
    }

    #[test]
    fn language_case_insensitive() {
        assert_eq!(language_from_path("main.RS"), "rust");
        assert_eq!(language_from_path("app.Js"), "javascript");
        assert_eq!(language_from_path("main.CPP"), "cpp");
    }

    // ========================================================================
    // find_provider tests
    // ========================================================================

    #[test]
    fn find_provider_with_language_match() {
        let services = Arc::new(ServiceRegistry::new());
        let registry = services.get_or_create::<LspProviderRegistry>();

        let caps = lsp_types::ServerCapabilities::default();
        let provider: Arc<dyn LspProvider> = Arc::new(MockProvider::active_with_caps(caps));
        registry.register(LspKey::Language("rust".to_owned()), provider);

        let result = find_provider(&services, "src/main.rs");
        assert!(result.is_some());
    }

    #[test]
    fn find_provider_fallback_to_default() {
        let services = Arc::new(ServiceRegistry::new());
        let registry = services.get_or_create::<LspProviderRegistry>();

        let caps = lsp_types::ServerCapabilities::default();
        let provider: Arc<dyn LspProvider> = Arc::new(MockProvider::active_with_caps(caps));
        registry.register(LspKey::Default, provider);

        let result = find_provider(&services, "src/main.xyz");
        assert!(result.is_some());
    }

    #[test]
    fn find_provider_inactive_returns_none() {
        let services = Arc::new(ServiceRegistry::new());
        let registry = services.get_or_create::<LspProviderRegistry>();

        let provider: Arc<dyn LspProvider> = Arc::new(MockProvider::inactive());
        registry.register(LspKey::Language("rust".to_owned()), provider);

        let result = find_provider(&services, "src/main.rs");
        assert!(result.is_none());
    }

    #[test]
    fn find_provider_no_registry() {
        let services = Arc::new(ServiceRegistry::new());
        let result = find_provider(&services, "src/main.rs");
        assert!(result.is_none());
    }

    #[test]
    fn find_provider_inactive_default() {
        let services = Arc::new(ServiceRegistry::new());
        let registry = services.get_or_create::<LspProviderRegistry>();

        let provider: Arc<dyn LspProvider> = Arc::new(MockProvider::inactive());
        registry.register(LspKey::Default, provider);

        let result = find_provider(&services, "src/main.xyz");
        assert!(result.is_none());
    }

    #[test]
    fn find_provider_inactive_language_fallback_to_active_default() {
        let services = Arc::new(ServiceRegistry::new());
        let registry = services.get_or_create::<LspProviderRegistry>();

        let inactive: Arc<dyn LspProvider> = Arc::new(MockProvider::inactive());
        registry.register(LspKey::Language("rust".to_owned()), inactive);

        let caps = lsp_types::ServerCapabilities::default();
        let active: Arc<dyn LspProvider> = Arc::new(MockProvider::active_with_caps(caps));
        registry.register(LspKey::Default, active);

        let result = find_provider(&services, "src/main.rs");
        assert!(result.is_some());
    }

    // ========================================================================
    // Capability check tests
    // ========================================================================

    #[test]
    fn has_definition_capability_with_provider() {
        let caps = lsp_types::ServerCapabilities {
            definition_provider: Some(lsp_types::OneOf::Left(true)),
            ..Default::default()
        };
        let provider = MockProvider::active_with_caps(caps);
        assert!(has_definition_capability(&provider));
    }

    #[test]
    fn has_definition_capability_without() {
        let caps = lsp_types::ServerCapabilities::default();
        let provider = MockProvider::active_with_caps(caps);
        assert!(!has_definition_capability(&provider));
    }

    #[test]
    fn has_definition_capability_none_caps() {
        let provider = MockProvider::inactive();
        assert!(!has_definition_capability(&provider));
    }

    #[test]
    fn has_references_capability_with_provider() {
        let caps = lsp_types::ServerCapabilities {
            references_provider: Some(lsp_types::OneOf::Left(true)),
            ..Default::default()
        };
        let provider = MockProvider::active_with_caps(caps);
        assert!(has_references_capability(&provider));
    }

    #[test]
    fn has_references_capability_without() {
        let caps = lsp_types::ServerCapabilities::default();
        let provider = MockProvider::active_with_caps(caps);
        assert!(!has_references_capability(&provider));
    }

    #[test]
    fn has_references_capability_none_caps() {
        let provider = MockProvider::inactive();
        assert!(!has_references_capability(&provider));
    }

    // ========================================================================
    // Hover capability check tests
    // ========================================================================

    #[test]
    fn has_hover_capability_with_provider() {
        let caps = lsp_types::ServerCapabilities {
            hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
            ..Default::default()
        };
        let provider = MockProvider::active_with_caps(caps);
        assert!(has_hover_capability(&provider));
    }

    #[test]
    fn has_hover_capability_without() {
        let caps = lsp_types::ServerCapabilities::default();
        let provider = MockProvider::active_with_caps(caps);
        assert!(!has_hover_capability(&provider));
    }

    #[test]
    fn has_hover_capability_none_caps() {
        let provider = MockProvider::inactive();
        assert!(!has_hover_capability(&provider));
    }

    // ========================================================================
    // Signature help capability check tests
    // ========================================================================

    #[test]
    fn has_signature_help_capability_with_provider() {
        let caps = lsp_types::ServerCapabilities {
            signature_help_provider: Some(lsp_types::SignatureHelpOptions::default()),
            ..Default::default()
        };
        let provider = MockProvider::active_with_caps(caps);
        assert!(has_signature_help_capability(&provider));
    }

    #[test]
    fn has_signature_help_capability_without() {
        let caps = lsp_types::ServerCapabilities::default();
        let provider = MockProvider::active_with_caps(caps);
        assert!(!has_signature_help_capability(&provider));
    }

    #[test]
    fn has_signature_help_capability_none_caps() {
        let provider = MockProvider::inactive();
        assert!(!has_signature_help_capability(&provider));
    }

    // ========================================================================
    // format_hover_content tests
    // ========================================================================

    #[test]
    fn format_hover_scalar_string() {
        let hover = lsp_types::Hover {
            contents: HoverContents::Scalar(MarkedString::String("hello world".to_string())),
            range: None,
        };
        assert_eq!(format_hover_content(&hover), "hello world");
    }

    #[test]
    fn format_hover_scalar_language_string() {
        let hover = lsp_types::Hover {
            contents: HoverContents::Scalar(MarkedString::LanguageString(
                lsp_types::LanguageString {
                    language: "rust".to_string(),
                    value: "fn main() {}".to_string(),
                },
            )),
            range: None,
        };
        assert_eq!(format_hover_content(&hover), "```rust\nfn main() {}\n```");
    }

    #[test]
    fn format_hover_array() {
        let hover = lsp_types::Hover {
            contents: HoverContents::Array(vec![
                MarkedString::String("Type: i32".to_string()),
                MarkedString::LanguageString(lsp_types::LanguageString {
                    language: "rust".to_string(),
                    value: "let x: i32".to_string(),
                }),
            ]),
            range: None,
        };
        let result = format_hover_content(&hover);
        assert!(result.contains("Type: i32"));
        assert!(result.contains("```rust\nlet x: i32\n```"));
    }

    #[test]
    fn format_hover_markup() {
        let hover = lsp_types::Hover {
            contents: HoverContents::Markup(lsp_types::MarkupContent {
                kind: lsp_types::MarkupKind::Markdown,
                value: "## Documentation\nSome doc text".to_string(),
            }),
            range: None,
        };
        assert_eq!(format_hover_content(&hover), "## Documentation\nSome doc text");
    }

    #[test]
    fn format_hover_empty_array() {
        let hover = lsp_types::Hover {
            contents: HoverContents::Array(vec![]),
            range: None,
        };
        assert!(format_hover_content(&hover).is_empty());
    }

    // ========================================================================
    // format_signature_help tests
    // ========================================================================

    #[test]
    fn format_signature_help_single() {
        let help = lsp_types::SignatureHelp {
            signatures: vec![lsp_types::SignatureInformation {
                label: "fn foo(x: i32, y: &str) -> bool".to_string(),
                documentation: None,
                parameters: None,
                active_parameter: None,
            }],
            active_signature: Some(0),
            active_parameter: None,
        };
        assert_eq!(format_signature_help(&help), "fn foo(x: i32, y: &str) -> bool");
    }

    #[test]
    fn format_signature_help_multiple_active_second() {
        let help = lsp_types::SignatureHelp {
            signatures: vec![
                lsp_types::SignatureInformation {
                    label: "fn bar(a: u8)".to_string(),
                    documentation: None,
                    parameters: None,
                    active_parameter: None,
                },
                lsp_types::SignatureInformation {
                    label: "fn bar(a: u8, b: u8)".to_string(),
                    documentation: None,
                    parameters: None,
                    active_parameter: None,
                },
            ],
            active_signature: Some(1),
            active_parameter: None,
        };
        assert_eq!(format_signature_help(&help), "fn bar(a: u8, b: u8)");
    }

    #[test]
    fn format_signature_help_no_active() {
        let help = lsp_types::SignatureHelp {
            signatures: vec![lsp_types::SignatureInformation {
                label: "fn default()".to_string(),
                documentation: None,
                parameters: None,
                active_parameter: None,
            }],
            active_signature: None,
            active_parameter: None,
        };
        assert_eq!(format_signature_help(&help), "fn default()");
    }

    #[test]
    fn format_signature_help_empty_signatures() {
        let help = lsp_types::SignatureHelp {
            signatures: vec![],
            active_signature: None,
            active_parameter: None,
        };
        assert!(format_signature_help(&help).is_empty());
    }

    // ========================================================================
    // Command metadata tests (Hover + SignatureHelp)
    // ========================================================================

    #[test]
    fn hover_command_metadata() {
        use reovim_driver_command::Command;
        let cmd = HoverCommand;
        assert_eq!(cmd.id(), ids::HOVER);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn signature_help_command_metadata() {
        use reovim_driver_command::Command;
        let cmd = SignatureHelpCommand;
        assert_eq!(cmd.id(), ids::SIGNATURE_HELP);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn command_handlers_count() {
        let handlers = command_handlers();
        assert_eq!(handlers.len(), 4);
    }
}
