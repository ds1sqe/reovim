//! Command handlers for LSP navigation.
//!
//! - `GotoDefinition`: Jump to definition (single result) or open picker (multiple).
//! - `References`: Find all references and open in picker.

use std::{path::Path, sync::Arc, time::Duration};

use {
    lsp_types::GotoDefinitionResponse,
    reovim_driver_command::CommandHandler,
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_lsp::{LspKey, LspProvider, LspProviderRegistry, LspRequest, uri_from_path},
    reovim_driver_picker::{PickerData, PickerItem, push_items},
    reovim_driver_session::{
        BufferApi, ChangeTracker, ExtensionApi, ModeApi, SessionRuntime, TransitionContext,
        WindowApi,
    },
    reovim_kernel::api::v1::{CommandId, ServiceRegistry},
    reovim_module_microscope::{MicroscopeState, modes::MicroscopeMode},
    reovim_module_notification::{NotificationLevel, NotificationState},
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
            return CommandResult::Success;
        };
        let Some(file_path) = runtime.buffer_file_path(buf_id) else {
            notify_info(runtime, "No file associated with buffer");
            return CommandResult::Success;
        };
        let Some(cursor) = runtime.cursor_position() else {
            return CommandResult::Success;
        };

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
        let (tx, rx) = reovim_kernel::api::v1::oneshot();

        if !provider.send_request(LspRequest::GotoDefinition {
            uri,
            position: lsp_pos,
            response_tx: tx,
        }) {
            notify_info(runtime, "LSP server busy");
            return CommandResult::Success;
        }

        let response = match rx.recv_timeout(LSP_TIMEOUT) {
            Ok(Ok(Some(resp))) => resp,
            Ok(Ok(None)) => {
                notify_info(runtime, "No definition found");
                return CommandResult::Success;
            }
            Ok(Err(_)) | Err(_) => {
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
        let (tx, rx) = reovim_kernel::api::v1::oneshot();

        if !provider.send_request(LspRequest::References {
            uri,
            position: lsp_pos,
            include_declaration: true,
            response_tx: tx,
        }) {
            notify_info(runtime, "LSP server busy");
            return CommandResult::Success;
        }

        let locations = match rx.recv_timeout(LSP_TIMEOUT) {
            Ok(Ok(Some(locs))) => locs,
            Ok(Ok(None)) => {
                notify_info(runtime, "No references found");
                return CommandResult::Success;
            }
            Ok(Err(_)) | Err(_) => {
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
    vec![Box::new(GotoDefinition), Box::new(References)]
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
    fn command_handlers_count() {
        let handlers = command_handlers();
        assert_eq!(handlers.len(), 2);
    }

    #[test]
    fn command_handlers_unique_ids() {
        let handlers = command_handlers();
        let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
        assert_ne!(ids[0], ids[1]);
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
}
