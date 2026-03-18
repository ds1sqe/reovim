//! Unified notification handler for interactive and headless TUI.
//!
//! This module provides a single source of truth for handling server
//! notifications. `TuiApp<O: TuiOutput>` implements
//! the `NotificationContext` trait and uses `handle_notification()`.
//!
//! # Design
//!
//! The `NotificationContext` trait provides:
//! - Access to shared state (`state_mut()`)
//! - Access to gRPC client (`client_mut()`)
//! - Optional hooks for interactive-specific behavior (`on_buffer_modified()`)
//!
//! Default hook implementations are no-ops, allowing headless TUI to skip
//! syntax highlighting refresh while interactive TUI provides real implementations.

use reovim_protocol::v2::{
    Notification, notification::Payload, option_changed_payload::Value as OptionValue,
};

use reovim_client_driver::{
    BufferId, BufferUpdateEvent, ClientModule, OptionValue as ClientOptionValue,
};

use crate::{
    CursorPosition, RemoteClient, SelectionState, TuiCoreState,
    core_helpers::apply_layout_notification,
    grpc_client::{TuiGrpcClient, TuiGrpcError},
};

/// Context trait for notification handling.
///
/// Provides access to shared state and client, plus optional hooks
/// for TUI-specific behavior (syntax refresh, theme changes, etc.).
pub trait NotificationContext {
    /// Get mutable reference to the core state.
    fn state_mut(&mut self) -> &mut TuiCoreState;

    /// Get mutable reference to the gRPC client.
    fn client_mut(&mut self) -> &mut TuiGrpcClient;

    /// Called when a buffer is modified.
    ///
    /// Default implementation does nothing. Interactive TUI overrides
    /// to refresh syntax tokens.
    #[allow(unused_variables)]
    fn on_buffer_modified(&mut self, buffer_id: u64) {
        // Default: no-op
    }

    /// Called when an option changes.
    ///
    /// Default implementation does nothing. Interactive TUI overrides
    /// to handle colorscheme changes.
    #[allow(unused_variables)]
    fn on_option_changed(&mut self, name: &str, value: Option<OptionValue>) {
        // Default: no-op
    }

    /// Called when a capture request is received.
    ///
    /// Returns the frame content if this TUI should handle the request,
    /// or None if the request is for a different client.
    ///
    /// Default implementation returns None. Both TUIs override this
    /// to provide capture handling.
    #[allow(unused_variables)]
    fn on_capture_request(
        &mut self,
        request_id: u64,
        format: &str,
        target_client_id: u64,
    ) -> Option<String> {
        None
    }

    /// Called when a resize request is received.
    ///
    /// Default implementation does nothing. Headless TUI overrides
    /// to resize its frame buffer.
    #[allow(unused_variables)]
    fn on_resize(&mut self, width: u16, height: u16) {
        // Default: no-op (interactive TUI uses screen.resize() separately)
    }

    /// Get mutable access to TUI extensions for notification dispatch.
    ///
    /// Extensions own their state and handle notifications generically.
    /// The engine dispatches via `kind()` matching — zero extension knowledge.
    fn extensions_mut(&mut self) -> &mut [Box<dyn ClientModule>];
}

/// Result of notification handling.
#[derive(Debug)]
pub enum NotificationResult {
    /// Notification handled successfully, redraw needed.
    Redraw,
    /// Redraw needed AND buffer metadata should be refreshed.
    ///
    /// Used for `BufferModified` and `LayoutChanged` — avoids calling
    /// `dispatch_buffer_metadata` on every notification (#691).
    RedrawWithMetadata,
    /// Notification handled, no redraw needed.
    NoRedraw,
    /// Client should stop (detach received).
    Stop,
}

/// Handle a server notification using the provided context.
///
/// This is the single source of truth for notification handling. Both
/// interactive and headless TUI call this function.
///
/// # Arguments
///
/// * `ctx` - Notification context (interactive or headless TUI)
/// * `notif` - Notification from server
///
/// # Returns
///
/// Returns the result indicating what action to take.
///
/// # Errors
///
/// Returns an error if buffer refetch fails.
#[allow(clippy::too_many_lines)]
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn handle_notification<C: NotificationContext>(
    ctx: &mut C,
    notif: Notification,
) -> Result<NotificationResult, TuiGrpcError> {
    let Some(payload) = notif.payload else {
        return Ok(NotificationResult::NoRedraw);
    };

    match payload {
        Payload::ModeChanged(mode) => {
            let is_insert = mode.is_insert;
            let (is_local, mode_display) = {
                let state = ctx.state_mut();
                let local = mode.client_id == state.my_client_id;
                if local {
                    state.mode_name = mode.name;
                    state.mode_display = mode.display;
                    state.set_insert_mode(is_insert);
                    (true, state.mode_display.clone())
                } else {
                    if let Some(remote) = state.other_clients.get_mut(&mode.client_id) {
                        remote.mode.clone_from(&mode.display);
                    }
                    (false, String::new())
                }
            };
            // Notify extensions of mode change (after dropping state borrow)
            if is_local {
                for ext in ctx.extensions_mut() {
                    ext.on_mode_change(&mode_display);
                }
            }
            Ok(NotificationResult::Redraw)
        }

        Payload::CursorMoved(cursor) => {
            let (is_local, buffer_id, cursor_line, cursor_col) = {
                let state = ctx.state_mut();
                let local = cursor.client_id == state.my_client_id;

                // Debug: trace cursor notifications to diagnose position bugs
                tracing::debug!(
                    notif_client_id = cursor.client_id,
                    my_client_id = state.my_client_id,
                    is_local = local,
                    window_id = cursor.window_id,
                    position = ?cursor.position,
                    "CursorMoved notification"
                );

                if let Some(pos) = cursor.position {
                    if local {
                        // Use focused_window_id as storage key: when per-client compositor
                        // is active (#474) the IDs match, but this also handles edge cases
                        // where cursor.window_id differs from focused_window_id (e.g., when
                        // no compositor is loaded).
                        let key = state.focused_window_id;
                        state.update_local_cursor(key, pos.line, pos.column);
                    } else {
                        state.update_remote_cursor(cursor.client_id, pos.line, pos.column);
                    }
                    let bid = state.get_focused_buffer_id().unwrap_or(0);
                    #[allow(clippy::cast_possible_truncation)]
                    (local, bid, pos.line as usize, pos.column as usize)
                } else {
                    (local, 0, 0, 0)
                }
            };
            // Notify extensions of cursor update (after dropping state borrow)
            if is_local {
                #[allow(clippy::cast_possible_truncation)]
                let bid = BufferId(buffer_id as usize);
                for ext in ctx.extensions_mut() {
                    ext.on_cursor_update(bid, cursor_line, cursor_col);
                }
            }
            Ok(NotificationResult::Redraw)
        }

        Payload::BufferModified(buf) => {
            let buffer_id = buf.buffer_id;

            // Refetch buffer content (keep stale data until refetch completes
            // to avoid race condition where capture reads empty cache)
            match ctx
                .client_mut()
                .get_buffer_content(Some(buffer_id), None, None)
                .await
            {
                Ok(content) => {
                    ctx.state_mut()
                        .buffer_cache
                        .insert(buffer_id, content.lines);
                }
                Err(e) => {
                    tracing::warn!(buffer_id, error = %e, "Failed to refetch buffer");
                }
            }

            // Notify extensions of buffer content change
            let lines = ctx.state_mut().buffer_cache.get(&buffer_id).cloned();
            if let Some(lines) = &lines {
                #[allow(clippy::cast_possible_truncation)]
                let total = lines.len();
                let event = BufferUpdateEvent {
                    #[allow(clippy::cast_possible_truncation)]
                    buffer_id: BufferId(buffer_id as usize),
                    revision: 0,
                    changed_range: 0..total,
                    new_lines: lines.clone(),
                    total_lines: total,
                };
                for ext in ctx.extensions_mut() {
                    ext.on_buffer_update(&event);
                }
            }

            // Notify context for optional syntax refresh
            ctx.on_buffer_modified(buffer_id);

            Ok(NotificationResult::RedrawWithMetadata)
        }

        Payload::LayoutChanged(layout) => {
            let state = ctx.state_mut();
            let is_local = layout.client_id == 0 || layout.client_id == state.my_client_id;

            if is_local {
                // Local client: apply full layout update (windows + focus)
                apply_layout_notification(state, layout.focused_window_id, layout.windows);
                // #401 Phase 5: Store tab info from server
                state.active_tab_id = layout.active_tab_id;
                state.tabs = layout.tabs;
            } else {
                // Remote client changed layout: update window list but keep our focus
                let our_focus = state.focused_window_id;
                apply_layout_notification(state, layout.focused_window_id, layout.windows);
                state.focused_window_id = our_focus;
                for w in &mut state.windows {
                    w.focused = w.window_id == our_focus;
                }
            }

            // Fetch content for any newly visible buffers not yet in cache.
            // This covers set_window_buffer() which only emits LayoutChanged,
            // not BufferModified (e.g., picker file open, buffer switch).
            let missing: Vec<u64> = {
                let state = ctx.state_mut();
                state
                    .windows
                    .iter()
                    .filter_map(|w| w.buffer_id)
                    .filter(|id| !state.buffer_cache.contains_key(id))
                    .collect()
            };
            let mut fetched = Vec::new();
            for buf_id in missing {
                match ctx
                    .client_mut()
                    .get_buffer_content(Some(buf_id), None, None)
                    .await
                {
                    Ok(content) => {
                        ctx.state_mut().buffer_cache.insert(buf_id, content.lines);
                        fetched.push(buf_id);
                    }
                    Err(e) => {
                        tracing::warn!(
                            buffer_id = buf_id,
                            error = %e,
                            "Failed to fetch buffer after layout change"
                        );
                    }
                }
            }

            // Notify extensions about newly fetched buffer content
            for buf_id in fetched {
                let lines = ctx.state_mut().buffer_cache.get(&buf_id).cloned();
                if let Some(lines) = &lines {
                    #[allow(clippy::cast_possible_truncation)]
                    let total = lines.len();
                    let event = BufferUpdateEvent {
                        #[allow(clippy::cast_possible_truncation)]
                        buffer_id: BufferId(buf_id as usize),
                        revision: 0,
                        changed_range: 0..total,
                        new_lines: lines.clone(),
                        total_lines: total,
                    };
                    for ext in ctx.extensions_mut() {
                        ext.on_buffer_update(&event);
                    }
                }
            }

            // Dispatch on_buffer_focus for the focused buffer
            if is_local {
                let focused_buf = ctx.state_mut().get_focused_buffer_id();
                if let Some(buf_id) = focused_buf {
                    #[allow(clippy::cast_possible_truncation)]
                    let bid = BufferId(buf_id as usize);
                    for ext in ctx.extensions_mut() {
                        ext.on_buffer_focus(bid);
                    }
                }
            }

            Ok(NotificationResult::RedrawWithMetadata)
        }

        Payload::RenderComplete(_) => Ok(NotificationResult::Redraw),

        Payload::Detach(detach) => {
            tracing::info!("Server requested detach: {}", detach.reason);
            Ok(NotificationResult::Stop)
        }

        Payload::OptionChanged(opt) => {
            let value = opt.value.clone();
            ctx.on_option_changed(&opt.name, value.clone());

            // Dispatch to extensions so ClientModules (e.g., LineNumbers)
            // receive option changes like :set nu / :set rnu.
            if let Some(client_value) = value.map(proto_to_client_option) {
                for ext in ctx.extensions_mut() {
                    ext.on_option_changed(&opt.name, &client_value);
                }
            }

            Ok(NotificationResult::Redraw)
        }

        Payload::PresenceJoined(p) => {
            if let Some(client) = p.client {
                let state = ctx.state_mut();
                if client.client_id != state.my_client_id {
                    tracing::debug!(
                        client_id = client.client_id,
                        display_name = %client.display_name,
                        buffer_id = ?client.buffer_id,
                        "PresenceJoined: Adding remote client"
                    );
                    state.add_remote_client(RemoteClient {
                        client_id: client.client_id,
                        display_name: client.display_name,
                        cursor_line: 0, // Updated via CursorMoved
                        cursor_col: 0,
                        buffer_id: client.buffer_id,
                        mode: client.mode,
                        selection: None, // Updated via SelectionChanged
                    });
                }
            }
            Ok(NotificationResult::Redraw)
        }

        Payload::PresenceUpdated(p) => {
            if let Some(client) = p.client {
                let state = ctx.state_mut();
                if client.client_id != state.my_client_id {
                    let old = state.other_clients.get(&client.client_id);

                    // Check if buffer changed - if so, reset cursor to (0,0)
                    // The old cursor position doesn't make sense in a new buffer
                    let buffer_changed = old.is_none_or(|o| o.buffer_id != client.buffer_id);

                    let (cursor_line, cursor_col) = if buffer_changed {
                        // Reset cursor for new buffer context
                        // Server will send CursorMoved with actual position
                        (0, 0)
                    } else {
                        // Preserve existing cursor position within same buffer
                        (old.map_or(0, |c| c.cursor_line), old.map_or(0, |c| c.cursor_col))
                    };

                    let selection = if buffer_changed {
                        // Also clear selection on buffer switch
                        None
                    } else {
                        old.and_then(|c| c.selection.clone())
                    };

                    state.other_clients.insert(
                        client.client_id,
                        RemoteClient {
                            client_id: client.client_id,
                            display_name: client.display_name,
                            cursor_line,
                            cursor_col,
                            buffer_id: client.buffer_id,
                            mode: client.mode,
                            selection,
                        },
                    );
                }
            }
            Ok(NotificationResult::Redraw)
        }

        Payload::PresenceLeft(p) => {
            let state = ctx.state_mut();
            state.remove_remote_client(p.client_id);
            Ok(NotificationResult::Redraw)
        }

        Payload::SelectionChanged(sel) => {
            let state = ctx.state_mut();
            let is_local = sel.client_id == state.my_client_id;

            let selection = if sel.has_selection {
                sel.selection.map(|s| {
                    let start = s
                        .start
                        .map_or_else(CursorPosition::default, |p| CursorPosition {
                            line: p.line,
                            column: p.column,
                        });
                    let end = s
                        .end
                        .map_or_else(CursorPosition::default, |p| CursorPosition {
                            line: p.line,
                            column: p.column,
                        });
                    SelectionState {
                        start,
                        end,
                        mode: sel.visual_mode.clone().unwrap_or_default(),
                    }
                })
            } else {
                None
            };

            if is_local {
                state.update_local_selection(sel.window_id, selection);
            } else {
                state.update_remote_selection(sel.client_id, selection);
            }

            Ok(NotificationResult::Redraw)
        }

        Payload::ResizeRequest(resize_req) => {
            // Only handle resize targeted at us (0 = no target for backward compat)
            let my_client_id = ctx.state_mut().my_client_id;
            if resize_req.target_client_id != 0 && resize_req.target_client_id != my_client_id {
                tracing::trace!(
                    target_client_id = resize_req.target_client_id,
                    my_client_id,
                    "Ignoring resize request for different client"
                );
                return Ok(NotificationResult::NoRedraw);
            }

            #[allow(clippy::cast_possible_truncation)]
            let width = resize_req.width as u16;
            #[allow(clippy::cast_possible_truncation)]
            let height = resize_req.height as u16;

            if width > 0 && height > 0 {
                tracing::debug!(width, height, "Resize request");
                let state = ctx.state_mut();
                state.width = width;
                state.height = height;
                // Notify context for frame buffer resize (headless TUI)
                ctx.on_resize(width, height);
            }
            Ok(NotificationResult::Redraw)
        }

        Payload::CaptureRequest(capture_req) => {
            // Check if this request is for us
            let my_client_id = ctx.state_mut().my_client_id;
            if capture_req.target_client_id != my_client_id {
                tracing::trace!(
                    target_client_id = capture_req.target_client_id,
                    my_client_id,
                    "Ignoring capture request for different client"
                );
                return Ok(NotificationResult::NoRedraw);
            }

            // Let context handle capture
            if let Some(content) = ctx.on_capture_request(
                capture_req.request_id,
                &capture_req.format,
                capture_req.target_client_id,
            ) {
                // Submit capture response - scope client reference tightly
                let (width, height) = {
                    let state = ctx.state_mut();
                    (state.width, state.height)
                };
                let result = {
                    let client = ctx.client_mut();
                    client
                        .submit_capture_response(
                            capture_req.request_id,
                            u64::from(width),
                            u64::from(height),
                            &capture_req.format,
                            content,
                        )
                        .await
                };

                match result {
                    Ok(reply) => {
                        tracing::debug!(
                            request_id = capture_req.request_id,
                            ok = reply.ok,
                            "Submitted capture response"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            request_id = capture_req.request_id,
                            error = %e,
                            "Failed to submit capture response"
                        );
                    }
                }
            }
            Ok(NotificationResult::NoRedraw)
        }

        Payload::ExtensionUpdated(ext) => {
            let is_local = {
                let state = ctx.state_mut();
                ext.client_id == 0 || ext.client_id == state.my_client_id
            };

            if is_local {
                // Generic dispatch — engine has ZERO knowledge of specific extensions
                for extension in ctx.extensions_mut() {
                    if extension.kind() == ext.kind {
                        extension.on_notification(&ext.data);
                    }
                }
            }
            Ok(NotificationResult::Redraw)
        }

        _ => {
            // Other notifications - trigger redraw
            Ok(NotificationResult::Redraw)
        }
    }
}

/// Fetch buffer list and dispatch metadata as a JSON notification to
/// the statusline module (kind = "statusline"). Called by `TuiApp`
/// after layout changes and buffer modifications (#661).
///
/// The JSON payload format: `{"filename":"...", "filetype":"...",
/// "encoding":"...", "modified":bool, "readonly":bool}`
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) async fn dispatch_buffer_metadata(
    state: &TuiCoreState,
    client: &mut TuiGrpcClient,
    extensions: &mut [Box<dyn ClientModule>],
) {
    let Some(focused_id) = state.get_focused_buffer_id() else {
        return;
    };

    let Ok(response) = client.list_buffers().await else {
        return;
    };

    let Some(info) = response.buffers.iter().find(|b| b.id == focused_id) else {
        return;
    };

    // Build JSON payload from BufferInfo proto fields
    let filename = if info.name.is_empty() {
        info.path.as_deref().unwrap_or("")
    } else {
        &info.name
    };
    let filetype = guess_filetype(filename);
    let encoding = info
        .codec_metadata
        .as_ref()
        .and_then(|m| m.line_ending.as_deref())
        .map_or("utf-8", |le| if le == "crlf" { "crlf" } else { "utf-8" });
    let modified = info.modified;
    let readonly = info.readonly.unwrap_or(false);

    // Escape filename for JSON (simple: replace backslash and quotes)
    let escaped = filename.replace('\\', "\\\\").replace('"', "\\\"");
    let json = format!(
        r#"{{"filename":"{escaped}","filetype":"{filetype}","encoding":"{encoding}","modified":{modified},"readonly":{readonly}}}"#
    );

    for ext in extensions {
        if ext.kind() == "statusline" {
            ext.on_notification(&json);
        }
    }
}

/// Convert a proto `OptionValue` to a client-driver `OptionValue`.
fn proto_to_client_option(value: OptionValue) -> ClientOptionValue {
    match value {
        OptionValue::BoolValue(b) => ClientOptionValue::Bool(b),
        OptionValue::IntValue(i) => ClientOptionValue::Integer(i),
        OptionValue::StringValue(s) => ClientOptionValue::String(s),
    }
}

/// Guess filetype from filename extension.
fn guess_filetype(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        "jsx" => "jsx",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "cxx" | "cc" | "hpp" => "cpp",
        "java" => "java",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        "sh" | "bash" => "bash",
        "html" | "htm" => "html",
        "css" => "css",
        "sql" => "sql",
        "lua" => "lua",
        "rb" => "ruby",
        "xml" => "xml",
        "txt" => "text",
        _ => "",
    }
}

#[cfg(test)]
#[path = "notification_handler_tests.rs"]
mod tests;
