//! Main event loop for the editor

use std::sync::Arc;

use crate::{
    buffer::{Buffer, SelectionOps, TextOps},
    event::{
        BufferEvent, CommandHandler, HighlightEvent, InnerEvent, InputEventBroker,
        TerminateHandler, TextInputEvent, WindowEvent,
    },
    modd::{ModeState, SubMode},
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
        let mut command_hdr = CommandHandler::new(self.tx.clone(), mode_rx, self.keymap.clone());
        let mut terminate_hdr = TerminateHandler::new(self.tx.clone());

        input_broker.key_broker.enlist(&mut command_hdr);
        input_broker.key_broker.enlist(&mut terminate_hdr);

        tokio::spawn(async move { command_hdr.run().await });
        tokio::spawn(async move { terminate_hdr.run().await });
        tokio::spawn(async move { input_broker.subscribe().await });

        // Spawn EventBus event processor
        if let Some(mut event_rx) = self.event_bus.take_receiver() {
            let event_bus = Arc::clone(&self.event_bus);
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    let sender = event_bus.sender();
                    let mut ctx = crate::event_bus::HandlerContext::new(&sender);
                    let _ = event_bus.dispatch(&event, &mut ctx);
                }
            });
        }

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
        let mut command_hdr =
            crate::event::CommandHandler::new(self.tx.clone(), mode_rx, self.keymap.clone());
        let mut terminate_hdr = crate::event::TerminateHandler::new(self.tx.clone());

        input_broker.key_broker.enlist(&mut command_hdr);
        input_broker.key_broker.enlist(&mut terminate_hdr);

        tokio::spawn(async move { command_hdr.run().await });
        tokio::spawn(async move { terminate_hdr.run().await });
        tokio::spawn(async move { input_broker.subscribe().await });

        // Spawn EventBus event processor
        if let Some(mut event_rx) = self.event_bus.take_receiver() {
            let event_bus = Arc::clone(&self.event_bus);
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    let sender = event_bus.sender();
                    let mut ctx = crate::event_bus::HandlerContext::new(&sender);
                    let _ = event_bus.dispatch(&event, &mut ctx);
                }
            });
        }

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
                let loop_start = std::time::Instant::now();
                let ev_name = format!("{:?}", std::mem::discriminant(&ev));
                if self.handle_event(ev) {
                    break;
                }
                // Drain all pending events before rendering
                // This coalesces renders across multiple related events
                // (e.g., PendingKeysEvent + CommandEvent + ModeChangeEvent from one key)
                let mut drained = 0;
                while let Ok(ev) = self.rx.try_recv() {
                    drained += 1;
                    if self.handle_event(ev) {
                        // Flush before breaking on quit
                        self.flush_render();
                        return;
                    }
                }
                // Flush render once after all pending events processed
                let pre_render = loop_start.elapsed();
                self.flush_render();
                tracing::debug!(
                    "[RTT] event_loop: first_ev={} drained={} pre_render={:?} total={:?}",
                    ev_name, drained, pre_render, loop_start.elapsed()
                );
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
            InnerEvent::SyntaxEvent(syntax_event) => {
                use crate::event::SyntaxEvent;
                match syntax_event {
                    SyntaxEvent::Attach { buffer_id, syntax } => {
                        if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
                            buffer.attach_syntax(syntax);
                            // Start saturator for background cache computation
                            if !buffer.has_saturator() {
                                buffer.start_saturator(self.tx.clone());
                            }
                            tracing::debug!(buffer_id, "Attached syntax provider and started saturator");
                            self.request_render();
                        }
                    }
                    SyntaxEvent::Detach { buffer_id } => {
                        if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
                            buffer.detach_syntax();
                            tracing::debug!(buffer_id, "Detached syntax provider");
                            self.request_render();
                        }
                    }
                    SyntaxEvent::Reparse { buffer_id } => {
                        if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
                            // Get content first before borrowing syntax mutably
                            let content: String = buffer
                                .contents
                                .iter()
                                .map(|l| l.inner.as_str())
                                .collect::<Vec<_>>()
                                .join("\n");
                            if let Some(syntax) = buffer.syntax_mut() {
                                syntax.parse(&content);
                                tracing::debug!(buffer_id, "Reparsed syntax");
                            }
                            self.request_render();
                        }
                    }
                }
            }
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
            InnerEvent::TextInputEvent(focus_event) => {
                self.handle_interactor_input(focus_event);
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
            // File open request (from explorer, etc.)
            InnerEvent::OpenFileRequest { path } => {
                tracing::info!("Runtime: Opening file from request: {:?}", path);
                // Convert PathBuf to &str for open_file
                if let Some(path_str) = path.to_str() {
                    self.open_file(path_str);
                    self.screen.set_editor_buffer(self.active_buffer_id);
                    self.request_render();
                } else {
                    tracing::error!("Runtime: Invalid UTF-8 in file path: {:?}", path);
                }
            }
            // Set register content (from plugins)
            InnerEvent::SetRegister { register, text } => {
                tracing::debug!(
                    "Runtime: Setting register {:?} with text length {}",
                    register,
                    text.len()
                );
                self.registers.set_by_name(register, text);
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
    pub(crate) fn handle_mode_change(&mut self, new_mode: ModeState) {
        tracing::info!(
            "Mode changed: edit_mode={:?}, interactor={}",
            new_mode.edit_mode,
            new_mode.interactor_id.0
        );

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

        // Handle normal mode
        if new_mode.is_normal() && matches!(new_mode.sub_mode, SubMode::None) {
            // Clear selection when returning to normal mode
            if let Some(buffer) = self.buffers.get_mut(&self.active_buffer_id) {
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
    fn handle_interactor_input(&mut self, event: TextInputEvent) {
        use crate::{
            event_bus::{
                DynEvent,
                core_events::{PluginBackspace, PluginTextInput},
            },
            modd::ComponentId,
        };

        let interactor_id = self.mode_state.interactor_id;

        // Fast path: Built-in components with direct Runtime access
        if true {
            match interactor_id {
                ComponentId::EDITOR => {
                    match event {
                        TextInputEvent::InsertChar(c) => {
                            crate::runtime::handle_editor_input(self, Some(c), false, false);
                        }
                        TextInputEvent::DeleteCharBackward => {
                            crate::runtime::handle_editor_input(self, None, true, false);
                        }
                    }
                    self.request_render();
                    return;
                }
                ComponentId::COMMAND_LINE => {
                    match event {
                        TextInputEvent::InsertChar(c) => {
                            crate::runtime::handle_command_line_input(self, Some(c), false, false);
                        }
                        TextInputEvent::DeleteCharBackward => {
                            crate::runtime::handle_command_line_input(self, None, true, false);
                        }
                    }
                    self.request_render();
                    return;
                }
                _ => {} // Fall through to plugin path
            }
        }

        // Plugin path: Emit events via EventBus for plugins to handle

        let dyn_event = match event {
            TextInputEvent::InsertChar(c) => DynEvent::new(PluginTextInput {
                target: interactor_id,
                c,
            }),
            TextInputEvent::DeleteCharBackward => DynEvent::new(PluginBackspace {
                target: interactor_id,
            }),
        };

        // Dispatch via event bus
        let sender = self.event_bus.sender();
        let mut ctx = crate::event_bus::HandlerContext::new(&sender);
        let _ = self.event_bus.dispatch(&dyn_event, &mut ctx);
        if ctx.render_requested() {
            self.request_render();
        }
    }
}
