//! Command handlers for the completion popup.
//!
//! Commands for triggering, navigating, confirming, and dismissing completions.
//! These handlers operate on `CompletionState` stored in the session's
//! `ExtensionMap`.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use {
    reovim_driver_command::CommandHandler,
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_completion::{CompletionContext, CompletionSourceRegistry},
    reovim_driver_lsp::{
        LspKey, LspLifecycleRegistry, LspProviderRegistry, LspRequest, LspServerConfig,
        uri_from_path,
    },
    reovim_driver_session::{
        BufferApi, ChangeTracker, ExtensionApi, NotificationDrainRegistry, SessionRuntime,
        SnippetExpanderRegistry,
    },
    reovim_kernel::api::v1::{CommandId, Position, ServiceRegistry, oneshot},
    tracing::{debug, info, warn},
};

use crate::{
    ids,
    lsp_source::{LspCompletionSource, map_lsp_item},
    notification_queue::{PendingLevel, PendingNotificationQueue},
    state::{CompletionItemSnapshot, CompletionState},
};

// ============================================================================
// Trigger completion
// ============================================================================

/// Trigger the completion popup with items from all registered sources.
#[derive(Debug, Clone, Copy, Default)]
pub struct Trigger;

impl reovim_driver_command::Command for Trigger {
    fn id(&self) -> CommandId {
        ids::TRIGGER
    }

    fn description(&self) -> &'static str {
        "Trigger completion"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Trigger {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Drain pending notifications from background threads.
        drain_pending_notifications(runtime);

        // Build completion context from current buffer state.
        let Some(ctx) = build_context(runtime) else {
            return CommandResult::Success;
        };

        // Fire-and-forget: send LSP completion request in background.
        // The response populates the LspCompletionSource cache so that
        // the NEXT trigger shows LSP items.
        if ctx.language_id.is_some() {
            fire_lsp_completion(&runtime.kernel().services, &ctx);
        }

        // Gather items from all registered sources (sync).
        let registry = runtime.kernel().services.get::<CompletionSourceRegistry>();
        let mut all_items = Vec::new();

        if let Some(registry) = registry {
            let mut sources = registry.all();
            // Sort by priority (highest first).
            sources.sort_by_key(|s| std::cmp::Reverse(s.priority()));

            for source in &sources {
                if source.is_available(&ctx) {
                    let items = source.complete(&ctx);
                    all_items.extend(items);
                }
            }
        }

        if all_items.is_empty() {
            return CommandResult::Success;
        }

        let snapshots: Vec<CompletionItemSnapshot> = all_items
            .iter()
            .map(CompletionItemSnapshot::from_item)
            .collect();

        let state = runtime.ext_mut::<CompletionState>();
        state.open(snapshots, &ctx.prefix);

        CommandResult::Success
    }
}

// ============================================================================
// Navigation
// ============================================================================

/// Move to the next completion item.
#[derive(Debug, Clone, Copy, Default)]
pub struct Next;

impl reovim_driver_command::Command for Next {
    fn id(&self) -> CommandId {
        ids::NEXT
    }

    fn description(&self) -> &'static str {
        "Next completion item"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Next {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<CompletionState>();
        if state.active {
            state.next();
        }
        CommandResult::Success
    }
}

/// Move to the previous completion item.
#[derive(Debug, Clone, Copy, Default)]
pub struct Prev;

impl reovim_driver_command::Command for Prev {
    fn id(&self) -> CommandId {
        ids::PREV
    }

    fn description(&self) -> &'static str {
        "Previous completion item"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Prev {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<CompletionState>();
        if state.active {
            state.prev();
        }
        CommandResult::Success
    }
}

// ============================================================================
// Confirm / Dismiss
// ============================================================================

/// Confirm the selected completion item.
#[derive(Debug, Clone, Copy, Default)]
pub struct Confirm;

impl reovim_driver_command::Command for Confirm {
    fn id(&self) -> CommandId {
        ids::CONFIRM
    }

    fn description(&self) -> &'static str {
        "Confirm selected completion"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Confirm {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Extract needed data from state before closing.
        let state = runtime.ext_mut::<CompletionState>();
        let Some(selected) = state.selected_item().cloned() else {
            state.close();
            return CommandResult::Success;
        };
        let prefix_len = state.prefix.len();
        state.close();

        // Get buffer and cursor.
        let Some(buffer_id) = runtime.active_buffer() else {
            return CommandResult::Success;
        };
        let Some(window) = runtime.windows().active() else {
            return CommandResult::Success;
        };
        let cursor = Position::new(window.cursor.line, window.cursor.column);

        // Delete the typed prefix.
        if prefix_len > 0 && cursor.column >= prefix_len {
            let prefix_start = Position::new(cursor.line, cursor.column - prefix_len);
            runtime.delete_range(buffer_id, prefix_start, cursor);
        }
        let insert_pos = Position::new(cursor.line, cursor.column.saturating_sub(prefix_len));

        if selected.is_snippet {
            confirm_snippet(runtime, buffer_id, insert_pos, &selected.insert_text);
        } else {
            // Plain text: insert directly.
            runtime.insert_text(buffer_id, insert_pos, &selected.insert_text);
            // Move cursor to end of inserted text.
            let mut end_line = insert_pos.line;
            let mut end_col = insert_pos.column;
            for ch in selected.insert_text.chars() {
                if ch == '\n' {
                    end_line += 1;
                    end_col = 0;
                } else {
                    end_col += 1;
                }
            }
            if let Some(w) = runtime.windows_mut().active_mut() {
                w.cursor.line = end_line;
                w.cursor.column = end_col;
            }
            runtime.record_cursor_move(buffer_id);
        }

        CommandResult::Success
    }
}

/// Handle snippet insertion from completion confirm.
///
/// Delegates to [`SnippetExpanderRegistry`] (#542: decouple from module-snippet).
/// Falls back to raw text insertion if no expander is registered.
#[cfg_attr(coverage_nightly, coverage(off))]
fn confirm_snippet(
    runtime: &mut SessionRuntime<'_>,
    buffer_id: reovim_kernel::api::v1::BufferId,
    insert_pos: Position,
    snippet_body: &str,
) {
    let expander = runtime
        .kernel()
        .services
        .get::<SnippetExpanderRegistry>()
        .and_then(|reg| reg.get());

    if let Some(exp) = expander {
        exp.expand(runtime, buffer_id, insert_pos, snippet_body);
    } else {
        // Fallback: insert raw text if no snippet expander is available.
        runtime.insert_text(buffer_id, insert_pos, snippet_body);
    }
}

/// Dismiss the completion popup.
#[derive(Debug, Clone, Copy, Default)]
pub struct Dismiss;

impl reovim_driver_command::Command for Dismiss {
    fn id(&self) -> CommandId {
        ids::DISMISS
    }

    fn description(&self) -> &'static str {
        "Dismiss completion popup"
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl CommandHandler for Dismiss {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let state = runtime.ext_mut::<CompletionState>();
        state.close();
        CommandResult::Success
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Drain pending notifications from background threads into session state.
///
/// Delegates to [`NotificationDrainRegistry`] (#542: decouple from module-notification).
/// The notification module registers its implementation during `init()`.
#[cfg_attr(coverage_nightly, coverage(off))]
fn drain_pending_notifications(runtime: &mut SessionRuntime<'_>) {
    let Some(reg) = runtime.kernel().services.get::<NotificationDrainRegistry>() else {
        return;
    };
    let Some(drain) = reg.get() else {
        return;
    };
    drain.drain_pending(runtime);
}

/// Build a `CompletionContext` from the current buffer state.
///
/// Returns `None` if there is no active buffer or window.
#[cfg_attr(coverage_nightly, coverage(off))]
fn build_context(runtime: &SessionRuntime<'_>) -> Option<CompletionContext> {
    let buffer_id = runtime.active_buffer()?;
    let content = runtime.buffer_content(buffer_id)?;

    // Get cursor from per-client window (#471).
    let window = runtime.windows().active()?;
    let cursor = Position::new(window.cursor.line, window.cursor.column);
    let cursor_line = cursor.line;
    let cursor_col = cursor.column;

    // Extract the prefix: the word fragment before the cursor on the current line.
    let prefix = {
        let line_text = content.lines().nth(cursor_line).unwrap_or("");
        let before_cursor = if cursor_col <= line_text.len() {
            &line_text[..cursor_col]
        } else {
            line_text
        };
        before_cursor
            .rfind(|c: char| !c.is_alphanumeric() && c != '_')
            .map_or(before_cursor, |pos| &before_cursor[pos + 1..])
            .to_owned()
    };

    // Calculate byte offset of cursor.
    let cursor_offset = content
        .lines()
        .take(cursor_line)
        .map(|l| l.len() + 1) // +1 for newline
        .sum::<usize>()
        + cursor_col;

    let file_path = runtime.buffer_file_path(buffer_id);
    let language_id = file_path.as_deref().and_then(language_id_from_path);

    Some(CompletionContext {
        content,
        cursor_offset,
        line: cursor_line,
        col: cursor_col,
        prefix,
        buffer_id: buffer_id.as_usize(),
        file_path,
        language_id,
    })
}

/// Fire an asynchronous LSP completion request in the background.
///
/// The response updates the `LspCompletionSource` cache so that the NEXT
/// trigger returns LSP items via the sync `complete()` contract.
#[cfg_attr(coverage_nightly, coverage(off))]
fn fire_lsp_completion(services: &Arc<ServiceRegistry>, ctx: &CompletionContext) {
    let lang = match ctx.language_id {
        Some(ref l) => l.clone(),
        None => return,
    };
    let file_path = match ctx.file_path {
        Some(ref p) => p.clone(),
        None => return,
    };

    let lsp_registry = services.get_or_create::<LspProviderRegistry>();

    // Look up provider: try per-language key first, then Default.
    let provider = lsp_registry
        .get(&LspKey::Language(lang.clone()))
        .or_else(|| lsp_registry.get(&LspKey::Default));

    let provider = match provider {
        Some(p) if p.is_active() => p,
        _ => {
            // No active provider — try to auto-start one.
            try_auto_start_lsp(services, &file_path, &lang, &ctx.content);
            return;
        }
    };

    // Check server supports completion before sending request (#521).
    if let Some(caps) = provider.capabilities()
        && caps.completion_provider.is_none()
    {
        debug!("LSP server does not support completion");
        return;
    }

    // Build the completion request.
    let path = Path::new(&file_path);
    let uri = uri_from_path(path);
    let position = lsp_types::Position::new(
        u32::try_from(ctx.line).unwrap_or(0),
        u32::try_from(ctx.col).unwrap_or(0),
    );

    let (response_tx, response_rx) = oneshot();
    let request = LspRequest::Completion {
        uri,
        position,
        response_tx,
    };

    if !provider.send_request(request) {
        debug!("LSP completion request not accepted (channel full or closed)");
        return;
    }

    // Spawn a background thread to wait for the response and update cache.
    let lsp_source = services.get::<LspCompletionSource>();
    let notify_queue = services.get::<PendingNotificationQueue>();
    std::thread::spawn(move || {
        let Some(source) = lsp_source else {
            return;
        };
        match response_rx.recv_timeout(std::time::Duration::from_secs(10)) {
            Ok(Ok(Some(response))) => {
                let items: Vec<_> = match response {
                    lsp_types::CompletionResponse::Array(arr) => {
                        arr.iter().map(map_lsp_item).collect()
                    }
                    lsp_types::CompletionResponse::List(list) => {
                        list.items.iter().map(map_lsp_item).collect()
                    }
                };
                info!(count = items.len(), "LSP completion cache updated");
                source.update_cache(items);
            }
            Ok(Ok(None)) => {
                debug!("LSP returned no completion results");
            }
            Ok(Err(e)) => {
                warn!("LSP completion error: {e}");
                if let Some(q) = &notify_queue {
                    q.push(PendingLevel::Warning, format!("LSP completion error: {e}"));
                }
            }
            Err(_) => {
                debug!("LSP completion response timed out");
                if let Some(q) = &notify_queue {
                    q.push(PendingLevel::Info, "LSP completion timed out");
                }
            }
        }
    });
}

/// Guard to prevent concurrent LSP server starts (#521).
///
/// Stored in `ServiceRegistry`. `compare_exchange` ensures only one
/// spawn task runs at a time per service registry.
struct LspStartingGuard(AtomicBool);

impl Default for LspStartingGuard {
    fn default() -> Self {
        Self(AtomicBool::new(false))
    }
}

impl reovim_kernel::api::v1::Service for LspStartingGuard {}

/// Try to auto-start an LSP server for the given language.
///
/// Currently supports Rust (rust-analyzer) only. Spawns the server
/// in the background via the tokio runtime and registers it in
/// `LspProviderRegistry`.
#[cfg_attr(coverage_nightly, coverage(off))]
fn try_auto_start_lsp(
    services: &Arc<ServiceRegistry>,
    file_path: &str,
    lang: &str,
    buffer_content: &str,
) {
    // Only Rust is supported for now.
    if lang != "rust" {
        return;
    }

    // Prevent concurrent LSP starts (#521).
    let guard = services.get_or_create::<LspStartingGuard>();
    if guard
        .0
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        debug!("LSP server start already in progress, skipping");
        return;
    }

    let Some(root) = find_project_root(Path::new(file_path)) else {
        debug!("No project root found for {file_path}");
        guard.0.store(false, Ordering::Release);
        return;
    };

    let config = LspServerConfig::rust_analyzer(&root);

    // Delegate to LspLifecycleRegistry (#542: decouple from module-lsp).
    let Some(reg) = services.get::<LspLifecycleRegistry>() else {
        debug!("No LspLifecycleRegistry registered, cannot auto-start LSP");
        guard.0.store(false, Ordering::Release);
        return;
    };
    let Some(lifecycle) = reg.get() else {
        debug!("No LspLifecycle implementation registered");
        guard.0.store(false, Ordering::Release);
        return;
    };

    lifecycle.auto_start(
        services,
        config,
        lang.to_owned(),
        file_path.to_owned(),
        buffer_content.to_owned(),
    );
    // Guard not reset here — once the provider registers in LspProviderRegistry,
    // fire_lsp_completion() finds it and skips try_auto_start_lsp entirely.
}

/// Walk up from a file path to find the project root.
///
/// Looks for `Cargo.toml` (Rust), `package.json` (JS/TS), or `pyproject.toml` (Python).
/// Returns the directory containing the project marker file.
#[must_use]
pub fn find_project_root(file_path: &Path) -> Option<PathBuf> {
    let markers = ["Cargo.toml", "package.json", "pyproject.toml", "go.mod"];

    let mut dir = if file_path.is_file() {
        file_path.parent()?
    } else {
        file_path
    };

    loop {
        for marker in &markers {
            if dir.join(marker).exists() {
                return Some(dir.to_path_buf());
            }
        }
        dir = dir.parent()?;
    }
}

/// Derive the language ID from a file path's extension.
///
/// Maps file extensions to LSP language identifiers used for server
/// lookup and `textDocument/didOpen` notifications.
#[must_use]
pub fn language_id_from_path(path: &str) -> Option<String> {
    let ext = Path::new(path).extension()?.to_str()?;
    let lang = match ext {
        "rs" => "rust",
        "py" | "pyi" => "python",
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "js" => "javascript",
        "jsx" => "javascriptreact",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "go" => "go",
        "java" => "java",
        "lua" => "lua",
        "rb" => "ruby",
        "zig" => "zig",
        "toml" => "toml",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        _ => return None,
    };
    Some(lang.to_owned())
}

/// Collect all command handlers for registration.
#[must_use]
pub fn command_handlers() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(Trigger),
        Box::new(Next),
        Box::new(Prev),
        Box::new(Confirm),
        Box::new(Dismiss),
    ]
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_driver_command::Command};

    #[test]
    fn trigger_metadata() {
        let cmd = Trigger;
        assert_eq!(cmd.id(), ids::TRIGGER);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn next_metadata() {
        let cmd = Next;
        assert_eq!(cmd.id(), ids::NEXT);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn prev_metadata() {
        let cmd = Prev;
        assert_eq!(cmd.id(), ids::PREV);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn confirm_metadata() {
        let cmd = Confirm;
        assert_eq!(cmd.id(), ids::CONFIRM);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn dismiss_metadata() {
        let cmd = Dismiss;
        assert_eq!(cmd.id(), ids::DISMISS);
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn command_handlers_count() {
        let handlers = command_handlers();
        assert_eq!(handlers.len(), 5);
    }

    #[test]
    fn command_handlers_unique_ids() {
        let handlers = command_handlers();
        let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
        let mut deduped = ids.clone();
        deduped.sort_by_key(CommandId::name);
        deduped.dedup_by_key(|id| id.name());
        assert_eq!(ids.len(), deduped.len());
    }

    // ========================================================================
    // language_id_from_path tests
    // ========================================================================

    #[test]
    fn language_id_rust() {
        assert_eq!(language_id_from_path("src/main.rs"), Some("rust".to_owned()));
    }

    #[test]
    fn language_id_python() {
        assert_eq!(language_id_from_path("script.py"), Some("python".to_owned()));
        assert_eq!(language_id_from_path("stubs.pyi"), Some("python".to_owned()));
    }

    #[test]
    fn language_id_typescript() {
        assert_eq!(language_id_from_path("app.ts"), Some("typescript".to_owned()));
        assert_eq!(language_id_from_path("Component.tsx"), Some("typescriptreact".to_owned()));
    }

    #[test]
    fn language_id_javascript() {
        assert_eq!(language_id_from_path("index.js"), Some("javascript".to_owned()));
        assert_eq!(language_id_from_path("App.jsx"), Some("javascriptreact".to_owned()));
    }

    #[test]
    fn language_id_c_cpp() {
        assert_eq!(language_id_from_path("main.c"), Some("c".to_owned()));
        assert_eq!(language_id_from_path("util.h"), Some("c".to_owned()));
        assert_eq!(language_id_from_path("main.cpp"), Some("cpp".to_owned()));
        assert_eq!(language_id_from_path("main.cc"), Some("cpp".to_owned()));
        assert_eq!(language_id_from_path("main.cxx"), Some("cpp".to_owned()));
        assert_eq!(language_id_from_path("header.hpp"), Some("cpp".to_owned()));
    }

    #[test]
    fn language_id_other_languages() {
        assert_eq!(language_id_from_path("main.go"), Some("go".to_owned()));
        assert_eq!(language_id_from_path("Main.java"), Some("java".to_owned()));
        assert_eq!(language_id_from_path("init.lua"), Some("lua".to_owned()));
        assert_eq!(language_id_from_path("app.rb"), Some("ruby".to_owned()));
        assert_eq!(language_id_from_path("main.zig"), Some("zig".to_owned()));
    }

    #[test]
    fn language_id_config_files() {
        assert_eq!(language_id_from_path("Cargo.toml"), Some("toml".to_owned()));
        assert_eq!(language_id_from_path("data.json"), Some("json".to_owned()));
        assert_eq!(language_id_from_path("config.yaml"), Some("yaml".to_owned()));
        assert_eq!(language_id_from_path("config.yml"), Some("yaml".to_owned()));
        assert_eq!(language_id_from_path("README.md"), Some("markdown".to_owned()));
        assert_eq!(language_id_from_path("doc.markdown"), Some("markdown".to_owned()));
    }

    #[test]
    fn language_id_unknown_extension() {
        assert_eq!(language_id_from_path("file.xyz"), None);
        assert_eq!(language_id_from_path("file.wasm"), None);
    }

    #[test]
    fn language_id_no_extension() {
        assert_eq!(language_id_from_path("Makefile"), None);
        assert_eq!(language_id_from_path("/usr/bin/cat"), None);
    }

    #[test]
    fn language_id_nested_path() {
        assert_eq!(language_id_from_path("/home/user/project/src/lib.rs"), Some("rust".to_owned()));
    }

    // ========================================================================
    // find_project_root tests
    // ========================================================================

    #[test]
    fn find_project_root_from_this_crate() {
        // This crate has a Cargo.toml, so we should find it.
        let this_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands.rs");
        let root = find_project_root(&this_file);
        assert!(root.is_some());
        let root = root.unwrap();
        assert!(root.join("Cargo.toml").exists());
    }

    #[test]
    fn find_project_root_from_directory() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let root = find_project_root(&dir);
        assert!(root.is_some());
    }

    #[test]
    fn find_project_root_nonexistent() {
        // Root "/" has no Cargo.toml.
        let root = find_project_root(Path::new("/nonexistent/path/file.rs"));
        assert!(root.is_none());
    }

    // ========================================================================
    // LspStartingGuard tests (#521)
    // ========================================================================

    #[test]
    fn lsp_starting_guard_prevents_concurrent_starts() {
        let guard = LspStartingGuard::default();
        // First acquisition succeeds.
        assert!(
            guard
                .0
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
        );
        // Second acquisition fails — already in progress.
        assert!(
            guard
                .0
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                .is_err()
        );
    }

    #[test]
    fn lsp_starting_guard_resets_after_completion() {
        let guard = LspStartingGuard::default();
        guard
            .0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .unwrap();
        // Reset.
        guard.0.store(false, Ordering::Release);
        // Now re-acquisition succeeds.
        assert!(
            guard
                .0
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
        );
    }

    #[test]
    fn lsp_starting_guard_service_trait() {
        use reovim_kernel::api::v1::ServiceRegistry;
        let registry = ServiceRegistry::new();
        let guard = registry.get_or_create::<LspStartingGuard>();
        assert!(!guard.0.load(Ordering::Relaxed));
    }
}
