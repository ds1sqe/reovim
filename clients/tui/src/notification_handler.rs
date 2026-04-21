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
//! In proto v3 the per-field notification variants (`ModeChanged`, `CursorMoved`,
//! `SelectionChanged`, `OptionChanged`, `BufferModified`, `ResizeRequest`) are replaced
//! by the generic `ProjectionUpdated` dispatch and `SurfaceChanged`.
//! Only server-lifecycle notifications (`LayoutChanged`, `RenderComplete`, `Detach`,
//! `Capture`, `Presence`) remain as typed arms.

use reovim_protocol::v3::{Notification, notification::Payload};

use reovim_client_driver::{BufferId, BufferUpdateEvent, ClientModule};

use crate::{
    RemoteClient, TuiCoreState,
    core_helpers::apply_layout_notification,
    grpc_client::{TuiGrpcClient, TuiGrpcError},
    projection_handlers::handle_projection_updated,
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

    /// Called when a surface change (resize) is received.
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
    /// Used for `BufferOpened` and `LayoutChanged` — avoids calling
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
        // ─────────────────────────────────────────────────────────────
        // Generic projection dispatch (v3: replaces ModeChanged,
        // CursorMoved, SelectionChanged, OptionChanged, BufferModified)
        // ─────────────────────────────────────────────────────────────
        Payload::ProjectionUpdated(p) => {
            handle_projection_updated(ctx, p).await?;
            Ok(NotificationResult::Redraw)
        }

        // ─────────────────────────────────────────────────────────────
        // Surface change (v3: replaces ResizeRequest)
        // ─────────────────────────────────────────────────────────────
        Payload::SurfaceChanged(surface_payload) => {
            let my_client_id = ctx.state_mut().my_client_id;
            if surface_payload.target_client_id != 0
                && surface_payload.target_client_id != my_client_id
            {
                tracing::trace!(
                    target_client_id = surface_payload.target_client_id,
                    my_client_id,
                    "Ignoring surface change for different client"
                );
                return Ok(NotificationResult::NoRedraw);
            }

            // Dispatch the opaque SurfaceDescriptorProto through the
            // process-global registry populated at TUI startup by
            // `surface_decoding::register_builtin_surface_handlers`.
            // Kind-specific decode policy lives in the ext handler
            // crate (e.g. `reovim-tui-mod-surface-descriptor-cell-grid`
            // for kind = 0x0001), not here.
            if let Some(desc) = &surface_payload.surface {
                decode_surface_descriptor(ctx, desc);
            }
            Ok(NotificationResult::Redraw)
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
            //
            // NOTE: In v3, get_buffer_content is a no-op stub pending the
            // server-side `text.buffer_lines` projection emitter.
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
                {
                    Ok(content) => {
                        if !content.lines.is_empty() {
                            ctx.state_mut().buffer_cache.insert(buf_id, content.lines);
                            fetched.push(buf_id);
                        }
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
                        cursor_line: 0, // Default; updated via ProjectionUpdated
                        cursor_col: 0,
                        buffer_id: client.buffer_id,
                        mode: String::new(), // Default; updated via ProjectionUpdated
                        selection: None,     // Default; updated via ProjectionUpdated
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
                    let buffer_changed = old.is_none_or(|o| o.buffer_id != client.buffer_id);

                    let (cursor_line, cursor_col) = if buffer_changed {
                        (0, 0)
                    } else {
                        (old.map_or(0, |c| c.cursor_line), old.map_or(0, |c| c.cursor_col))
                    };

                    let mode = if buffer_changed {
                        String::new()
                    } else {
                        old.map_or_else(String::new, |c| c.mode.clone())
                    };

                    let selection = if buffer_changed {
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
                            mode,
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
            // Other notifications (BufferOpened, BufferClosed, CaptureResponse, etc.)
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
    // v3 BufferInfo dropped codec_metadata; encoding defaults to "utf-8"
    // until a `text.codec_metadata` projection lands.
    let encoding = "utf-8";
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

/// Dispatch buffer list to all client modules.
///
/// Fetches the full buffer list via `list_buffers()` gRPC, filters to
/// buffers attached to the client's windows, and broadcasts a JSON payload
/// to all modules via `on_notification()`. Each module decides whether the
/// `buffer_list` payload is relevant. Called alongside
/// `dispatch_buffer_metadata` on layout/buffer changes.
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) async fn dispatch_buffer_list(
    state: &TuiCoreState,
    client: &mut TuiGrpcClient,
    extensions: &mut [Box<dyn ClientModule>],
) {
    // Collect buffer IDs from client's windows.
    let window_buf_ids: Vec<u64> = state.windows.iter().filter_map(|w| w.buffer_id).collect();
    if window_buf_ids.is_empty() {
        return;
    }

    let Ok(response) = client.list_buffers().await else {
        return;
    };

    // Build JSON entries for buffers visible in client's windows.
    let mut entries = String::from("[");
    let mut first = true;
    for info in &response.buffers {
        if !window_buf_ids.contains(&info.id) {
            continue;
        }
        let name = if info.name.is_empty() {
            info.path.as_deref().unwrap_or("[No Name]")
        } else {
            &info.name
        };
        let filetype = guess_filetype(name);
        let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
        if !first {
            entries.push(',');
        }
        first = false;
        let _ = std::fmt::Write::write_fmt(
            &mut entries,
            format_args!(
                r#"{{"id":{},"name":"{escaped}","modified":{},"filetype":"{filetype}"}}"#,
                info.id, info.modified
            ),
        );
    }
    entries.push(']');

    let window_count = state.windows.len();
    let json =
        format!(r#"{{"type":"buffer_list","buffers":{entries},"window_count":{window_count}}}"#);

    for ext in extensions {
        ext.on_notification(&json);
    }
}

/// Dispatch a `SurfaceDescriptorProto` through the process-global
/// registry (populated at TUI startup in
/// [`crate::surface_decoding`]).
///
/// Kind-specific decode policy lives in the ext handler crates; this
/// function only routes by `kind`, downcasts the typed result, and
/// applies the side-effect (window resize, viewport hint, DPI change,
/// …) to the notification context. Unknown kinds log at `trace!` and
/// are otherwise ignored.
fn decode_surface_descriptor<C: NotificationContext>(
    ctx: &mut C,
    desc: &reovim_protocol::v3::SurfaceDescriptorProto,
) {
    use reovim_client_subsys_codec::SurfaceDescriptorHandlerRegistry;
    use reovim_tui_mod_surface_descriptor_cell_grid::CellGridSurfaceInfo;

    let Some(registry) = crate::surface_decoding::global_registry() else {
        tracing::warn!(
            "surface-descriptor registry not initialised — \
             check clients/tui/src/lib.rs startup path calls \
             surface_decoding::register_builtin_surface_handlers()",
        );
        return;
    };

    // Proto wire field is u32 but the envelope contract is u16
    // (comment in uapi/protocol/proto/reovim/v3/common.proto:69).
    // Values above u16::MAX are guaranteed unregistered; log trace and
    // bail without surfacing the decode error.
    let Ok(kind) = u16::try_from(desc.kind) else {
        tracing::trace!(
            kind = desc.kind,
            "surface descriptor kind exceeds u16::MAX — no handler registered",
        );
        return;
    };

    let Some(handler) = registry.get(kind) else {
        tracing::trace!(
            kind,
            "unknown surface descriptor kind — no handler registered",
        );
        return;
    };

    match handler.decode(&desc.body) {
        Ok(boxed) => {
            if let Ok(info) = boxed.downcast::<CellGridSurfaceInfo>() {
                let (w, h) = (info.width, info.height);
                if w > 0 && h > 0 {
                    tracing::debug!(
                        width = w,
                        height = h,
                        "SurfaceChanged (cell-grid resize)",
                    );
                    let state = ctx.state_mut();
                    state.width = w;
                    state.height = h;
                    ctx.on_resize(w, h);
                }
            } else {
                tracing::warn!(
                    kind = desc.kind,
                    "surface descriptor boxed type mismatch \
                     — handler registered under this kind returned \
                     a type this routing layer doesn't recognise",
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                kind = desc.kind,
                error = %e,
                "surface descriptor decode failed",
            );
        }
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
