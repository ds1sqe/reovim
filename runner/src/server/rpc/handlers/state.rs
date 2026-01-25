//! State-related RPC handlers.
//!
//! Handlers for `state/mode`, `state/cursor`, and related methods.

use {
    reovim_driver_display::Rect,
    reovim_kernel::api::v1::BufferId as KernelBufferId,
    reovim_protocol::v1::{
        BufferId as ProtocolBufferId, CursorInfo, ModeInfo, Position, ScreenInfo, SelectionInfo,
        SelectionMode, WireLayerId, WireLayoutInfo, WireRect, WireWindowId, WireWindowPlacement,
        WireZone,
    },
};

use super::super::dispatcher::{HandlerFuture, RpcContext};

/// Handler for `state/mode` method.
///
/// Returns the current mode information from the session state.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "state/mode", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"focus": "Editor", "edit_mode": "Normal", ...}}
/// ```
///
/// # Panics
///
/// This function will not panic as `ModeInfo` serialization is infallible.
#[must_use]
pub fn state_mode(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        // Get the current mode from session state
        let mode_id = ctx.session.current_mode().await;

        // Get the display text from mode registry
        let display = ctx
            .session
            .with_state(|state| state.mode_registry.display_name(&mode_id).to_string())
            .await;

        let mode_info = ModeInfo {
            focus: "Editor".to_string(),
            edit_mode: mode_id.name().to_string(),
            sub_mode: "None".to_string(),
            display,
        };

        Ok(serde_json::to_value(mode_info).expect("ModeInfo serialization cannot fail"))
    })
}

/// Handler for `state/cursor` method.
///
/// Returns the cursor position in this client's active buffer.
///
/// # Per-Client Viewport
///
/// This handler reads from the client's viewport to determine the active buffer,
/// allowing different clients to view different buffers simultaneously.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "state/cursor", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"line": 0, "column": 0}}
/// ```
///
/// # Panics
///
/// This function will not panic as `CursorInfo` serialization is infallible.
#[must_use]
pub fn state_cursor(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        // Read client's active buffer from viewport (Level 2 lock)
        let active_buffer = {
            let viewport = ctx.client.viewport().read().await;
            viewport.active_buffer
        }; // Lock dropped before acquiring session lock

        // Query cursor position from session state
        // Falls back to session's active buffer if client viewport has none,
        // then to (0, 0) if no buffer at all
        let cursor = ctx
            .session
            .with_state(|state| {
                // Try client's active buffer first, fall back to session's active buffer
                let buffer_id = active_buffer.or_else(|| state.session_active_buffer());

                if let Some(buffer_id) = buffer_id
                    && let Some(buffer_arc) = state.app.kernel.buffers.get(buffer_id)
                {
                    let pos = buffer_arc.read().position();
                    return CursorInfo {
                        line: pos.line,
                        column: pos.column,
                    };
                }
                // Fallback when no buffer at all
                CursorInfo { line: 0, column: 0 }
            })
            .await;

        Ok(serde_json::to_value(cursor).expect("CursorInfo serialization cannot fail"))
    })
}

/// Handler for `state/screen` method.
///
/// Returns the current screen/viewport information for this client.
///
/// # Per-Client Viewport
///
/// This handler reads from the client's viewport, allowing each client to have
/// independent terminal dimensions and active buffer.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "state/screen", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"width": 80, "height": 24, "active_buffer_id": 0, "window_count": 1}}
/// ```
///
/// # Panics
///
/// This function will not panic as `ScreenInfo` serialization is infallible.
#[must_use]
pub fn state_screen(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        // Read screen info from client's viewport (Level 2 lock)
        let screen_info = {
            let viewport = ctx.client.viewport().read().await;
            ScreenInfo {
                width: viewport.terminal_width,
                height: viewport.terminal_height,
                active_buffer_id: ProtocolBufferId::from(
                    viewport.active_buffer.map_or(0, KernelBufferId::as_usize),
                ),
                active_window_id: None,
                window_count: 1,
            }
        }; // Lock dropped

        Ok(serde_json::to_value(screen_info).expect("ScreenInfo serialization cannot fail"))
    })
}

/// Handler for `state/selection` method.
///
/// Returns the current selection state.
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "state/selection", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "result": {"active": false, "mode": "character", ...}}
/// ```
///
/// # Panics
///
/// This function will not panic as `SelectionInfo` serialization is infallible.
#[must_use]
pub fn state_selection(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        let selection_info = ctx
            .session
            .with_state(|state| {
                // Check if we have an active buffer with a selection (from driver_session SSOT)
                if let Some(buffer_id) = state.session_active_buffer()
                    && let Some(buffer_arc) = state.app.kernel.buffers.get(buffer_id)
                {
                    let buffer = buffer_arc.read();
                    let selection = buffer.selection();

                    // Check if selection is active
                    if selection.is_active() {
                        let anchor = selection.anchor;
                        let cursor_pos = buffer.position();

                        return SelectionInfo {
                            active: true,
                            mode: match selection.mode() {
                                reovim_kernel::api::v1::SelectionMode::Character => {
                                    SelectionMode::Character
                                }
                                reovim_kernel::api::v1::SelectionMode::Line => SelectionMode::Line,
                                reovim_kernel::api::v1::SelectionMode::Block => {
                                    SelectionMode::Block
                                }
                            },
                            anchor: Position::new(anchor.line, anchor.column),
                            cursor: Position::new(cursor_pos.line, cursor_pos.column),
                        };
                    }
                }

                // No selection active
                SelectionInfo {
                    active: false,
                    mode: SelectionMode::Character,
                    anchor: Position::new(0, 0),
                    cursor: Position::new(0, 0),
                }
            })
            .await;

        Ok(serde_json::to_value(selection_info).expect("SelectionInfo serialization cannot fail"))
    })
}

/// Handler for `state/layout` method.
///
/// Returns the current window layout state from the compositor.
///
/// If no compositor is attached, returns a single-window layout covering
/// the full screen (backwards compatible with single-window mode).
///
/// # Request
///
/// ```json
/// {"jsonrpc": "2.0", "id": 1, "method": "state/layout", "params": {}}
/// ```
///
/// # Response
///
/// ```json
/// {
///   "screen": {"x": 0, "y": 0, "width": 80, "height": 24},
///   "windows": [...],
///   "focused_window": 0,
///   "active_layer": 0,
///   "window_count": 1
/// }
/// ```
///
/// # Panics
///
/// This function will not panic as `WireLayoutInfo` serialization is infallible.
#[must_use]
#[allow(clippy::option_if_let_else)] // if/else is clearer than map_or_else here
pub fn state_layout(ctx: RpcContext, _params: serde_json::Value) -> HandlerFuture {
    Box::pin(async move {
        // Read client's viewport dimensions (Level 2 lock)
        let (width, height) = {
            let viewport = ctx.client.viewport().read().await;
            (viewport.terminal_width, viewport.terminal_height)
        }; // Lock dropped before session lock

        let screen = Rect::new(0, 0, width, height);

        // Query layout from session state
        let layout_info = ctx
            .session
            .with_state(|state| {
                // Get compositor from driver_session
                if let Some(compositor) = state.driver_session.compositor() {
                    // Get composite result with all window placements
                    let result = compositor.composite(screen);

                    // Convert to wire format
                    convert_composite_to_wire(&result)
                } else {
                    // No compositor - single window fallback
                    single_window_layout(width, height, state.session_active_buffer())
                }
            })
            .await;

        Ok(serde_json::to_value(layout_info).expect("WireLayoutInfo serialization cannot fail"))
    })
}

/// Convert compositor result to wire format.
fn convert_composite_to_wire(
    result: &reovim_driver_display::layout::CompositeResult,
) -> WireLayoutInfo {
    use reovim_driver_display::layout::Zone;

    let windows: Vec<WireWindowPlacement> = result
        .placements
        .iter()
        .map(|p| WireWindowPlacement {
            window_id: WireWindowId::from(p.window_id.as_usize()),
            layer_id: WireLayerId::from(p.layer_id.as_u16() as usize),
            zone: match p.zone {
                Zone::Tiled => WireZone::Tiled,
                Zone::Float => WireZone::Float,
                Zone::Overlay => WireZone::Overlay,
            },
            bounds: WireRect::new(p.bounds.x, p.bounds.y, p.bounds.width, p.bounds.height),
            z_order: p.z_order.as_u16(),
            visible: p.visible,
            focusable: p.focusable,
            buffer_id: None, // TODO: window-buffer mapping (#440)
        })
        .collect();

    WireLayoutInfo {
        screen: WireRect::new(
            result.screen.x,
            result.screen.y,
            result.screen.width,
            result.screen.height,
        ),
        windows,
        focused_window: result.focused.map(|id| WireWindowId::from(id.as_usize())),
        active_layer: result
            .active_layer
            .map(|id| WireLayerId::from(id.as_u16() as usize)),
        window_count: result.placements.len(),
    }
}

/// Create single-window layout when no compositor is attached.
fn single_window_layout(
    width: u16,
    height: u16,
    active_buffer: Option<KernelBufferId>,
) -> WireLayoutInfo {
    WireLayoutInfo::single_window(
        width,
        height,
        active_buffer.map(|id| ProtocolBufferId::from(id.as_usize())),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        server::rpc::{
            RpcContext,
            handlers::test_utils::{test_client, test_ctx, test_session},
        },
        session::ClientId,
    };

    #[tokio::test]
    async fn test_state_mode_handler() {
        let ctx = test_ctx();

        let result = state_mode(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("edit_mode").is_some());
    }

    #[tokio::test]
    async fn test_state_cursor_handler() {
        let ctx = test_ctx();

        let result = state_cursor(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("line").is_some());
        assert!(value.get("column").is_some());
    }

    #[tokio::test]
    async fn test_state_screen_returns_dimensions() {
        let ctx = test_ctx();

        let result = state_screen(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
        let value = result.unwrap();
        // Default viewport size is 80x24 (VT100 defaults)
        assert_eq!(value.get("width").and_then(serde_json::Value::as_u64), Some(80));
        assert_eq!(value.get("height").and_then(serde_json::Value::as_u64), Some(24));
        assert_eq!(
            value
                .get("active_buffer_id")
                .and_then(serde_json::Value::as_u64),
            Some(0)
        );
        assert_eq!(
            value
                .get("window_count")
                .and_then(serde_json::Value::as_u64),
            Some(1)
        );
    }

    #[tokio::test]
    async fn test_state_screen_no_active_buffer() {
        let ctx = test_ctx();

        let result = state_screen(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        // No active buffer in viewport should return buffer_id = 0
        assert_eq!(
            value
                .get("active_buffer_id")
                .and_then(serde_json::Value::as_u64),
            Some(0)
        );
    }

    #[tokio::test]
    async fn test_state_screen_after_resize() {
        let session = test_session();
        let client_id = ClientId::new(1);
        let client = test_client(session.id().clone(), client_id);

        // Resize the client's viewport
        {
            let mut viewport = client.viewport().write().await;
            viewport.terminal_width = 200;
            viewport.terminal_height = 50;
        }

        let ctx = RpcContext {
            session,
            client_id,
            client,
        };

        let result = state_screen(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        assert_eq!(value.get("width").and_then(serde_json::Value::as_u64), Some(200));
        assert_eq!(value.get("height").and_then(serde_json::Value::as_u64), Some(50));
    }

    // Layout handler tests (#444)

    #[tokio::test]
    async fn test_state_layout_single_window() {
        let ctx = test_ctx();

        let result = state_layout(ctx, serde_json::json!({})).await;

        assert!(result.is_ok());
        let value = result.unwrap();

        // Should have screen dimensions
        assert!(value.get("screen").is_some());

        // Should have windows array
        let windows = value.get("windows").and_then(|v| v.as_array());
        assert!(windows.is_some());

        // Single window layout (no compositor)
        assert_eq!(
            value
                .get("window_count")
                .and_then(serde_json::Value::as_u64),
            Some(1)
        );
    }

    #[tokio::test]
    async fn test_state_layout_screen_dimensions() {
        let session = test_session();
        let client_id = ClientId::new(1);
        let client = test_client(session.id().clone(), client_id);

        // Set viewport dimensions
        {
            let mut viewport = client.viewport().write().await;
            viewport.terminal_width = 120;
            viewport.terminal_height = 40;
        }

        let ctx = RpcContext {
            session,
            client_id,
            client,
        };

        let result = state_layout(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        let value = result.unwrap();
        let screen = value.get("screen").unwrap();

        // Check screen matches viewport
        assert_eq!(screen.get("width").and_then(serde_json::Value::as_u64), Some(120));
        assert_eq!(screen.get("height").and_then(serde_json::Value::as_u64), Some(40));
    }

    #[tokio::test]
    async fn test_state_layout_response_fields() {
        let ctx = test_ctx();

        let result = state_layout(ctx, serde_json::json!({})).await;
        assert!(result.is_ok());

        let value = result.unwrap();

        // Required fields
        assert!(value.get("screen").is_some());
        assert!(value.get("windows").is_some());
        assert!(value.get("window_count").is_some());

        // Optional fields (may or may not be present)
        // focused_window and active_layer can be null
    }
}
