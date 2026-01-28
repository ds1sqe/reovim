//! Main event loop for the editor

use std::sync::Arc;

use crate::{
    buffer::{Buffer, SelectionOps, TextOps},
    event::{
        BufferEvent, CommandHandler, EditingEvent, FileEvent, HighlightEvent, InputEvent,
        InputEventBroker, ModeEvent, RenderEvent, RuntimeEvent, RuntimeEventPayload, SettingsEvent,
        SyntaxEvent, TerminateHandler, TextInputEvent, WindowEvent,
    },
    event_bus::ViewportScrolled,
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
    pub async fn init(self) {
        tracing::info!("Runtime initializing");

        // Create terminal-based input broker
        let input_broker = InputEventBroker::with_event_sender(self.hi_tx.clone());

        self.init_common(input_broker).await;
    }

    /// Initialize and run the editor with a custom key source.
    ///
    /// Used for server mode where keys are injected via `ChannelKeySource`
    /// instead of reading from the terminal.
    #[allow(clippy::missing_panics_doc)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::future_not_send)]
    pub async fn init_with_key_source<K: crate::io::input::KeySource + 'static>(
        self,
        key_source: K,
    ) {
        tracing::info!("Runtime initializing (server mode)");

        // Create key-source-based input broker
        let input_broker = InputEventBroker::with_key_source(key_source);

        self.init_common(input_broker).await;
    }

    /// Common initialization logic shared by `init()` and `init_with_key_source()`
    #[allow(clippy::missing_panics_doc)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::future_not_send)]
    async fn init_common<K: crate::io::input::KeySource + 'static>(
        mut self,
        input_broker: InputEventBroker<K>,
    ) {
        // STEP 1: Set up input handlers FIRST (before file loading)
        // HIGH PRIORITY: User input events go through the high-priority channel

        // Command handler for key-to-command translation
        // Pass mode receiver so CommandHandler can read mode from Runtime (single source of truth)
        // HIGH PRIORITY: Commands are user-initiated and must be processed immediately
        let mode_rx = self.subscribe_mode();
        let mut command_hdr = CommandHandler::new(
            self.hi_tx.clone(),
            mode_rx,
            self.keymap.clone(),
            Arc::clone(&self.command_registry),
            Arc::clone(&self.interactor_registry),
        );
        // HIGH PRIORITY: Terminate handler processes Ctrl+D kill signal
        let mut terminate_hdr = TerminateHandler::new(self.hi_tx.clone());

        input_broker.key_broker.enlist(&mut command_hdr);
        input_broker.key_broker.enlist(&mut terminate_hdr);

        tokio::spawn(async move { command_hdr.run().await });
        tokio::spawn(async move { terminate_hdr.run().await });
        tokio::spawn(async move { input_broker.subscribe().await });

        // STEP 2: Spawn EventBus event processor on dedicated OS thread
        // Uses std::thread to avoid tokio scheduler starvation under parallel load (#85)
        if let Some(mut event_rx) = self.event_bus.take_receiver() {
            let event_bus = Arc::clone(&self.event_bus);
            let lo_tx = self.lo_tx.clone();
            std::thread::spawn(move || {
                while let Some(mut event) = event_rx.blocking_recv() {
                    let event_type = event.type_name();
                    // Extract scope from event for lifecycle tracking
                    let scope = event.take_scope();

                    let sender = event_bus.sender();
                    let mut ctx =
                        crate::event_bus::HandlerContext::new(&sender).with_scope(scope.clone());
                    let _ = event_bus.dispatch(&event, &mut ctx);

                    // Decrement scope AFTER dispatch completes
                    if let Some(scope) = scope {
                        scope.decrement();
                    }

                    // If any handler requested a render, send RenderSignal to main loop
                    // LOW PRIORITY: Render signals are background events
                    if ctx.render_requested() {
                        tracing::info!("==> Render requested by event: {}", event_type);
                        let _ = lo_tx.try_send(RuntimeEvent::render_signal());
                    }
                }
            });
        }

        // STEP 3: Let queued events process (RegisterLanguage events from subscribe phase)
        tokio::task::yield_now().await;

        // STEP 4: Boot phase - plugins can do post-EventBus initialization
        // Languages are now registered, syntax providers can be created
        // Make inner_event_tx available to plugins for background tasks (e.g., completion saturator)
        // LOW PRIORITY: Plugin background tasks use the low-priority channel
        self.plugin_state.set_inner_event_tx(self.lo_tx.clone());
        tracing::debug!("Boot phase starting with {} plugins", self.plugins.len());
        for plugin in &self.plugins {
            let plugin_id = plugin.id();
            tracing::debug!(plugin = %plugin_id, "Booting plugin");
            // LOW PRIORITY: Plugin boot events are background operations
            plugin.boot(&self.event_bus, Arc::clone(&self.plugin_state), Some(self.lo_tx.clone()));
        }
        tracing::debug!("Boot phase complete");

        // STEP 5: Load file AFTER languages are registered
        if let Some(path) = self.initial_file.clone() {
            // Use create_buffer_from_file which handles treesitter parsing and decorations
            if let Some(buffer_id) = self.create_buffer_from_file(&path) {
                // Update active buffer to the newly created one
                self.screen.set_editor_buffer(buffer_id);

                // Emit initial ViewportScrolled to trigger context computation
                if let Some(viewport_info) = self.screen.get_viewport_info(buffer_id) {
                    self.event_bus.emit(ViewportScrolled {
                        window_id: viewport_info.window_id,
                        buffer_id: viewport_info.buffer_id,
                        top_line: viewport_info.top_line,
                        bottom_line: viewport_info.bottom_line,
                    });
                    tracing::debug!(buffer_id, "init: emitted initial ViewportScrolled");
                }
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

    /// The main event processing loop with priority-based dual-channel architecture
    ///
    /// Uses biased select! to ensure high-priority events (user input) are processed
    /// before low-priority events (render signals, background tasks).
    #[allow(clippy::collapsible_if)]
    #[allow(clippy::match_same_arms)]
    #[allow(clippy::future_not_send)]
    #[allow(clippy::too_many_lines)]
    async fn run_event_loop(&mut self) {
        use {crate::constants::MAX_LO_DRAIN, std::time::Duration};

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
                // Use biased selection to process high-priority events first
                biased;

                // HIGH PRIORITY: User input events (commands, mode changes, text input)
                Some(ev) = self.hi_rx.recv() => {
                    let loop_start = std::time::Instant::now();
                    let ev_name = format!("{:?}", std::mem::discriminant(ev.payload()));

                    // Track user input for idle detection
                    self.track_input_for_idle(ev.payload());

                    if self.handle_event(ev) {
                        break;
                    }

                    // Drain ALL high-priority events first (user input takes precedence)
                    let mut hi_drained = 0;
                    while let Ok(ev) = self.hi_rx.try_recv() {
                        hi_drained += 1;
                        self.track_input_for_idle(ev.payload());
                        if self.handle_event(ev) {
                            self.flush_render();
                            return;
                        }
                    }

                    // Then drain some low-priority events for fairness
                    let mut lo_drained = 0;
                    for _ in 0..MAX_LO_DRAIN {
                        match self.lo_rx.try_recv() {
                            Ok(ev) => {
                                lo_drained += 1;
                                self.track_input_for_idle(ev.payload());
                                if self.handle_event(ev) {
                                    self.flush_render();
                                    return;
                                }
                            }
                            Err(_) => break,
                        }
                    }

                    // Flush render once after all pending events processed
                    let pre_render = loop_start.elapsed();
                    self.flush_render();
                    tracing::trace!(
                        "[RTT] event_loop(hi): first_ev={} hi_drained={} lo_drained={} pre_render={:?} total={:?}",
                        ev_name,
                        hi_drained,
                        lo_drained,
                        pre_render,
                        loop_start.elapsed()
                    );
                }

                // LOW PRIORITY: Background events (render signals, syntax updates, plugin events)
                Some(ev) = self.lo_rx.recv() => {
                    let loop_start = std::time::Instant::now();
                    let ev_name = format!("{:?}", std::mem::discriminant(ev.payload()));

                    // Track input for idle detection (some low-pri events may affect idle)
                    self.track_input_for_idle(ev.payload());

                    if self.handle_event(ev) {
                        break;
                    }

                    // Drain low-priority, but re-check high-priority between batches
                    let mut lo_drained = 0;
                    loop {
                        // Always check high-priority first during low-priority drain
                        while let Ok(ev) = self.hi_rx.try_recv() {
                            self.track_input_for_idle(ev.payload());
                            if self.handle_event(ev) {
                                self.flush_render();
                                return;
                            }
                        }
                        // Then process one low-priority event
                        match self.lo_rx.try_recv() {
                            Ok(ev) => {
                                lo_drained += 1;
                                self.track_input_for_idle(ev.payload());
                                if self.handle_event(ev) {
                                    self.flush_render();
                                    return;
                                }
                            }
                            Err(_) => break,
                        }
                    }

                    // Flush render once after all pending events processed
                    let pre_render = loop_start.elapsed();
                    self.flush_render();
                    tracing::trace!(
                        "[RTT] event_loop(lo): first_ev={} lo_drained={} pre_render={:?} total={:?}",
                        ev_name,
                        lo_drained,
                        pre_render,
                        loop_start.elapsed()
                    );
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
                            if let Some(buffer) = self.buffers.get_mut(&self.active_buffer_id()) {
                                buffer.set_content(&content);
                            }
                            self.request_render();
                            self.flush_render();
                        }
                    }
                }

                // Both channels closed - shutdown
                else => {
                    self.hi_tx
                        .send(RuntimeEvent::kill())
                        .await
                        .expect("cannot broadcast kill signal");
                    break;
                }
            }
        }
    }

    /// Track user input events for idle detection
    fn track_input_for_idle(&mut self, ev: &RuntimeEventPayload) {
        // Only track actual user input events
        let is_user_input = matches!(
            ev,
            RuntimeEventPayload::Command(_)
                | RuntimeEventPayload::Mode(ModeEvent::PendingKeys(_))
                | RuntimeEventPayload::Editing(EditingEvent::TextInput(_))
        );

        if is_user_input {
            self.last_input_at = std::time::Instant::now();

            // Stop idle shimmer if active
            if self.idle_shimmer_active {
                self.stop_idle_shimmer();
            }
        }
    }

    /// Handle a single event. Returns true if the editor should quit.
    #[allow(clippy::collapsible_if)]
    #[allow(clippy::match_same_arms)]
    #[allow(clippy::too_many_lines)]
    pub(crate) fn handle_event(&mut self, mut ev: RuntimeEvent) -> bool {
        // Extract scope for lifecycle tracking and store it for EventBus emissions
        let scope = ev.take_scope();
        self.current_scope.clone_from(&scope);

        // Handle the payload
        let result = match ev.into_payload() {
            RuntimeEventPayload::Buffer(buffer_event) => {
                match buffer_event {
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
                }
                false
            }
            RuntimeEventPayload::Command(cmd_event) => self.handle_command(cmd_event),
            RuntimeEventPayload::Mode(mode_event) => {
                match mode_event {
                    ModeEvent::Change(new_mode) => {
                        self.handle_mode_change(new_mode);
                    }
                    ModeEvent::PendingKeys(keys) => {
                        // If pending_keys is being cleared and had content, save as last_command
                        if keys.is_empty() && !self.pending_keys.is_empty() {
                            self.last_command.clone_from(&self.pending_keys);
                        }
                        // Update plugin state so which-key and other plugins can access pending keys
                        self.plugin_state.set_pending_keys(keys.clone());
                        self.pending_keys = keys;
                        self.request_render();
                    }
                }
                false
            }
            RuntimeEventPayload::Window(window_event) => {
                match window_event {
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
                }
                false
            }
            RuntimeEventPayload::Render(render_event) => {
                match render_event {
                    RenderEvent::Signal => {
                        self.request_render();
                    }
                    RenderEvent::Highlight(hl_event) => match hl_event {
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
                    RenderEvent::Syntax(syntax_event) => match syntax_event {
                        SyntaxEvent::Attach { buffer_id, syntax } => {
                            if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
                                buffer.attach_syntax(syntax);
                                // Start saturator for background cache computation
                                // LOW PRIORITY: Saturator sends render signals which are background events
                                if !buffer.has_saturator() {
                                    buffer.start_saturator(self.lo_tx.clone());
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
                    },
                }
                false
            }
            RuntimeEventPayload::Editing(editing_event) => {
                match editing_event {
                    EditingEvent::TextInput(focus_event) => {
                        self.handle_interactor_input(focus_event);
                    }
                    EditingEvent::OperatorMotion(action) => {
                        self.handle_operator_motion(&action);
                    }
                    EditingEvent::VisualTextObject(action) => {
                        self.handle_visual_text_object(&action);
                    }
                    EditingEvent::MoveCursor {
                        buffer_id,
                        line,
                        column,
                    } => {
                        tracing::debug!(
                            "Runtime: Moving cursor to line={}, col={} in buffer={}",
                            line,
                            column,
                            buffer_id
                        );
                        if let Some(buffer) = self.buffers.get_mut(&buffer_id) {
                            use crate::{buffer::CursorOps, motion::Motion};
                            buffer.apply_motion(
                                Motion::JumpTo { line, column },
                                1, // count
                            );
                        } else {
                            tracing::warn!(
                                "Runtime: Buffer {} not found for cursor move",
                                buffer_id
                            );
                        }
                    }
                    EditingEvent::SetRegister { register, text } => {
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
            RuntimeEventPayload::Settings(settings_event) => {
                match settings_event {
                    SettingsEvent::LineNumbers { enabled } => {
                        tracing::info!("Runtime: Setting line numbers: {}", enabled);
                        self.screen.set_number(enabled);
                    }
                    SettingsEvent::RelativeLineNumbers { enabled } => {
                        tracing::info!("Runtime: Setting relative line numbers: {}", enabled);
                        self.screen.set_relative_number(enabled);
                    }
                    SettingsEvent::Theme { name } => {
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
                    SettingsEvent::Scrollbar { enabled } => {
                        tracing::info!("Runtime: Setting scrollbar: {}", enabled);
                        self.screen.set_scrollbar(enabled);
                    }
                    SettingsEvent::IndentGuide { enabled } => {
                        tracing::info!("Runtime: Setting indent guide: {}", enabled);
                        self.indent_analyzer.set_enabled(enabled);
                    }
                    SettingsEvent::SignColumn { mode } => {
                        tracing::info!("Runtime: Setting sign column mode: {:?}", mode);
                        self.screen.set_sign_column_mode(mode);
                    }
                    SettingsEvent::ApplyCmdlineCompletion {
                        text,
                        replace_start,
                    } => {
                        tracing::debug!(
                            "Runtime: Applying cmdline completion: {:?} at {}",
                            text,
                            replace_start
                        );
                        self.command_line.apply_completion(&text, replace_start);
                        self.request_render();
                    }
                }
                false
            }
            RuntimeEventPayload::Input(input_event) => {
                match input_event {
                    InputEvent::ScreenResize { width, height } => {
                        tracing::debug!("Screen resize: {}x{}", width, height);
                        self.screen.resize(width, height);
                        self.request_render();
                    }
                    InputEvent::Mouse(mouse_event) => {
                        self.handle_mouse_event(mouse_event);
                    }
                }
                false
            }
            RuntimeEventPayload::File(file_event) => {
                match file_event {
                    FileEvent::Open { path } => {
                        tracing::info!("Runtime: Opening file from request: {:?}", path);
                        // Convert PathBuf to &str for open_file
                        if let Some(path_str) = path.to_str() {
                            self.open_file(path_str);
                            self.screen.set_editor_buffer(self.active_buffer_id());
                            self.request_render();
                        } else {
                            tracing::error!("Runtime: Invalid UTF-8 in file path: {:?}", path);
                        }
                    }
                    FileEvent::OpenAt { path, line, column } => {
                        tracing::info!(
                            "Runtime: Opening file at position: {:?}:{}:{}",
                            path,
                            line,
                            column
                        );
                        if let Some(path_str) = path.to_str() {
                            self.open_file(path_str);
                            self.screen.set_editor_buffer(self.active_buffer_id());
                            // Set cursor position in the opened buffer
                            if let Some(buffer) = self.buffers.get_mut(&self.active_buffer_id()) {
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
                }
                false
            }
            RuntimeEventPayload::Rpc(rpc_event) => {
                let response =
                    self.handle_rpc_request(rpc_event.id, &rpc_event.method, &rpc_event.params);
                let _ = rpc_event.response_tx.send(response);
                false
            }
            RuntimeEventPayload::Plugin(plugin_data) => {
                tracing::debug!(
                    "Runtime: Dispatching plugin event from {}: {:?}",
                    plugin_data.plugin_id,
                    plugin_data.event.type_name()
                );
                // Dispatch to event bus - subscribers will receive the event
                let sender = self.event_bus.sender();
                let mut ctx = crate::event_bus::HandlerContext::new(&sender);
                let _ = self.event_bus.dispatch(&plugin_data.event, &mut ctx);
                if ctx.render_requested() {
                    self.request_render();
                }
                false
            }
            RuntimeEventPayload::Kill => true,
        };

        // Clear current scope
        // Note: EventBus events emitted via emit_event() are dispatched synchronously
        // when a scope is present, so no drain is needed here.
        self.current_scope = None;

        // Decrement scope AFTER processing completes
        if let Some(scope) = scope {
            scope.decrement();
        }

        result
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
            if let Some(buffer) = self.buffers.get_mut(&self.active_buffer_id()) {
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
            let dispatch_result = self.event_bus.dispatch(&dyn_event, &mut ctx);
            tracing::info!("==> Dispatch result for sub_mode interactor: {:?}", dispatch_result);
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
        let dispatch_result = self.event_bus.dispatch(&dyn_event, &mut ctx);
        tracing::info!("==> Dispatch result for plugin path: {:?}", dispatch_result);
        if ctx.render_requested() {
            self.request_render();
        }
    }
}
