//! Main event loop for the editor

use std::sync::Arc;

use std::time::Duration;

use crate::{
    animation::{AnimatedStyle, Effect, EffectId, EffectTarget},
    buffer::{Buffer, SelectionOps, TextOps},
    event::{
        BufferEvent, CommandHandler, HighlightEvent, InnerEvent, InputEventBroker,
        TerminateHandler, TextInputEvent, WindowEvent,
    },
    modd::{EditMode, ModeState, SubMode},
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

        // STEP 1: Set up input handlers FIRST (before file loading)
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

        // STEP 2: Spawn EventBus event processor
        // This starts processing queued events (like RegisterLanguage from subscribe phase)
        if let Some(mut event_rx) = self.event_bus.take_receiver() {
            let event_bus = Arc::clone(&self.event_bus);
            let inner_tx = self.tx.clone();
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    let sender = event_bus.sender();
                    let mut ctx = crate::event_bus::HandlerContext::new(&sender);
                    let _ = event_bus.dispatch(&event, &mut ctx);
                    // If any handler requested a render, send RenderSignal to main loop
                    if ctx.render_requested() {
                        let _ = inner_tx.try_send(InnerEvent::RenderSignal);
                    }
                }
            });
        }

        // STEP 3: Let queued events process (RegisterLanguage events from subscribe phase)
        tokio::task::yield_now().await;

        // STEP 4: Boot phase - plugins can do post-EventBus initialization
        // Languages are now registered, syntax providers can be created
        // Make inner_event_tx available to plugins for background tasks (e.g., completion saturator)
        self.plugin_state.set_inner_event_tx(self.tx.clone());
        tracing::debug!("Boot phase starting with {} plugins", self.plugins.len());
        for plugin in &self.plugins {
            let plugin_id = plugin.id();
            tracing::debug!(plugin = %plugin_id, "Booting plugin");
            plugin.boot(&self.event_bus, Arc::clone(&self.plugin_state), Some(self.tx.clone()));
        }
        tracing::debug!("Boot phase complete");

        // STEP 5: Load file AFTER languages are registered
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
            let landing_state = crate::landing::LandingState::new(
                self.screen.width(),
                self.screen.height().saturating_sub(1), // Reserve status line
            );
            let landing_content =
                landing_state.generate(self.screen.width(), self.screen.height().saturating_sub(1));
            buffer.set_content(&landing_content);
            self.landing_state = Some(landing_state);
            self.showing_landing_page = true;
            self.buffers.insert(0, buffer);
        }

        // STEP 6: Initial render to show content immediately
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

        // STEP 1: Set up input handlers FIRST (before file loading)
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

        // STEP 2: Spawn EventBus event processor
        // This starts processing queued events (like RegisterLanguage from subscribe phase)
        if let Some(mut event_rx) = self.event_bus.take_receiver() {
            let event_bus = Arc::clone(&self.event_bus);
            let inner_tx = self.tx.clone();
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    let sender = event_bus.sender();
                    let mut ctx = crate::event_bus::HandlerContext::new(&sender);
                    let _ = event_bus.dispatch(&event, &mut ctx);
                    // If any handler requested a render, send RenderSignal to main loop
                    if ctx.render_requested() {
                        let _ = inner_tx.try_send(InnerEvent::RenderSignal);
                    }
                }
            });
        }

        // STEP 3: Let queued events process (RegisterLanguage events from subscribe phase)
        tokio::task::yield_now().await;

        // STEP 4: Boot phase - plugins can do post-EventBus initialization
        // Languages are now registered, syntax providers can be created
        // Make inner_event_tx available to plugins for background tasks (e.g., completion saturator)
        self.plugin_state.set_inner_event_tx(self.tx.clone());
        tracing::debug!("Boot phase starting with {} plugins", self.plugins.len());
        for plugin in &self.plugins {
            let plugin_id = plugin.id();
            tracing::debug!(plugin = %plugin_id, "Booting plugin");
            plugin.boot(&self.event_bus, Arc::clone(&self.plugin_state), Some(self.tx.clone()));
        }
        tracing::debug!("Boot phase complete");

        // STEP 5: Load file AFTER languages are registered
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
            let landing_state = crate::landing::LandingState::new(
                self.screen.width(),
                self.screen.height().saturating_sub(1), // Reserve status line
            );
            let landing_content =
                landing_state.generate(self.screen.width(), self.screen.height().saturating_sub(1));
            buffer.set_content(&landing_content);
            self.landing_state = Some(landing_state);
            self.showing_landing_page = true;
            self.buffers.insert(0, buffer);
        }

        // STEP 6: Initial render
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
        use std::time::Duration;

        // Idle timeout: start shimmer after 3 seconds of inactivity
        const IDLE_TIMEOUT: Duration = Duration::from_secs(3);
        // Check interval for idle detection
        const IDLE_CHECK_INTERVAL: Duration = Duration::from_millis(500);
        // Landing page animation interval (250ms = large lion roar frame duration)
        const LANDING_ANIM_INTERVAL: Duration = Duration::from_millis(250);

        let mut idle_check_interval = tokio::time::interval(IDLE_CHECK_INTERVAL);
        idle_check_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut landing_anim_interval = tokio::time::interval(LANDING_ANIM_INTERVAL);
        landing_anim_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Check for incoming events
                ev = self.rx.recv() => {
                    if let Some(ev) = ev {
                        let loop_start = std::time::Instant::now();
                        let ev_name = format!("{:?}", std::mem::discriminant(&ev));

                        // Track user input for idle detection
                        self.track_input_for_idle(&ev);

                        if self.handle_event(ev) {
                            break;
                        }
                        // Drain all pending events before rendering
                        // This coalesces renders across multiple related events
                        // (e.g., PendingKeysEvent + CommandEvent + ModeChangeEvent from one key)
                        let mut drained = 0;
                        while let Ok(ev) = self.rx.try_recv() {
                            drained += 1;
                            self.track_input_for_idle(&ev);
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
                            ev_name,
                            drained,
                            pre_render,
                            loop_start.elapsed()
                        );
                    } else {
                        self.tx
                            .send(InnerEvent::KillSignal)
                            .await
                            .expect("cannot broadcast kill signal");
                        break;
                    }
                }
                // Periodic idle check
                _ = idle_check_interval.tick() => {
                    let elapsed = self.last_input_at.elapsed();
                    if elapsed >= IDLE_TIMEOUT && !self.idle_shimmer_active {
                        // Start idle shimmer effect
                        self.start_idle_shimmer();
                    }
                }
                // Landing page animation tick (only when showing landing page)
                _ = landing_anim_interval.tick(), if self.showing_landing_page => {
                    if let Some(ref mut state) = self.landing_state {
                        // Tick returns true if frame changed (250ms matches the interval constant)
                        if state.tick(250.0) {
                            // Update buffer with new frame
                            let content = state.generate(
                                self.screen.width(),
                                self.screen.height().saturating_sub(1),
                            );
                            if let Some(buffer) = self.buffers.get_mut(&self.active_buffer_id) {
                                buffer.set_content(&content);
                            }
                            self.request_render();
                            self.flush_render();
                        }
                    }
                }
            }
        }
    }

    /// Track user input events for idle detection
    fn track_input_for_idle(&mut self, ev: &InnerEvent) {
        // Only track actual user input events
        let is_user_input = matches!(
            ev,
            InnerEvent::CommandEvent(_)
                | InnerEvent::PendingKeysEvent(_)
                | InnerEvent::TextInputEvent(_)
        );

        if is_user_input {
            self.last_input_at = std::time::Instant::now();

            // Stop idle shimmer if active
            if self.idle_shimmer_active {
                self.stop_idle_shimmer();
            }
        }
    }

    /// Start the idle shimmer effect on the status line
    fn start_idle_shimmer(&mut self) {
        use crate::animation::{SweepConfig, UiElementId};

        let Some(handle) = self.plugin_state.animation_handle() else {
            return;
        };

        // Create a sweep effect: 2 second period, 20% width glow, 40% intensity
        let sweep = SweepConfig::new(2000, 0.2, 0.4);

        // Create shimmer effect with sweep
        let shimmer_effect = Effect::new(
            EffectId::new(0), // ID assigned by handle.start()
            EffectTarget::UiElement(UiElementId::StatusLine),
            AnimatedStyle::new(), // Base style, shimmer comes from sweep
        )
        .with_sweep(sweep)
        .with_priority(50); // Lower priority than mode flash

        if handle.start(shimmer_effect).is_some() {
            self.idle_shimmer_active = true;
            tracing::debug!("Started idle shimmer effect");
        }
    }

    /// Stop the idle shimmer effect
    fn stop_idle_shimmer(&mut self) {
        use crate::animation::UiElementId;

        if let Some(handle) = self.plugin_state.animation_handle() {
            handle.stop_target(EffectTarget::UiElement(UiElementId::StatusLine));
            self.idle_shimmer_active = false;
            tracing::debug!("Stopped idle shimmer effect");
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
                    self.last_command.clone_from(&self.pending_keys);
                }
                // Update plugin state so which-key and other plugins can access pending keys
                self.plugin_state.set_pending_keys(keys.clone());
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
                            tracing::debug!(
                                buffer_id,
                                "Attached syntax provider and started saturator"
                            );
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
                    "Runtime: Dispatching plugin event from {}: {:?}",
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
            // File open at position request (from LSP navigation)
            InnerEvent::OpenFileAtPositionRequest { path, line, column } => {
                tracing::info!("Runtime: Opening file at position: {:?}:{}:{}", path, line, column);
                if let Some(path_str) = path.to_str() {
                    self.open_file(path_str);
                    self.screen.set_editor_buffer(self.active_buffer_id);
                    // Set cursor position in the opened buffer
                    if let Some(buffer) = self.buffers.get_mut(&self.active_buffer_id) {
                        // Ensure line is within bounds
                        let max_line = buffer.contents.len().saturating_sub(1);
                        let target_line = line.min(max_line);
                        // Ensure column is within bounds for the target line
                        let line_len = buffer
                            .contents
                            .get(target_line)
                            .map_or(0, |l| l.inner.len());
                        let target_col = column.min(line_len.saturating_sub(1).max(0));
                        #[allow(clippy::cast_possible_truncation)]
                        {
                            buffer.cur.y = target_line as u16;
                            buffer.cur.x = target_col as u16;
                        }
                        tracing::debug!(
                            "Runtime: Cursor set to line={}, col={}",
                            target_line,
                            target_col
                        );
                    }
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
            // Settings/Option capability events
            InnerEvent::SetLineNumbers { enabled } => {
                tracing::info!("Runtime: Setting line numbers: {}", enabled);
                self.screen.set_number(enabled);
            }
            InnerEvent::SetRelativeLineNumbers { enabled } => {
                tracing::info!("Runtime: Setting relative line numbers: {}", enabled);
                self.screen.set_relative_number(enabled);
            }
            InnerEvent::SetTheme { name } => {
                tracing::info!("Runtime: Setting theme: {}", name);
                if let Some(theme_name) = crate::highlight::ThemeName::parse(&name) {
                    self.theme = crate::highlight::Theme::from_name(theme_name);
                    self.rehighlight_all_buffers();
                    // Request a render to apply the new theme immediately
                    self.request_render();
                } else {
                    tracing::warn!("Runtime: Unknown theme name: {}", name);
                }
            }
            InnerEvent::SetScrollbar { enabled } => {
                tracing::info!("Runtime: Setting scrollbar: {}", enabled);
                self.screen.set_scrollbar(enabled);
            }
            InnerEvent::SetIndentGuide { enabled } => {
                tracing::info!("Runtime: Setting indent guide: {}", enabled);
                self.indent_analyzer.set_enabled(enabled);
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

        // Trigger mode transition animation
        self.trigger_mode_animation(&new_mode);

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
                self.landing_state = None;
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

    /// Trigger visual animation effects when mode changes
    ///
    /// Creates a brief status line flash effect to visually indicate mode transitions.
    /// The effect is a bright flash that fades to the mode's background color.
    fn trigger_mode_animation(&self, new_mode: &ModeState) {
        use crate::animation::UiElementId;

        // Get animation handle from plugin state
        let Some(handle) = self.plugin_state.animation_handle() else {
            return;
        };

        // Get the new mode's background color for the flash effect
        let mode_color = self.get_mode_bg_color(new_mode);

        // Create a bright flash color (boost the mode color towards white)
        let flash_color = Self::brighten_color(mode_color, 0.6);

        // Create a status line flash effect: bright flash → mode color
        // This provides a "glowing" indication of mode change
        let flash_effect = Effect::new(
            EffectId::new(0), // ID will be assigned by handle.start()
            EffectTarget::UiElement(UiElementId::StatusLine),
            AnimatedStyle::transition_bg(
                flash_color,
                mode_color,
                250, // 250ms duration for smooth fade
            ),
        )
        .with_duration(Duration::from_millis(250));

        // Start the flash effect
        if let Some(id) = handle.start(flash_effect) {
            tracing::debug!("Started status line flash animation with effect id {:?}", id);
        }
    }

    /// Brighten a color by blending it towards white
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn brighten_color(color: reovim_sys::style::Color, intensity: f32) -> reovim_sys::style::Color {
        use reovim_sys::style::Color;

        // Extract RGB components
        let (r, g, b) = match color {
            Color::Rgb { r, g, b } => (r, g, b),
            Color::AnsiValue(n) => Self::ansi_to_rgb(n),
            Color::Black => (0, 0, 0),
            Color::DarkGrey | Color::Reset => (128, 128, 128),
            Color::Red => (255, 0, 0),
            Color::DarkRed => (139, 0, 0),
            Color::Green => (0, 255, 0),
            Color::DarkGreen => (0, 100, 0),
            Color::Yellow => (255, 255, 0),
            Color::DarkYellow => (128, 128, 0),
            Color::Blue => (0, 0, 255),
            Color::DarkBlue => (0, 0, 139),
            Color::Magenta => (255, 0, 255),
            Color::DarkMagenta => (139, 0, 139),
            Color::Cyan => (0, 255, 255),
            Color::DarkCyan => (0, 139, 139),
            Color::White => (255, 255, 255),
            Color::Grey => (192, 192, 192),
        };

        // Blend towards white using mul_add for better precision
        let blend = |c: u8| -> u8 {
            let f = f32::from(c) / 255.0;
            let brightened = (1.0 - f).mul_add(intensity, f);
            (brightened.min(1.0) * 255.0) as u8
        };

        Color::Rgb {
            r: blend(r),
            g: blend(g),
            b: blend(b),
        }
    }

    /// Convert ANSI 256 color to RGB
    const fn ansi_to_rgb(n: u8) -> (u8, u8, u8) {
        match n {
            0 => (0, 0, 0),
            1 => (128, 0, 0),
            2 => (0, 128, 0),
            3 => (128, 128, 0),
            4 => (0, 0, 128),
            5 => (128, 0, 128),
            6 => (0, 128, 128),
            7 => (192, 192, 192),
            8 => (128, 128, 128),
            9 => (255, 0, 0),
            10 => (0, 255, 0),
            11 => (255, 255, 0),
            12 => (0, 0, 255),
            13 => (255, 0, 255),
            14 => (0, 255, 255),
            15 => (255, 255, 255),
            16..=231 => {
                let idx = n - 16;
                let r = (idx / 36) % 6;
                let g = (idx / 6) % 6;
                let b = idx % 6;
                let r_val = if r == 0 { 0 } else { 55 + r * 40 };
                let g_val = if g == 0 { 0 } else { 55 + g * 40 };
                let b_val = if b == 0 { 0 } else { 55 + b * 40 };
                (r_val, g_val, b_val)
            }
            232..=255 => {
                let gray = 8 + (n - 232) * 10;
                (gray, gray, gray)
            }
        }
    }

    /// Get the background color for a mode from the theme
    fn get_mode_bg_color(&self, mode: &ModeState) -> reovim_sys::style::Color {
        let mode_styles = &self.theme.statusline.mode;

        // Check sub-modes first
        match &mode.sub_mode {
            SubMode::Command => {
                return mode_styles
                    .command
                    .bg
                    .unwrap_or(reovim_sys::style::Color::Blue);
            }
            SubMode::OperatorPending { .. } => {
                return mode_styles
                    .operator_pending
                    .bg
                    .unwrap_or(reovim_sys::style::Color::Yellow);
            }
            SubMode::Interactor(_) | SubMode::None => {}
        }

        // Then check edit mode
        match &mode.edit_mode {
            EditMode::Normal => mode_styles
                .normal
                .bg
                .unwrap_or(reovim_sys::style::Color::Blue),
            EditMode::Insert(_) => mode_styles
                .insert
                .bg
                .unwrap_or(reovim_sys::style::Color::Green),
            EditMode::Visual(_) => mode_styles
                .visual
                .bg
                .unwrap_or(reovim_sys::style::Color::Magenta),
        }
    }

    /// Handle interactor input events by routing to the active interactor
    fn handle_interactor_input(&mut self, event: TextInputEvent) {
        use crate::{
            event_bus::{
                DynEvent,
                core_events::{PluginBackspace, PluginTextInput},
            },
            modd::{ComponentId, SubMode},
        };

        let interactor_id = self.mode_state.interactor_id;

        // Check if there's an Interactor sub-mode that should receive input instead
        // This allows plugins like which-key to receive text input while
        // keeping the main interactor as EDITOR
        let sub_mode_interactor = match &self.mode_state.sub_mode {
            SubMode::Interactor(id) => Some(*id),
            _ => None,
        };

        // If there's a sub-mode interactor, route input to it via plugin path
        if let Some(target_id) = sub_mode_interactor {
            let dyn_event = match event {
                TextInputEvent::InsertChar(c) => DynEvent::new(PluginTextInput {
                    target: target_id,
                    c,
                }),
                TextInputEvent::DeleteCharBackward => {
                    DynEvent::new(PluginBackspace { target: target_id })
                }
            };

            let sender = self.event_bus.sender();
            let mut ctx = crate::event_bus::HandlerContext::new(&sender);
            let _ = self.event_bus.dispatch(&dyn_event, &mut ctx);
            if ctx.render_requested() {
                self.request_render();
            }
            return;
        }

        // Fast path: Built-in components with direct Runtime access
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

    /// Trigger yank blink animation for the yanked region
    ///
    /// Creates a brief flash effect on the yanked text range to provide visual feedback.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn trigger_yank_animation(
        &self,
        buffer_id: usize,
        start_pos: crate::screen::Position,
        motion: crate::motion::Motion,
        count: usize,
        _line_count: usize,
    ) {
        use crate::buffer::calculate_motion;

        let Some(buffer) = self.buffers.get(&buffer_id) else {
            return;
        };

        // Calculate the target position
        let target = calculate_motion(&buffer.contents, start_pos, motion, count);

        // For linewise motions, animate entire lines
        if motion.is_linewise() {
            let start_line = start_pos.y.min(target.y);
            let end_line = start_pos.y.max(target.y);

            // Get the length of the last line for end column
            #[allow(clippy::cast_possible_truncation)]
            let end_col = buffer
                .contents
                .get(end_line as usize)
                .map_or(0, |line| line.inner.len() as u16);

            self.trigger_yank_range_animation(
                buffer_id,
                crate::screen::Position {
                    x: 0,
                    y: start_line,
                },
                crate::screen::Position {
                    x: end_col,
                    y: end_line,
                },
            );
        } else {
            // For characterwise motions, use position-based range
            let (start, end) = if start_pos.y < target.y
                || (start_pos.y == target.y && start_pos.x <= target.x)
            {
                (start_pos, target)
            } else {
                (target, start_pos)
            };

            // Adjust end for inclusive motions
            let end = if motion.is_inclusive() {
                crate::screen::Position {
                    x: end.x.saturating_add(1),
                    y: end.y,
                }
            } else {
                end
            };

            self.trigger_yank_range_animation(buffer_id, start, end);
        }
    }

    /// Trigger yank blink animation for a specific range
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn trigger_yank_range_animation(
        &self,
        buffer_id: usize,
        start: crate::screen::Position,
        end: crate::screen::Position,
    ) {
        let Some(handle) = self.plugin_state.animation_handle() else {
            return;
        };

        // Create a brief flash effect on the yanked range
        // Use a semi-transparent yellow/gold color for yank highlight
        let yank_color = reovim_sys::style::Color::Rgb {
            r: 255,
            g: 220,
            b: 100,
        };
        let transparent = reovim_sys::style::Color::Rgb {
            r: 40,
            g: 40,
            b: 50,
        };

        // Create cell region effect
        let yank_effect = Effect::new(
            EffectId::new(0),
            EffectTarget::CellRegion {
                buffer_id,
                start_line: u32::from(start.y),
                start_col: u32::from(start.x),
                end_line: u32::from(end.y),
                end_col: u32::from(end.x),
            },
            AnimatedStyle::transition_bg(yank_color, transparent, 200),
        )
        .with_duration(Duration::from_millis(200))
        .with_priority(100); // High priority for yank feedback

        if let Some(id) = handle.start(yank_effect) {
            tracing::debug!(
                "Started yank blink animation with effect id {:?} for range {:?}-{:?}",
                id,
                start,
                end
            );
        }
    }

    /// Trigger a paste animation to highlight the pasted region
    ///
    /// Creates a pulsing cyan effect that oscillates between bright and dark cyan
    /// over 600ms (2 complete pulse cycles). This provides visual feedback when
    /// text is pasted, similar to the yank animation but with a different color
    /// and animation type.
    ///
    /// # Arguments
    /// * `buffer_id` - The buffer where paste occurred
    /// * `start` - Starting position of the pasted region
    /// * `end` - Ending position of the pasted region
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn trigger_paste_animation(
        &self,
        buffer_id: usize,
        start: crate::screen::Position,
        end: crate::screen::Position,
    ) {
        let Some(handle) = self.plugin_state.animation_handle() else {
            return;
        };

        // Bright cyan for paste highlight
        let paste_bright = reovim_sys::style::Color::Rgb {
            r: 100,
            g: 220,
            b: 255,
        };
        // Darker cyan for pulse low point
        let paste_dark = reovim_sys::style::Color::Rgb {
            r: 50,
            g: 110,
            b: 150,
        };

        // Create pulse effect (cyan oscillation, 300ms period)
        let paste_effect = Effect::new(
            EffectId::new(0),
            EffectTarget::CellRegion {
                buffer_id,
                start_line: u32::from(start.y),
                start_col: u32::from(start.x),
                end_line: u32::from(end.y),
                end_col: u32::from(end.x),
            },
            AnimatedStyle::pulse_bg(paste_bright, paste_dark, 300),
        )
        .with_duration(Duration::from_millis(600)) // 2 full pulse cycles
        .with_priority(100); // High priority for paste feedback

        if let Some(id) = handle.start(paste_effect) {
            tracing::debug!(
                "Started paste pulse animation with effect id {:?} for range {:?}-{:?}",
                id,
                start,
                end
            );
        }
    }
}
