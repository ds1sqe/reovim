//! RPC request handling for server mode
//!
//! This module handles JSON-RPC method dispatch for the editor's server mode,
//! providing handlers for state queries, buffer operations, and editor commands.

use crate::{
    buffer::TextOps,
    rpc::{RpcError, RpcResponse, methods},
};

use super::Runtime;

impl Runtime {
    /// Handle an RPC request from server mode
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::missing_panics_doc)]
    pub(crate) fn handle_rpc_request(
        &mut self,
        id: u64,
        method: &str,
        params: &serde_json::Value,
    ) -> RpcResponse {
        // Helper to extract buffer_id from params
        #[allow(clippy::cast_possible_truncation)]
        let get_buffer_id = |params: &serde_json::Value, default: usize| -> usize {
            params
                .get("buffer_id")
                .and_then(serde_json::Value::as_u64)
                .map_or(default, |v| v as usize)
        };

        // First, try plugin-registered RPC handlers
        let rpc_ctx = crate::rpc::RpcHandlerContext::new(
            &self.plugin_state,
            &self.mode_state,
            self.active_buffer_id(),
        );
        if let Some(result) = self.rpc_handler_registry.dispatch(method, params, &rpc_ctx) {
            return match result {
                crate::rpc::RpcResult::Success(value) => RpcResponse::success(id, value),
                crate::rpc::RpcResult::Error { code, message } => {
                    RpcResponse::error(id, RpcError::new(code, message))
                }
            };
        }

        // Fall back to core RPC handlers
        match method {
            methods::STATE_MODE => {
                let snapshot = self.mode_snapshot();
                RpcResponse::success(id, serde_json::to_value(snapshot).unwrap())
            }
            methods::STATE_CURSOR => {
                let buffer_id = get_buffer_id(params, self.active_buffer_id());
                self.cursor_snapshot(buffer_id).map_or_else(
                    || RpcResponse::error(id, RpcError::buffer_not_found(buffer_id)),
                    |snapshot| RpcResponse::success(id, serde_json::to_value(snapshot).unwrap()),
                )
            }
            methods::STATE_SELECTION => {
                let buffer_id = get_buffer_id(params, self.active_buffer_id());
                self.selection_snapshot(buffer_id).map_or_else(
                    || RpcResponse::error(id, RpcError::buffer_not_found(buffer_id)),
                    |snapshot| RpcResponse::success(id, serde_json::to_value(snapshot).unwrap()),
                )
            }
            methods::STATE_SCREEN => {
                let snapshot = self.screen_snapshot();
                RpcResponse::success(id, serde_json::to_value(snapshot).unwrap())
            }
            methods::STATE_SCREEN_CONTENT => {
                // Note: Full screen content capture is handled at the server level
                // via FrameBufferHandle. Return dimensions for now.
                let snapshot = crate::rpc::ScreenContentSnapshot {
                    width: self.screen.width(),
                    height: self.screen.height(),
                    format: crate::rpc::ScreenFormat::PlainText,
                    content: String::new(), // Populated by server layer
                };
                RpcResponse::success(id, serde_json::to_value(snapshot).unwrap())
            }
            methods::STATE_TELESCOPE => {
                // Telescope is now a plugin - state query is handled by the plugin
                // Return inactive state as the base response
                let response = serde_json::json!({
                    "active": false,
                    "query": "",
                    "selected_index": 0,
                    "item_count": 0,
                    "picker_name": "",
                    "title": "",
                    "selected_item": null,
                });
                RpcResponse::success(id, response)
            }
            methods::STATE_MICROSCOPE => {
                // Microscope is a plugin - state query is handled by the plugin
                // Return inactive state as the base response
                let response = serde_json::json!({
                    "active": false,
                    "query": "",
                    "selected_index": 0,
                    "item_count": 0,
                    "picker_name": "",
                    "title": "",
                    "selected_item": null,
                    "prompt_mode": "Insert",
                });
                RpcResponse::success(id, response)
            }
            methods::STATE_WINDOWS => {
                let snapshot = self.windows_snapshot();
                RpcResponse::success(id, serde_json::to_value(snapshot).unwrap())
            }
            methods::STATE_VISUAL_SNAPSHOT => {
                // Return visual snapshot with cell grid and layer info
                self.visual_snapshot().map_or_else(
                    || {
                        RpcResponse::error(
                            id,
                            RpcError::internal_error("frame renderer not enabled"),
                        )
                    },
                    |snapshot| RpcResponse::success(id, serde_json::to_value(snapshot).unwrap()),
                )
            }
            methods::STATE_ASCII_ART => {
                // Return ASCII art representation of the screen
                let annotated = params
                    .get("annotated")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);

                let cursor_pos = self
                    .buffers
                    .get(&self.active_buffer_id())
                    .map(|b| (b.cur.x, b.cur.y));

                let content = if annotated {
                    self.screen.to_annotated_ascii(cursor_pos)
                } else {
                    self.screen.to_ascii()
                };

                content.map_or_else(
                    || {
                        RpcResponse::error(
                            id,
                            RpcError::internal_error("frame renderer not enabled"),
                        )
                    },
                    |content| RpcResponse::success(id, serde_json::json!({ "content": content })),
                )
            }
            methods::STATE_LAYER_INFO => {
                // Return layer visibility information
                // Plugin windows provide their own z-order during rendering
                let layers = vec![
                    crate::visual::LayerInfo {
                        name: "base".to_string(),
                        z_order: 0,
                        visible: true,
                        bounds: crate::visual::BoundsInfo::new(
                            0,
                            0,
                            self.screen.width(),
                            self.screen.height(),
                        ),
                    },
                    crate::visual::LayerInfo {
                        name: "editor".to_string(),
                        z_order: 2,
                        visible: true,
                        bounds: crate::visual::BoundsInfo::new(
                            0,
                            1,
                            self.screen.width(),
                            self.screen.height().saturating_sub(2),
                        ),
                    },
                ];
                RpcResponse::success(id, serde_json::to_value(layers).unwrap())
            }
            methods::INPUT_KEYS => {
                // Key injection is handled at the server level via ChannelKeySource
                // This handler is a fallback that returns an error
                RpcResponse::error(
                    id,
                    RpcError::internal_error("input/keys must be handled at server level"),
                )
            }
            methods::COMMAND_EXECUTE => {
                // Direct command execution by command ID
                let command_name = params.get("command").and_then(serde_json::Value::as_str);
                #[allow(clippy::cast_possible_truncation)]
                let count = params
                    .get("count")
                    .and_then(serde_json::Value::as_u64)
                    .map(|v| v as usize);

                command_name.map_or_else(
                    || RpcResponse::error(id, RpcError::invalid_params("missing 'command' field")),
                    |name| {
                        // TODO: Full implementation needs command lookup
                        RpcResponse::success(
                            id,
                            serde_json::json!({
                                "executed": false,
                                "message": format!("Command execution for '{}' (count: {:?}) - not yet implemented", name, count)
                            }),
                        )
                    },
                )
            }
            methods::BUFFER_LIST => {
                let snapshots = self.buffer_list_snapshot();
                RpcResponse::success(id, serde_json::to_value(snapshots).unwrap())
            }
            methods::BUFFER_GET_CONTENT => {
                let buffer_id = get_buffer_id(params, self.active_buffer_id());
                self.buffer_content(buffer_id).map_or_else(
                    || RpcResponse::error(id, RpcError::buffer_not_found(buffer_id)),
                    |content| RpcResponse::success(id, serde_json::json!({ "content": content })),
                )
            }
            methods::BUFFER_SET_CONTENT => {
                let buffer_id = get_buffer_id(params, self.active_buffer_id());
                let content = params.get("content").and_then(serde_json::Value::as_str);

                match (self.buffers.get_mut(&buffer_id), content) {
                    (Some(buffer), Some(content)) => {
                        buffer.set_content(content);
                        // Clear landing page flag when buffer content is set via RPC
                        self.showing_landing_page = false;
                        self.landing_state = None;
                        self.request_render();
                        RpcResponse::ok(id)
                    }
                    (None, _) => RpcResponse::error(id, RpcError::buffer_not_found(buffer_id)),
                    (_, None) => {
                        RpcResponse::error(id, RpcError::invalid_params("missing 'content' field"))
                    }
                }
            }
            methods::BUFFER_OPEN_FILE => {
                let path = params.get("path").and_then(serde_json::Value::as_str);
                path.map_or_else(
                    || RpcResponse::error(id, RpcError::invalid_params("missing 'path' field")),
                    |path| {
                        self.open_file(path);
                        self.request_render();
                        RpcResponse::success(
                            id,
                            serde_json::json!({ "buffer_id": self.active_buffer_id() }),
                        )
                    },
                )
            }
            methods::EDITOR_RESIZE => {
                let width = params.get("width").and_then(serde_json::Value::as_u64);
                let height = params.get("height").and_then(serde_json::Value::as_u64);

                match (width, height) {
                    (Some(w), Some(h)) => {
                        #[allow(clippy::cast_possible_truncation)]
                        {
                            self.screen.resize(w as u16, h as u16);
                            self.request_render();
                        }
                        RpcResponse::ok(id)
                    }
                    _ => RpcResponse::error(
                        id,
                        RpcError::invalid_params("missing 'width' or 'height' field"),
                    ),
                }
            }
            methods::EDITOR_QUIT => {
                // Note: This doesn't actually quit - the caller needs to check response
                // and send KillSignal separately if needed
                RpcResponse::ok(id)
            }
            _ => RpcResponse::error(id, RpcError::method_not_found(method)),
        }
    }
}
