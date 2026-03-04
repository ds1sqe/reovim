//! Command handlers for the completion popup.
//!
//! Commands for triggering, navigating, confirming, and dismissing completions.
//! These handlers operate on `CompletionState` stored in the session's
//! `ExtensionMap`.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use {
    reovim_driver_command::CommandHandler,
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_completion::{CompletionContext, CompletionSourceRegistry},
    reovim_driver_lsp::{
        LspKey, LspProvider, LspProviderRegistry, LspRequest, LspServerConfig, uri_from_path,
    },
    reovim_driver_session::{
        BufferApi, ChangeTracker, ExtensionApi, ModeApi, Selection, SessionRuntime,
        TransitionContext,
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
#[cfg_attr(coverage_nightly, coverage(off))]
fn confirm_snippet(
    runtime: &mut SessionRuntime<'_>,
    buffer_id: reovim_kernel::api::v1::BufferId,
    insert_pos: Position,
    snippet_body: &str,
) {
    use reovim_module_snippet::{
        engine::ActiveSnippet, ids as snippet_ids, parser, state::SnippetSessionState,
        variables::VariableContext,
    };

    // Parse the snippet body.
    let Ok(body) = parser::parse(snippet_body) else {
        // Fallback: insert raw text if parsing fails.
        runtime.insert_text(buffer_id, insert_pos, snippet_body);
        return;
    };

    // Build variable context.
    let var_ctx = VariableContext {
        file_path: runtime.buffer_file_path(buffer_id),
        line_number: insert_pos.line,
        ..VariableContext::empty()
    };

    // Expand the snippet.
    let (expanded_text, active_snippet) = ActiveSnippet::expand(&body, insert_pos, &var_ctx);
    runtime.insert_text(buffer_id, insert_pos, &expanded_text);

    let has_tab_stops = !active_snippet.is_done();
    let first_stop_range = active_snippet.current().map(|ts| (ts.start, ts.end));

    // Store active snippet state.
    let state = runtime.ext_mut::<SnippetSessionState>();
    state.active = Some(active_snippet);

    // Enter snippet navigation mode if there are tab stops.
    if has_tab_stops {
        runtime.push_mode(snippet_ids::NAVIGATING_MODE, TransitionContext::new());
        if let Some((start, end)) = first_stop_range {
            if let Some(w) = runtime.windows_mut().active_mut() {
                w.cursor.line = start.line;
                w.cursor.column = start.column;
                if start == end {
                    w.selection = None;
                } else {
                    w.selection = Some(Selection::character(start, end));
                }
            }
            runtime.record_cursor_move(buffer_id);
            if start != end {
                runtime.record_selection_change(buffer_id);
            }
        }
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

/// Drain pending notifications from background threads into `NotificationState`.
///
/// Background threads (LSP completion, auto-start) push to
/// `PendingNotificationQueue` in `ServiceRegistry`. This function drains the
/// queue and forwards them to the per-client `NotificationState` so they
/// appear as toast messages in the TUI/web client.
#[cfg_attr(coverage_nightly, coverage(off))]
fn drain_pending_notifications(runtime: &mut SessionRuntime<'_>) {
    use reovim_module_notification::{NotificationLevel, NotificationState};

    let queue = runtime.kernel().services.get::<PendingNotificationQueue>();
    let Some(queue) = queue else { return };
    let pending = queue.drain();
    if pending.is_empty() {
        return;
    }

    let state = runtime.ext_mut::<NotificationState>();
    for notification in &pending {
        let level = match notification.level {
            PendingLevel::Info => NotificationLevel::Info,
            PendingLevel::Success => NotificationLevel::Success,
            PendingLevel::Warning => NotificationLevel::Warning,
            PendingLevel::Error => NotificationLevel::Error,
        };
        state.push(level, &notification.title);
    }
    runtime
        .take_changes()
        .record_extension_change("notification".into());
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

    let Some(root) = find_project_root(Path::new(file_path)) else {
        debug!("No project root found for {file_path}");
        return;
    };

    let config = LspServerConfig::rust_analyzer(&root);
    let services_clone = Arc::clone(services);
    let lang_owned = lang.to_owned();
    let file_path_owned = file_path.to_owned();
    let content_owned = buffer_content.to_owned();
    let notify_queue = services.get::<PendingNotificationQueue>();

    if let Some(q) = &notify_queue {
        q.push(PendingLevel::Info, "Starting rust-analyzer...");
    }

    // Use tokio runtime to spawn the async LSP server start.
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async move {
            info!(root = ?root, "Auto-starting rust-analyzer");
            match reovim_module_lsp::LspSaturator::start(config).await {
                Ok(lsp_handle) => {
                    // Send DidOpen so the server knows about the file.
                    let uri = uri_from_path(Path::new(&file_path_owned));
                    lsp_handle.send_request(LspRequest::DidOpen {
                        uri,
                        language_id: lang_owned.clone(),
                        version: 1,
                        content: content_owned,
                    });

                    // Register in LspProviderRegistry.
                    let registry = services_clone.get_or_create::<LspProviderRegistry>();
                    registry.register(LspKey::Language(lang_owned), Arc::new(lsp_handle));
                    info!("rust-analyzer registered and ready");
                    if let Some(q) = &notify_queue {
                        q.push(PendingLevel::Success, "rust-analyzer ready");
                    }
                }
                Err(e) => {
                    warn!("Failed to start rust-analyzer: {e}");
                    if let Some(q) = &notify_queue {
                        q.push(
                            PendingLevel::Warning,
                            format!("Failed to start rust-analyzer: {e}"),
                        );
                    }
                }
            }
        });
    }
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
}
