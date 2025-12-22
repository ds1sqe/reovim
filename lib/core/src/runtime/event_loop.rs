//! Main event loop for the editor

use crate::{
    buffer::{Buffer, SelectionOps, TextOps},
    event::{
        BufferEvent, CommandHandler, FocusInputEvent, HighlightEvent, InnerEvent, InputEventBroker,
        TerminateHandler, WindowEvent,
    },
    interactor::InputResult,
    modd::{EditMode, ModeState, SubMode, VisualVariant},
};

use super::Runtime;

impl Runtime {
    /// Initialize and run the editor event loop
    #[allow(clippy::missing_panics_doc)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::future_not_send)]
    #[allow(clippy::single_match_else)]
    #[allow(clippy::collapsible_if)]
    #[allow(clippy::match_same_arms)]
    pub async fn init(mut self) {
        tracing::info!("Runtime initializing");

        // Load file if provided, otherwise show landing page
        if let Some(path) = self.initial_file.clone() {
            // Use create_buffer_from_file which handles treesitter parsing and decorations
            if let Some(buffer_id) = self.create_buffer_from_file(&path) {
                // Update active buffer to the newly created one
                self.active_buffer_id = buffer_id;
            } else {
                // File failed to load, create empty buffer
                let mut buffer = Buffer::empty(0);
                buffer.file_path = Some(path);
                self.buffers.insert(0, buffer);
            }
        } else {
            // Show landing page when no file is opened
            let mut buffer = Buffer::empty(0);
            let landing_content = crate::landing::generate(
                self.screen.width(),
                self.screen.height().saturating_sub(1), // Reserve status line
            );
            buffer.set_content(&landing_content);
            self.showing_landing_page = true;
            self.buffers.insert(0, buffer);
        }
        let input_broker = InputEventBroker::with_event_sender(self.tx.clone());

        // Command handler for key-to-command translation
        // Pass mode receiver so CommandHandler can read mode from Runtime (single source of truth)
        let mode_rx = self.subscribe_mode();
        let mut command_hdr = CommandHandler::new(self.tx.clone(), mode_rx);
        let mut terminate_hdr = TerminateHandler::new(self.tx.clone());

        input_broker.key_broker.enlist(&mut command_hdr);
        input_broker.key_broker.enlist(&mut terminate_hdr);

        tokio::spawn(async move { command_hdr.run().await });
        tokio::spawn(async move { terminate_hdr.run().await });
        tokio::spawn(async move { input_broker.subscribe().await });

        // Initial render to show content immediately (use render directly, not coalesced)
        self.render();

        tracing::debug!("Entering event loop");
        self.run_event_loop().await;

        tracing::debug!("Event loop ended, finalizing screen");
        let _ = self.screen.finalize();
    }

    /// Initialize and run the editor with a custom key source.
    ///
    /// Used for server mode where keys are injected via `ChannelKeySource`
    /// instead of reading from the terminal.
    #[allow(clippy::missing_panics_doc)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::future_not_send)]
    pub async fn init_with_key_source<K: crate::io::input::KeySource + 'static>(
        mut self,
        key_source: K,
    ) {
        tracing::info!("Runtime initializing (server mode)");

        // Load file if provided, otherwise show landing page
        if let Some(path) = self.initial_file.clone() {
            // Use create_buffer_from_file which handles treesitter parsing and decorations
            if let Some(buffer_id) = self.create_buffer_from_file(&path) {
                // Update active buffer to the newly created one
                self.active_buffer_id = buffer_id;
            } else {
                // File failed to load, create empty buffer with path
                let mut buffer = Buffer::empty(0);
                buffer.file_path = Some(path);
                self.buffers.insert(0, buffer);
            }
        } else {
            // Show landing page when no file is opened (unified with regular mode)
            let mut buffer = Buffer::empty(0);
            let landing_content = crate::landing::generate(
                self.screen.width(),
                self.screen.height().saturating_sub(1), // Reserve status line
            );
            buffer.set_content(&landing_content);
            self.showing_landing_page = true;
            self.buffers.insert(0, buffer);
        }

        // Use custom key source for server mode
        let input_broker = crate::event::InputEventBroker::with_key_source(key_source);

        // Command handler for key-to-command translation
        let mode_rx = self.subscribe_mode();
        let mut command_hdr = crate::event::CommandHandler::new(self.tx.clone(), mode_rx);
        let mut terminate_hdr = crate::event::TerminateHandler::new(self.tx.clone());

        input_broker.key_broker.enlist(&mut command_hdr);
        input_broker.key_broker.enlist(&mut terminate_hdr);

        tokio::spawn(async move { command_hdr.run().await });
        tokio::spawn(async move { terminate_hdr.run().await });
        tokio::spawn(async move { input_broker.subscribe().await });

        // Initial render (use render directly, not coalesced)
        self.render();

        tracing::debug!("Entering event loop (server mode)");
        self.run_event_loop().await;

        tracing::debug!("Event loop ended (server mode)");
        let _ = self.screen.finalize();
    }

    /// The main event processing loop
    #[allow(clippy::collapsible_if)]
    #[allow(clippy::match_same_arms)]
    #[allow(clippy::future_not_send)]
    async fn run_event_loop(&mut self) {
        loop {
            // Check for incoming events
            if let Some(ev) = self.rx.recv().await {
                if self.handle_event(ev) {
                    break;
                }
                // Drain all pending events before rendering
                // This coalesces renders across multiple related events
                // (e.g., PendingKeysEvent + CommandEvent + ModeChangeEvent from one key)
                while let Ok(ev) = self.rx.try_recv() {
                    if self.handle_event(ev) {
                        // Flush before breaking on quit
                        self.flush_render();
                        return;
                    }
                }
                // Flush render once after all pending events processed
                self.flush_render();
            } else {
                self.tx
                    .send(InnerEvent::KillSignal)
                    .await
                    .expect("cannot broadcast kill signal");
                break;
            }
        }
    }

    /// Handle a single event. Returns true if the editor should quit.
    #[allow(clippy::collapsible_if)]
    #[allow(clippy::match_same_arms)]
    #[allow(clippy::too_many_lines)]
    pub(crate) fn handle_event(&mut self, ev: InnerEvent) -> bool {
        match ev {
            InnerEvent::BufferEvent(buffer_event) => match buffer_event {
                BufferEvent::SetContent { buffer_id, content } => {
                    if let Some(b) = self.buffers.get_mut(&buffer_id) {
                        b.set_content(&content);
                        self.request_render();
                    }
                }
                BufferEvent::LoadFile { buffer_id, path } => {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Some(b) = self.buffers.get_mut(&buffer_id) {
                            b.set_content(&content);
                            b.file_path = Some(path.to_string_lossy().to_string());
                            self.request_render();
                        }
                    }
                }
                BufferEvent::Create { buffer_id } => {
                    let buffer = Buffer::empty(buffer_id);
                    self.buffers.insert(buffer_id, buffer);
                }
                BufferEvent::Close { buffer_id } => {
                    self.close_buffer(buffer_id);
                    self.request_render();
                }
                BufferEvent::Switch { buffer_id } => {
                    self.switch_buffer(buffer_id);
                    self.screen.set_editor_buffer(buffer_id);
                    self.request_render();
                }
            },
            InnerEvent::CommandEvent(cmd_event) => {
                if self.handle_command(cmd_event) {
                    return true;
                }
            }
            InnerEvent::ModeChangeEvent(new_mode) => {
                self.handle_mode_change(new_mode);
            }
            InnerEvent::PendingKeysEvent(keys) => {
                // If pending_keys is being cleared and had content, save as last_command
                if keys.is_empty() && !self.pending_keys.is_empty() {
                    self.last_command = self.pending_keys.clone();
                }
                self.pending_keys = keys;
                self.request_render();
            }
            InnerEvent::WindowEvent(window_event) => match window_event {
                WindowEvent::FocusPlugin { id } => {
                    self.screen.focus_plugin(id);
                    self.request_render();
                }
                WindowEvent::FocusEditor => {
                    self.screen.focus_editor();
                    self.request_render();
                }
                // TODO: Implement in Phase 7
                WindowEvent::SplitHorizontal { .. }
                | WindowEvent::SplitVertical { .. }
                | WindowEvent::Close { .. }
                | WindowEvent::CloseOthers
                | WindowEvent::FocusDirection { .. }
                | WindowEvent::MoveWindow { .. }
                | WindowEvent::Resize { .. }
                | WindowEvent::Equalize
                | WindowEvent::TabNew { .. }
                | WindowEvent::TabClose
                | WindowEvent::TabNext
                | WindowEvent::TabPrev
                | WindowEvent::TabGoto { .. } => {
                    // Window management events - to be implemented
                }
            },
            InnerEvent::HighlightEvent(hl_event) => match hl_event {
                HighlightEvent::Add {
                    buffer_id,
                    highlights,
                } => {
                    self.highlight_store.add(buffer_id, highlights);
                    self.request_render();
                }
                HighlightEvent::ClearGroup { buffer_id, group } => {
                    self.highlight_store.clear_group(buffer_id, group);
                    self.request_render();
                }
                HighlightEvent::ClearAll { buffer_id } => {
                    self.highlight_store.clear_all(buffer_id);
                    self.request_render();
                }
            },
            InnerEvent::RenderSignal => {
                self.request_render();
            }
            InnerEvent::KillSignal => {
                return true;
            }
            InnerEvent::OperatorMotionEvent(ref action) => {
                self.handle_operator_motion(action);
            }
            InnerEvent::VisualTextObjectEvent(ref action) => {
                self.handle_visual_text_object(action);
            }
            InnerEvent::ScreenResizeEvent { width, height } => {
                tracing::debug!("Screen resize: {}x{}", width, height);
                self.screen.resize(width, height);
                self.request_render();
            }
            InnerEvent::RpcRequest {
                id,
                method,
                params,
                response_tx,
            } => {
                let response = self.handle_rpc_request(id, &method, &params);
                let _ = response_tx.send(response);
            }
            InnerEvent::FocusInputEvent(focus_event) => {
                self.handle_interactor_input(focus_event);
            }
            // Generic focus input - dispatched to registered handler (enlist pattern)
            InnerEvent::FocusInput {
                char,
                delete,
                clear_landing,
            } => {
                let interactor_id = self.mode_state.interactor_id;
                // Generic dispatch via registered handler - NO match on InteractorId!
                if let Some(&handler) = self.focus_input_handlers.get(&interactor_id) {
                    handler(self, char, delete, clear_landing);
                }
                self.request_render();
            }
            // Plugin-defined events - dispatched via event bus
            InnerEvent::PluginEvent { plugin_id, event } => {
                tracing::debug!(
                    "Dispatching plugin event from {}: {:?}",
                    plugin_id,
                    event.type_name()
                );
                // Dispatch to event bus - subscribers will receive the event
                let sender = self.event_bus.sender();
                let mut ctx = crate::event_bus::HandlerContext::new(&sender);
                let _ = self.event_bus.dispatch(&event, &mut ctx);
                if ctx.render_requested() {
                    self.request_render();
                }
            }
        }
        false
    }

    /// Handle an RPC request from server mode
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::missing_panics_doc)]
    fn handle_rpc_request(
        &mut self,
        id: u64,
        method: &str,
        params: &serde_json::Value,
    ) -> crate::rpc::RpcResponse {
        use crate::rpc::{RpcError, RpcResponse, methods};

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
            self.active_buffer_id,
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
                let buffer_id = get_buffer_id(params, self.active_buffer_id);
                self.cursor_snapshot(buffer_id).map_or_else(
                    || RpcResponse::error(id, RpcError::buffer_not_found(buffer_id)),
                    |snapshot| RpcResponse::success(id, serde_json::to_value(snapshot).unwrap()),
                )
            }
            methods::STATE_SELECTION => {
                let buffer_id = get_buffer_id(params, self.active_buffer_id);
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
                    .get(&self.active_buffer_id)
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
                // Return layer visibility information - dynamically query overlay registry
                let ctx = crate::component::RenderContext::new(
                    self.screen.width(),
                    self.screen.height(),
                    &self.theme,
                    self.color_mode,
                );
                let layers = self.screen.layer_info(&self.overlay_registry, &ctx);
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
                let buffer_id = get_buffer_id(params, self.active_buffer_id);
                self.buffer_content(buffer_id).map_or_else(
                    || RpcResponse::error(id, RpcError::buffer_not_found(buffer_id)),
                    |content| RpcResponse::success(id, serde_json::json!({ "content": content })),
                )
            }
            methods::BUFFER_SET_CONTENT => {
                let buffer_id = get_buffer_id(params, self.active_buffer_id);
                let content = params.get("content").and_then(serde_json::Value::as_str);

                match (self.buffers.get_mut(&buffer_id), content) {
                    (Some(buffer), Some(content)) => {
                        buffer.set_content(content);
                        // Clear landing page flag when buffer content is set via RPC
                        self.showing_landing_page = false;
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
                            serde_json::json!({ "buffer_id": self.active_buffer_id }),
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

    /// Handle mode change events
    #[allow(clippy::collapsible_if)]
    fn handle_mode_change(&mut self, new_mode: ModeState) {
        tracing::debug!(?new_mode, "Mode changed");

        // Handle insert mode
        if new_mode.is_insert() {
            // Clear landing page content when entering insert mode (only once)
            if self.showing_landing_page {
                if let Some(buffer) = self.buffers.get_mut(&0) {
                    buffer.contents.clear();
                    buffer.cur.x = 0;
                    buffer.cur.y = 0;
                }
                self.showing_landing_page = false;
            }
        }

        // Handle visual mode
        if let EditMode::Visual(variant) = &new_mode.edit_mode {
            // Start selection when entering visual mode
            if let Some(buffer) = self.buffers.get_mut(&0) {
                match variant {
                    VisualVariant::Block => buffer.start_block_selection(),
                    VisualVariant::Char | VisualVariant::Line => buffer.start_selection(),
                }
            }
        }

        // Handle normal mode
        if new_mode.is_normal() && matches!(new_mode.sub_mode, SubMode::None) {
            // Clear selection when returning to normal mode
            if let Some(buffer) = self.buffers.get_mut(&0) {
                buffer.clear_selection();
            }
            // Note: command line is cleared in handle_command_line_command
            // after the command is executed, not here (to avoid race condition)
        }

        // Handle command mode
        if new_mode.is_command() {
            // Activate command line when entering command mode
            self.command_line.activate();
        }

        // Explorer, ExplorerInput, OperatorPending, Telescope modes are handled elsewhere:
        // - Explorer mode is handled via window focus
        // - ExplorerInput mode is for file operations and filter
        // - OperatorPending mode just waits for a motion key (handled by CommandHandler)
        // - Telescope mode is handled separately when opening a picker

        // Use set_mode to broadcast via watch channel
        self.set_mode(new_mode);
        self.request_render();
    }

    /// Handle interactor input events by routing to the active interactor
    fn handle_interactor_input(&mut self, event: FocusInputEvent) {
        let interactor_id = self.mode_state.interactor_id;

        // Get the result from the interactor
        let result = if let Some(interactor) = self.interactor_registry.get_mut(interactor_id) {
            match event {
                FocusInputEvent::InsertChar(c) => {
                    interactor.handle_insert_char(c, &self.mode_state)
                }
                FocusInputEvent::DeleteCharBackward => {
                    interactor.handle_delete_backward(&self.mode_state)
                }
            }
        } else {
            InputResult::NotHandled
        };

        // Dispatch based on result (enlist pattern)
        match result {
            InputResult::NotHandled => {}
            InputResult::Handled => {
                self.request_render();
            }
            InputResult::SendEvent(event) => {
                // Send event to be handled by registered handler
                let _ = self.tx.try_send(event);
            }
        }
    }
}
