//! Main event loop for the runner.
//!
//! The event loop is the heart of the runner - it reads key events,
//! looks up keybindings, executes commands, and delegates unmatched
//! keys to the fallback handler.
//!
//! # Design Philosophy
//!
//! Following the "mechanism vs policy" principle:
//! - **Mechanism** (this module): Key dispatch, command execution orchestration
//! - **Policy** (modules): Actual command behavior, fallback handling
//!
//! The event loop contains NO business logic - it just wires things together.

use {
    reovim_driver_command::{CommandContext, CommandResult, UndoAction},
    reovim_driver_input::{FallbackResult, InputFallbackHandler, KeyCode, KeyEvent},
    reovim_kernel::{
        api::v1::{
            Edit, EventResult, ModeId, Motion, MotionEngine, Position, UndoResult,
            events::ModeChanged,
        },
        profile_scope,
    },
    std::sync::{Arc, Mutex},
};

use super::{
    AppState,
    app::LastVisualSelection,
    registry::{CommandRegistry, KeyLookupResult, KeymapRegistry, ModeRegistry},
};

/// Error type for event loop operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventLoopError {
    /// Error reading input.
    InputError(String),
    /// Error during rendering.
    RenderError(String),
}

impl std::fmt::Display for EventLoopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputError(msg) => write!(f, "input error: {msg}"),
            Self::RenderError(msg) => write!(f, "render error: {msg}"),
        }
    }
}

impl std::error::Error for EventLoopError {}

/// Main event loop for the runner.
///
/// Generic over the fallback handler type `F`, allowing different modules
/// to provide different fallback behavior without changing the event loop.
///
/// # Type Parameter
///
/// * `F` - The fallback handler type implementing [`InputFallbackHandler<AppState>`]
///
/// # Example
///
/// ```ignore
/// use runner::{AppState, EventLoop, NoOpFallback};
/// use runner::registry::{ModeRegistry, CommandRegistry, KeymapRegistry};
///
/// let app = AppState::new(kernel, initial_mode);
/// let mut event_loop = EventLoop::new(
///     app,
///     ModeRegistry::new(),
///     CommandRegistry::new(),
///     KeymapRegistry::new(),
///     NoOpFallback,
/// );
///
/// // Run until quit
/// event_loop.run()?;
/// ```
pub struct EventLoop<F: InputFallbackHandler<AppState>> {
    /// Application state (kernel + runtime).
    app: AppState,

    /// Registry for mode metadata.
    mode_registry: ModeRegistry,

    /// Registry for command handlers.
    command_registry: CommandRegistry,

    /// Registry for keybindings.
    keymap_registry: KeymapRegistry,

    /// Handler for unmatched key events.
    fallback_handler: F,

    /// Callback for key input (for testing/injection).
    key_reader: Option<Box<dyn FnMut() -> Option<KeyEvent> + Send>>,

    /// Last error message (for status line).
    last_error: Option<String>,

    /// Pending mode change from `ModeChanged` events.
    ///
    /// Commands emit `ModeChanged` events with `target_mode`. The event handler
    /// stores the target here, and we apply it after command execution.
    /// This enables event-driven mode transitions without hardcoded command names.
    pending_mode_change: Arc<Mutex<Option<ModeId>>>,
}

impl<F: InputFallbackHandler<AppState>> EventLoop<F> {
    /// Create a new event loop.
    ///
    /// # Arguments
    ///
    /// * `app` - Application state
    /// * `mode_registry` - Registry of modes
    /// * `command_registry` - Registry of commands
    /// * `keymap_registry` - Registry of keybindings
    /// * `fallback_handler` - Handler for unmatched keys
    #[must_use]
    pub fn new(
        app: AppState,
        mode_registry: ModeRegistry,
        command_registry: CommandRegistry,
        keymap_registry: KeymapRegistry,
        fallback_handler: F,
    ) -> Self {
        // Create shared storage for pending mode changes from ModeChanged events
        let pending_mode_change: Arc<Mutex<Option<ModeId>>> = Arc::new(Mutex::new(None));

        // Subscribe to ModeChanged events to capture target_mode
        let pending_clone = Arc::clone(&pending_mode_change);
        app.kernel.event_bus.subscribe::<ModeChanged, _>(
            0, // Priority 0 (highest) - mode changes should be processed first
            move |event| {
                if let Some(mode_id) = event.target_mode()
                    && let Ok(mut guard) = pending_clone.lock()
                {
                    *guard = Some(mode_id.clone());
                }
                EventResult::Handled
            },
        );

        Self {
            app,
            mode_registry,
            command_registry,
            keymap_registry,
            fallback_handler,
            key_reader: None,
            last_error: None,
            pending_mode_change,
        }
    }

    /// Set a custom key reader for testing.
    ///
    /// The reader is called to get each key event. When it returns `None`,
    /// the event loop exits.
    #[must_use]
    pub fn with_key_reader<R>(mut self, reader: R) -> Self
    where
        R: FnMut() -> Option<KeyEvent> + Send + 'static,
    {
        self.key_reader = Some(Box::new(reader));
        self
    }

    /// Run the event loop until quit.
    ///
    /// # Errors
    ///
    /// Returns an error if input reading or rendering fails.
    pub fn run(&mut self) -> Result<(), EventLoopError> {
        while self.app.is_running() {
            // Read next key
            let Some(key) = self.read_key()? else {
                // No more keys (EOF or test ended)
                break;
            };

            // Handle the key
            self.handle_key(key);

            // Render if needed (TODO: actual rendering in Phase 4)
            self.render_if_needed()?;
        }

        Ok(())
    }

    /// Run a single iteration (for testing).
    ///
    /// Returns `true` if a key was processed, `false` if no key available.
    ///
    /// # Errors
    ///
    /// Returns an error if input reading or rendering fails.
    pub fn step(&mut self) -> Result<bool, EventLoopError> {
        let Some(key) = self.read_key()? else {
            return Ok(false);
        };

        self.handle_key(key);
        self.render_if_needed()?;

        Ok(true)
    }

    /// Handle a single key event.
    ///
    /// This is the core dispatch logic:
    /// 0. Check for char-wait state (find-char commands)
    /// 1. Add key to pending sequence
    /// 2. Look up in keymap
    /// 3. If found: execute command
    /// 4. If prefix: wait for more keys
    /// 5. If not found: delegate to fallback handler
    fn handle_key(&mut self, key: KeyEvent) {
        profile_scope!("handle_key", "runner::event_loop");

        // Check for search input mode (typing search pattern)
        if self.app.search.input_active {
            self.handle_search_input(key);
            return;
        }

        // Check for pending character operation (f/F/t/T or r waiting for character)
        if self.app.has_pending_char() {
            self.handle_pending_char(key);
            return;
        }

        // Add to pending sequence
        self.app.pending_keys.push(key);

        // Get current mode
        let mode = self.app.current_mode().clone();

        // Look up in keymap
        match self.keymap_registry.lookup(&mode, &self.app.pending_keys) {
            KeyLookupResult::Found(cmd_id) => {
                // Flush pending edits before command execution
                // (any command breaks insert mode batching)
                self.app.flush_pending_edits();

                // Build command context (TODO: parse count/register from pending keys)
                let mut ctx = CommandContext::new();

                // Set buffer_id in context if we have an active buffer
                if let Some(buffer_id) = self.app.active_buffer {
                    ctx.set_buffer_id(buffer_id);
                }

                // Save visual selection if currently in visual mode (for gv command).
                // We check the current mode rather than hardcoding command names that exit visual.
                // Any command that exits visual mode will have the selection saved before execution.
                let in_visual_mode = self.app.current_mode().name().starts_with("visual");
                if in_visual_mode {
                    self.save_visual_selection_if_active();
                }

                // Clear pending mode change before command execution
                if let Ok(mut guard) = self.pending_mode_change.lock() {
                    *guard = None;
                }

                // Execute command
                if let Some(result) = self.command_registry.execute(&cmd_id, &mut self.app, &ctx) {
                    self.handle_command_result(result);

                    // Handle mode transition from ModeChanged events
                    // Commands emit ModeChanged::with_mode_id() which sets pending_mode_change.
                    // This event-driven approach replaces hardcoded mode_for_command() mapping.
                    let pending_new_mode = self
                        .pending_mode_change
                        .lock()
                        .ok()
                        .and_then(|mut guard| guard.take());
                    if let Some(new_mode) = pending_new_mode {
                        // Check if exiting insert mode with block insert active
                        let old_mode = self.app.current_mode();
                        if old_mode.name() == "insert" && self.app.block_insert.is_active() {
                            self.apply_block_insert_text();
                        }
                        self.app.mode_stack.set(new_mode);
                    }
                } else {
                    // Command not found in registry
                    self.set_error(format!("Unknown command: {cmd_id}"));
                }

                // Clear pending keys
                self.app.clear_pending_keys();
            }

            KeyLookupResult::Prefix => {
                // Wait for more keys - don't clear pending
            }

            KeyLookupResult::NotFound => {
                // Delegate to fallback handler (MECHANISM delegates to POLICY)
                // The handler uses ctx.record_edit() for undo tracking
                let result = self.fallback_handler.handle_unmatched(key, &mut self.app);

                match result {
                    FallbackResult::Handled | FallbackResult::Ignored => {
                        // Key was handled (char inserted) or ignored
                    }
                    FallbackResult::Beep => {
                        // Show warning
                        self.set_error("Invalid key");
                    }
                }

                // Clear pending keys
                self.app.clear_pending_keys();
            }
        }
    }

    /// Handle a key event when in search input mode (typing pattern).
    ///
    /// Keys are buffered until Enter executes the search or Escape cancels.
    fn handle_search_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Escape => {
                // Cancel search input
                self.app.search.cancel_input();
                self.app.clear_pending_keys();
            }
            KeyCode::Enter => {
                // Complete search input and execute
                if let Some((pattern, direction)) = self.app.search.complete_input() {
                    self.execute_search(&pattern, direction);
                }
                self.app.clear_pending_keys();
            }
            KeyCode::Backspace => {
                // Delete last character from input buffer
                self.app.search.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                // Add character to input buffer
                self.app.search.input_buffer.push(c);
            }
            _ => {
                // Ignore other keys in search input mode
            }
        }
    }

    /// Handle a key event when in pending-char state (f/F/t/T or r pending).
    ///
    /// The key provides the character argument for the pending operation.
    /// Escape cancels the wait without executing any operation.
    fn handle_pending_char(&mut self, key: KeyEvent) {
        use super::app::PendingCharOp;

        // Take the pending-char state
        let pending = self
            .app
            .take_pending_char()
            .expect("pending_char should be Some");

        // Handle escape - cancel pending operation
        if key.code == KeyCode::Escape {
            self.app.clear_pending_keys();
            return;
        }

        // Extract character from key event
        let KeyCode::Char(c) = key.code else {
            // Non-character keys cancel pending operation
            self.app.clear_pending_keys();
            return;
        };

        match pending {
            PendingCharOp::FindForward { start: _ }
            | PendingCharOp::FindBackward { start: _ }
            | PendingCharOp::TillForward { start: _ }
            | PendingCharOp::TillBackward { start: _ } => {
                self.execute_find_char(c, &pending);
            }
            PendingCharOp::ReplaceChar { count } => {
                self.execute_replace_char(c, count);
            }
        }

        self.app.clear_pending_keys();
    }

    /// Execute a find-char motion with the given character.
    fn execute_find_char(&mut self, c: char, pending: &super::app::PendingCharOp) {
        use super::app::LastFind;

        let Some(find_type) = pending.find_type() else {
            return; // Not a find-char operation
        };

        // Build the find-char motion
        let motion = Motion::FindChar {
            char: c,
            direction: find_type.direction(),
            till: find_type.is_till(),
        };

        // Get active buffer for motion calculation
        let Some(buffer_id) = self.app.active_buffer else {
            self.set_error("No active buffer");
            return;
        };

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            self.set_error("Buffer not found");
            return;
        };

        // Calculate motion target
        let buffer = buffer_arc.read();
        let target = MotionEngine::calculate(&buffer, buffer.cursor(), motion, 1);
        drop(buffer);

        // Apply motion if target found
        if let Some(pos) = target {
            buffer_arc.write().set_position(pos);

            // Update last_find for ; and , repeat
            self.app.last_find = Some(LastFind::new(c, find_type));
        }
        // Note: If target is None (char not found), cursor doesn't move,
        // and last_find is NOT updated (per Vim behavior)
    }

    /// Execute a replace-char operation with the given character.
    fn execute_replace_char(&mut self, c: char, count: usize) {
        // Get active buffer
        let Some(buffer_id) = self.app.active_buffer else {
            self.set_error("No active buffer");
            return;
        };

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            self.set_error("Buffer not found");
            return;
        };

        let mut buffer = buffer_arc.write();
        let cursor_pos = buffer.cursor().position;

        // Replace count characters with c
        let line = buffer.lines().get(cursor_pos.line).map(String::from);
        if let Some(line) = line {
            let start_col = cursor_pos.column;
            let end_col = (start_col + count).min(line.len());

            if start_col < line.len() {
                // Build replacement string
                let replacement: String = std::iter::repeat_n(c, end_col - start_col).collect();

                // Delete old characters and insert new
                let start = Position::new(cursor_pos.line, start_col);
                let end = Position::new(cursor_pos.line, end_col);
                buffer.delete_range(start, end);
                buffer.insert_at(start, &replacement);

                // Cursor stays at start position (Vim behavior)
                buffer.set_position(start);
            }
        }
    }

    /// Handle command execution result.
    fn handle_command_result(&mut self, result: CommandResult) {
        match result {
            CommandResult::Success => {
                // Clear any previous error
                self.last_error = None;
            }
            CommandResult::Error(msg) => {
                self.set_error(msg);
            }
            CommandResult::Quit | CommandResult::ForceQuit => {
                self.app.request_quit();
            }
            CommandResult::UndoAction(action) => {
                self.handle_undo_action(action);
            }
            CommandResult::EditAction(action) => {
                // Record edit in undo registry for later undo/redo
                self.app.undo_registry.record(
                    action.buffer_id,
                    action.edits,
                    action.cursor_before,
                    action.cursor_after,
                );
                self.last_error = None;
            }
            CommandResult::UndotreeAction(action) => {
                // Undotree visualization actions are handled by the runner.
                // Full implementation in #257.
                self.handle_undotree_action(action);
            }
            CommandResult::WindowAction(action) => {
                // Window management actions are handled by the runner.
                // Full implementation in Phase 5 of #276.
                self.handle_window_action(action);
            }
            CommandResult::ModeAction(action) => {
                // Mode stack actions are handled by the runner.
                self.handle_mode_action(action);
            }
            CommandResult::WaitingForChar(ctx) => {
                // Command (f/F/t/T or r) needs a character argument.
                // Set pending-char state so next key press completes the operation.
                use {
                    super::app::{FindType, PendingCharOp},
                    reovim_driver_command::CharWaitOp,
                };

                let pending = match ctx.op_type {
                    CharWaitOp::FindForward => {
                        let start = ctx.start_position.expect("FindForward requires position");
                        PendingCharOp::from_find_type(FindType::FindForward, start)
                    }
                    CharWaitOp::FindBackward => {
                        let start = ctx.start_position.expect("FindBackward requires position");
                        PendingCharOp::from_find_type(FindType::FindBackward, start)
                    }
                    CharWaitOp::TillForward => {
                        let start = ctx.start_position.expect("TillForward requires position");
                        PendingCharOp::from_find_type(FindType::TillForward, start)
                    }
                    CharWaitOp::TillBackward => {
                        let start = ctx.start_position.expect("TillBackward requires position");
                        PendingCharOp::from_find_type(FindType::TillBackward, start)
                    }
                    CharWaitOp::ReplaceChar => {
                        let count = ctx.count.unwrap_or(1);
                        PendingCharOp::replace_char(count)
                    }
                };

                self.app.set_pending_char(pending);
                self.last_error = None;
            }
            CommandResult::RepeatFindSame => {
                // Repeat last find-char in the same direction (;)
                self.execute_repeat_find(false);
            }
            CommandResult::RepeatFindReverse => {
                // Repeat last find-char in the opposite direction (,)
                self.execute_repeat_find(true);
            }
            CommandResult::SearchAction(ref action) => {
                // Search commands (/, ?, n, N, *, #, :noh) return search actions.
                // The runner handles input mode, pattern storage, and search execution.
                self.handle_search_action(action);
            }
            CommandResult::RepeatAction => {
                // Repeat command (.) wants to replay the last repeatable command.
                // Full implementation comes in Phase 5.
                tracing::debug!("RepeatAction requested - not yet implemented");
                self.last_error = None;
            }
            CommandResult::OperatorRange {
                start,
                end,
                is_linewise,
            } => {
                // Text object command returned a range for the pending operator.
                // Execute the pending operator (d, y, c) with this range.
                tracing::debug!(
                    ?start,
                    ?end,
                    is_linewise,
                    "OperatorRange received from text object"
                );
                // TODO: Execute pending operator with range
                // For now, just clear error state
                self.last_error = None;
            }
            CommandResult::ReselectVisual => {
                // The reselect-last (gv) command requests restoration of the
                // last visual selection. Handle it here instead of checking
                // the command name.
                self.handle_reselect_last();
            }
            CommandResult::BlockInsertAction(action) => {
                // Visual-block I/A commands return this to initiate block insert mode.
                // Store the block bounds and enter insert mode.
                self.handle_block_insert_action(action);
            }
        }
    }

    /// Handle a search action from search commands.
    fn handle_search_action(&mut self, action: &reovim_driver_command::SearchAction) {
        use {crate::server::app::SearchDirection, reovim_driver_command::SearchAction};

        match *action {
            SearchAction::EnterSearchMode { direction } => {
                let dir = match direction {
                    reovim_driver_command::SearchDirection::Forward => SearchDirection::Forward,
                    reovim_driver_command::SearchDirection::Backward => SearchDirection::Backward,
                };
                self.app.search.start_input(dir);
                self.last_error = None;
            }
            SearchAction::Next => {
                // Go to next match in same direction
                if let Some(pattern) = self.app.search.pattern.clone() {
                    let direction = self.app.search.direction;
                    self.execute_search(&pattern, direction);
                } else {
                    self.set_error("No previous search pattern");
                }
            }
            SearchAction::Previous => {
                // Go to previous match (reverse direction)
                if let Some(pattern) = self.app.search.pattern.clone() {
                    let direction = match self.app.search.direction {
                        SearchDirection::Forward => SearchDirection::Backward,
                        SearchDirection::Backward => SearchDirection::Forward,
                    };
                    self.execute_search(&pattern, direction);
                } else {
                    self.set_error("No previous search pattern");
                }
            }
            SearchAction::WordUnderCursor { direction } => {
                self.execute_word_search(direction);
            }
            SearchAction::ClearHighlight => {
                self.app.search.clear_highlight();
                self.last_error = None;
            }
        }
    }

    /// Handle a window action from window commands.
    fn handle_window_action(&mut self, action: reovim_driver_command::WindowAction) {
        use {
            reovim_driver_command::WindowAction,
            reovim_kernel::api::v1::events::{WindowClosed, WindowFocused},
        };

        tracing::debug!(?action, "Handling window action");

        match action {
            WindowAction::SplitHorizontal => self.handle_split(true),
            WindowAction::SplitVertical => self.handle_split(false),
            WindowAction::CloseWindow => {
                let Some(active) = self.app.windows.active_window() else {
                    self.set_error("No active window");
                    return;
                };
                let window_id = active.raw() as u64;
                if self.app.windows.close_window(active) {
                    self.app.kernel.event_bus.emit(WindowClosed { window_id });
                    self.last_error = None;
                } else {
                    self.set_error("Cannot close last window");
                }
            }
            WindowAction::CloseOthers => {
                let Some(active) = self.app.windows.active_window() else {
                    self.set_error("No active window");
                    return;
                };
                let others: Vec<_> = self
                    .app
                    .windows
                    .windows()
                    .filter(|&id| id != active)
                    .collect();
                for id in others {
                    let window_id = id.raw() as u64;
                    if self.app.windows.close_window(id) {
                        self.app.kernel.event_bus.emit(WindowClosed { window_id });
                    }
                }
                self.last_error = None;
            }
            WindowAction::FocusDirection(direction) => {
                if let Some(new_focus) = self.app.windows.focus_direction(direction) {
                    let old_focus = self.app.windows.active_window();
                    self.app.windows.set_active_window(new_focus);
                    self.app.kernel.event_bus.emit(WindowFocused {
                        from: old_focus.map(|w| w.raw() as u64),
                        to: new_focus.raw() as u64,
                    });
                    self.last_error = None;
                }
            }
            WindowAction::CycleForward => self.handle_cycle(true),
            WindowAction::CycleBackward => self.handle_cycle(false),
            WindowAction::ResizeHeightIncrease
            | WindowAction::ResizeHeightDecrease
            | WindowAction::ResizeWidthIncrease
            | WindowAction::ResizeWidthDecrease
            | WindowAction::ResizeEqual => {
                tracing::debug!(?action, "Window resize action (deferred)");
                self.last_error = None;
            }
        }
    }

    /// Helper for window split operations.
    fn handle_split(&mut self, horizontal: bool) {
        use reovim_kernel::api::v1::events::WindowCreated;

        let Some(active) = self.app.windows.active_window() else {
            self.set_error("No active window");
            return;
        };
        let buffer_id = self.app.windows.get(active).and_then(|w| w.buffer_id);
        let new_id = if horizontal {
            self.app.windows.split_horizontal(active)
        } else {
            self.app.windows.split_vertical(active)
        };
        let Some(new_id) = new_id else {
            self.set_error("Failed to split window");
            return;
        };
        // New window gets same buffer
        if let Some(buffer_id) = buffer_id
            && let Some(w) = self.app.windows.get_mut(new_id)
        {
            w.buffer_id = Some(buffer_id);
        }
        self.app.kernel.event_bus.emit(WindowCreated {
            window_id: new_id.raw() as u64,
        });
        self.last_error = None;
    }

    /// Helper for window cycle operations.
    fn handle_cycle(&mut self, forward: bool) {
        use reovim_kernel::api::v1::events::WindowFocused;

        let old_focus = self.app.windows.active_window();
        let changed = if forward {
            self.app.windows.cycle_forward()
        } else {
            self.app.windows.cycle_backward()
        };
        if changed {
            if let Some(new_focus) = self.app.windows.active_window() {
                self.app.kernel.event_bus.emit(WindowFocused {
                    from: old_focus.map(|w| w.raw() as u64),
                    to: new_focus.raw() as u64,
                });
            }
            self.last_error = None;
        }
    }

    /// Handle a mode action from mode-changing commands.
    fn handle_mode_action(&mut self, action: reovim_driver_command::ModeAction) {
        use {reovim_driver_command::ModeAction, reovim_kernel::api::v1::ModeId};

        tracing::debug!(?action, "Handling mode action");

        match action {
            ModeAction::Push(mode_name) => {
                // Push a new mode onto the stack.
                // Mode names are typically "window", "insert", etc.
                let mode_id = ModeId::new(
                    reovim_kernel::api::v1::ModuleId::new("editor"),
                    Box::leak(mode_name.clone().into_boxed_str()),
                );
                self.app.mode_stack.push(mode_id);
                tracing::info!(mode = %mode_name, "Pushed mode onto stack");
                self.last_error = None;
            }
            ModeAction::Pop => {
                // Pop the current mode from the stack.
                if let Some(popped) = self.app.mode_stack.pop() {
                    tracing::info!(mode = %popped, "Popped mode from stack");
                }
                self.last_error = None;
            }
            ModeAction::Set(mode_name) => {
                // Replace the current mode with a new one.
                let mode_id = ModeId::new(
                    reovim_kernel::api::v1::ModuleId::new("editor"),
                    Box::leak(mode_name.clone().into_boxed_str()),
                );
                self.app.mode_stack.set(mode_id);
                tracing::info!(mode = %mode_name, "Set mode on stack");
                self.last_error = None;
            }
        }
    }

    /// Handle an undotree action from undotree commands.
    fn handle_undotree_action(&mut self, action: reovim_driver_command::UndotreeAction) {
        use reovim_driver_command::UndotreeAction;

        tracing::debug!(?action, "Handling undotree action");

        match action {
            UndotreeAction::Toggle { buffer_id } => {
                self.toggle_undotree_panel(buffer_id);
            }
            UndotreeAction::Close => {
                self.close_undotree_panel();
            }
            UndotreeAction::MoveUp => {
                self.undotree_move_up();
            }
            UndotreeAction::MoveDown => {
                self.undotree_move_down();
            }
            UndotreeAction::GotoNode { node_index } => {
                self.undotree_goto_node(node_index);
            }
            UndotreeAction::GotoSelected => {
                let selected = self.app.undotree_state.selected_node();
                self.undotree_goto_node(selected);
            }
            UndotreeAction::PreviewDiff => {
                self.undotree_preview_diff();
            }
            UndotreeAction::ClearPreview => {
                self.app.undotree_state.clear_preview();
                self.refresh_undotree_panel();
            }
        }
    }

    /// Toggle the undotree panel for the given buffer.
    ///
    /// If the panel is closed, opens it with a vertical split on the right.
    /// If the panel is open, closes it and restores focus.
    fn toggle_undotree_panel(&mut self, buffer_id: usize) {
        use reovim_kernel::api::v1::{BufferId, ModeId, ModuleId, events::WindowCreated};

        let buffer_id = BufferId::from_raw(buffer_id);

        if self.app.undotree_state.is_open() {
            // Panel is open - close it
            self.close_undotree_panel();
        } else {
            // Panel is closed - open it
            let previous_window = self.app.windows.active_window();

            // Create a vertical split on the right for the panel
            let Some(active) = previous_window else {
                self.set_error("No active window for undotree panel");
                return;
            };

            let Some(panel_id) = self.app.windows.split_vertical(active) else {
                self.set_error("Failed to create undotree panel");
                return;
            };

            // Open the panel state
            self.app
                .undotree_state
                .open(panel_id, buffer_id, previous_window);

            // Focus the panel window
            self.app.windows.set_active_window(panel_id);

            // Emit window created event
            self.app.kernel.event_bus.emit(WindowCreated {
                window_id: panel_id.raw() as u64,
            });

            // Push undotree mode onto the mode stack
            let undotree_mode = ModeId::new(ModuleId::new("undotree"), "undotree");
            self.app.mode_stack.push(undotree_mode);

            tracing::info!(
                panel_window = panel_id.raw(),
                source_buffer = buffer_id.as_usize(),
                "Opened undotree panel"
            );

            // Render initial panel content
            self.refresh_undotree_panel();
            self.last_error = None;
        }
    }

    /// Close the undotree panel.
    fn close_undotree_panel(&mut self) {
        use reovim_kernel::api::v1::events::{WindowClosed, WindowFocused};

        if !self.app.undotree_state.is_open() {
            // Already closed - no-op
            self.last_error = None;
            return;
        }

        let panel_id = self.app.undotree_state.panel_window_id();

        // Close the panel state and get the previous window for focus restoration
        let previous_window = self.app.undotree_state.close();

        // Close the panel window
        if let Some(panel_id) = panel_id
            && self.app.windows.close_window(panel_id)
        {
            self.app.kernel.event_bus.emit(WindowClosed {
                window_id: panel_id.raw() as u64,
            });
        }

        // Restore focus to the previous window
        if let Some(prev) = previous_window {
            let old_focus = self.app.windows.active_window();
            self.app.windows.set_active_window(prev);
            self.app.kernel.event_bus.emit(WindowFocused {
                from: old_focus.map(|w| w.raw() as u64),
                to: prev.raw() as u64,
            });
        }

        // Pop undotree mode from the mode stack
        if let Some(popped) = self.app.mode_stack.pop() {
            tracing::debug!(mode = %popped, "Popped undotree mode");
        }

        tracing::info!("Closed undotree panel");
        self.last_error = None;
    }

    /// Move selection up in the undotree (toward parent).
    fn undotree_move_up(&mut self) {
        if !self.app.undotree_state.is_open() {
            return;
        }

        let Some(buffer_id) = self.app.undotree_state.source_buffer_id() else {
            return;
        };

        let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
            return;
        };

        let current_selection = self.app.undotree_state.selected_node();
        let Some(node) = tree.node(current_selection) else {
            return;
        };

        // Move to parent if available
        if let Some(parent_idx) = node.parent() {
            self.app.undotree_state.set_selected_node(parent_idx);

            // Clear preview when navigating away from the previewed node
            if self.app.undotree_state.is_preview_active()
                && self.app.undotree_state.preview_node() != Some(parent_idx)
            {
                self.app.undotree_state.clear_preview();
            }

            tracing::debug!(
                from = current_selection,
                to = parent_idx,
                "Undotree selection moved up"
            );
            // Refresh the panel display
            self.refresh_undotree_panel();
        }
        // At root - no-op

        self.last_error = None;
    }

    /// Move selection down in the undotree (toward child).
    fn undotree_move_down(&mut self) {
        if !self.app.undotree_state.is_open() {
            return;
        }

        let Some(buffer_id) = self.app.undotree_state.source_buffer_id() else {
            return;
        };

        let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
            return;
        };

        let current_selection = self.app.undotree_state.selected_node();
        let Some(node) = tree.node(current_selection) else {
            return;
        };

        // Move to first child if available
        let children = node.children();
        if !children.is_empty() {
            // Use the first child (could use active branch in future)
            let child_idx = children[0];
            self.app.undotree_state.set_selected_node(child_idx);

            // Clear preview when navigating away from the previewed node
            if self.app.undotree_state.is_preview_active()
                && self.app.undotree_state.preview_node() != Some(child_idx)
            {
                self.app.undotree_state.clear_preview();
            }

            tracing::debug!(
                from = current_selection,
                to = child_idx,
                "Undotree selection moved down"
            );
            // Refresh the panel display
            self.refresh_undotree_panel();
        }
        // At leaf - no-op

        self.last_error = None;
    }

    /// Navigate to a specific node in the undotree.
    ///
    /// This applies the necessary undo/redo operations to move the buffer
    /// state to the target node. Uses the simple approach of undoing to
    /// root and redoing to target.
    fn undotree_goto_node(&mut self, target_node: usize) {
        if !self.app.undotree_state.is_open() {
            return;
        }

        // Clear preview before navigation
        self.app.undotree_state.clear_preview();

        let Some(buffer_id) = self.app.undotree_state.source_buffer_id() else {
            return;
        };

        // Check if we're already at the target
        {
            let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
                return;
            };
            if tree.current_index() == target_node {
                tracing::debug!(node = target_node, "Already at target node");
                self.last_error = None;
                return;
            }
        }

        // Calculate path from root to target
        let path_to_target = {
            let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
                return;
            };
            Self::calculate_path_to_node(tree, target_node)
        };

        // Undo to root
        loop {
            // Check if at root
            let at_root = self
                .app
                .undo_registry
                .get_tree(buffer_id)
                .is_some_and(|tree| tree.current_index() == 0);

            if at_root {
                break;
            }

            if let Some(result) = self.app.undo_registry.undo(buffer_id) {
                self.apply_undo_result(buffer_id, result);
            } else {
                break;
            }
        }

        // Redo following the path to target
        for &(node_idx, branch_idx) in &path_to_target {
            // Skip root
            if node_idx == 0 {
                continue;
            }

            if let Some(result) = self.app.undo_registry.redo_branch(buffer_id, branch_idx) {
                self.apply_undo_result(buffer_id, result);

                // Verify we reached the expected node
                let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
                    break;
                };
                if tree.current_index() != node_idx {
                    tracing::warn!(
                        expected = node_idx,
                        actual = tree.current_index(),
                        "Unexpected node after redo_branch"
                    );
                    break;
                }
            } else {
                tracing::warn!(target = node_idx, branch = branch_idx, "Failed to redo_branch");
                break;
            }
        }

        tracing::info!(target = target_node, "Navigated to undotree node");

        // Refresh the panel display
        self.refresh_undotree_panel();
        self.last_error = None;
    }

    /// Refresh the undotree panel display.
    ///
    /// Re-renders the tree with current selection highlighting and
    /// updates the cached render lines in the undotree state.
    fn refresh_undotree_panel(&mut self) {
        use {super::app::UndotreeRenderLine, reovim_module_undotree::UndotreeRenderer};

        if !self.app.undotree_state.is_open() {
            return;
        }

        let Some(buffer_id) = self.app.undotree_state.source_buffer_id() else {
            return;
        };

        let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
            return;
        };

        let selected_node = self.app.undotree_state.selected_node();

        // Render the tree
        let renderer = UndotreeRenderer::new();
        let render_lines = renderer.render(tree, selected_node);

        // Convert to our render line type
        let mut lines: Vec<UndotreeRenderLine> = render_lines
            .into_iter()
            .map(|rl| UndotreeRenderLine {
                text: rl.text,
                is_current: rl.is_current,
                is_selected: rl.is_selected,
            })
            .collect();

        // If preview is active, append separator and diff lines
        if self.app.undotree_state.is_preview_active() {
            lines.push(UndotreeRenderLine {
                text: String::from("─── Diff Preview ───"),
                is_current: false,
                is_selected: false,
            });

            for dl in self.app.undotree_state.preview_diff_lines() {
                lines.push(UndotreeRenderLine {
                    text: dl.text.clone(),
                    is_current: false,
                    // Highlight insertions and deletions
                    is_selected: dl.is_insert || dl.is_delete,
                });
            }
        }

        self.app.undotree_state.set_rendered_lines(lines);
        tracing::debug!("Refreshed undotree panel display");
    }

    /// Preview the diff of the currently selected undotree node.
    ///
    /// Extracts edits from the selected node and formats them as diff output,
    /// displaying them in the undotree panel below the tree visualization.
    fn undotree_preview_diff(&mut self) {
        use {
            super::app::DiffPreviewLine,
            reovim_module_undotree::{DiffLineType, format_edits_as_diff},
        };

        if !self.app.undotree_state.is_open() {
            return;
        }

        let Some(buffer_id) = self.app.undotree_state.source_buffer_id() else {
            return;
        };

        let Some(tree) = self.app.undo_registry.get_tree(buffer_id) else {
            return;
        };

        let selected_node = self.app.undotree_state.selected_node();
        let Some(node) = tree.node(selected_node) else {
            return;
        };

        // Format edits as diff
        let edits = node.edits();
        let diff_lines = format_edits_as_diff(edits);

        // Convert to preview lines
        let preview_lines: Vec<DiffPreviewLine> = diff_lines
            .into_iter()
            .map(|dl| DiffPreviewLine {
                text: dl.text,
                is_insert: dl.line_type == DiffLineType::Insert,
                is_delete: dl.line_type == DiffLineType::Delete,
                is_header: dl.line_type == DiffLineType::Header,
            })
            .collect();

        self.app
            .undotree_state
            .set_preview(selected_node, preview_lines);

        tracing::debug!(node = selected_node, edit_count = edits.len(), "Showing diff preview");

        // Refresh display to show preview
        self.refresh_undotree_panel();
        self.last_error = None;
    }

    /// Calculate the path from root to a target node.
    ///
    /// Returns a list of (`node_index`, `branch_index`) pairs representing
    /// the path from root to target. The `branch_index` indicates which
    /// child to follow at each step.
    fn calculate_path_to_node(
        tree: &reovim_kernel::api::v1::UndoTree,
        target: usize,
    ) -> Vec<(usize, usize)> {
        // Build path from target back to root
        let mut path = Vec::new();
        let mut current = target;

        while let Some(node) = tree.node(current) {
            if let Some(parent) = node.parent() {
                // Find which branch index leads to current from parent
                if let Some(parent_node) = tree.node(parent) {
                    let branch_idx = parent_node
                        .children()
                        .iter()
                        .position(|&c| c == current)
                        .unwrap_or(0);
                    path.push((current, branch_idx));
                }
                current = parent;
            } else {
                // At root
                path.push((current, 0));
                break;
            }
        }

        // Reverse to get path from root to target
        path.reverse();
        path
    }

    /// Execute search with pattern and direction.
    fn execute_search(&mut self, pattern: &str, direction: crate::server::app::SearchDirection) {
        use crate::search::{Direction, SearchEngine};

        let Some(buffer_id) = self.app.active_buffer else {
            self.set_error("No active buffer");
            return;
        };

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            self.set_error("Buffer not found");
            return;
        };

        let search_dir = match direction {
            crate::server::app::SearchDirection::Forward => Direction::Forward,
            crate::server::app::SearchDirection::Backward => Direction::Backward,
        };

        let buffer = buffer_arc.read();
        let cursor_pos = buffer.cursor().position;

        match SearchEngine::find_next(&buffer, cursor_pos, pattern, search_dir, true) {
            Ok(Some(m)) => {
                let wrapped = match search_dir {
                    Direction::Forward => m.start < cursor_pos,
                    Direction::Backward => m.start > cursor_pos,
                };
                drop(buffer);
                buffer_arc.write().set_position(m.start);
                self.app.search.highlight_active = true;
                self.last_error = None;

                if wrapped {
                    let msg = match search_dir {
                        Direction::Forward => "search hit BOTTOM, continuing at TOP",
                        Direction::Backward => "search hit TOP, continuing at BOTTOM",
                    };
                    tracing::info!(msg);
                }
            }
            Ok(None) => {
                self.set_error("Pattern not found");
            }
            Err(e) => {
                self.set_error(e.to_string());
            }
        }
    }

    /// Execute word search (* or #).
    fn execute_word_search(&mut self, direction: reovim_driver_command::SearchDirection) {
        use crate::{search::SearchEngine, server::app::SearchDirection};

        let Some(buffer_id) = self.app.active_buffer else {
            self.set_error("No active buffer");
            return;
        };

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            self.set_error("Buffer not found");
            return;
        };

        let buffer = buffer_arc.read();
        let cursor_pos = buffer.cursor().position;

        let Some(word_pattern) = SearchEngine::word_at_cursor(&buffer, cursor_pos) else {
            self.set_error("No word under cursor");
            return;
        };
        drop(buffer);

        // Store pattern and direction
        self.app.search.pattern = Some(word_pattern.clone());
        self.app.search.direction = match direction {
            reovim_driver_command::SearchDirection::Forward => SearchDirection::Forward,
            reovim_driver_command::SearchDirection::Backward => SearchDirection::Backward,
        };

        // Execute search
        self.execute_search(&word_pattern, self.app.search.direction);
    }

    /// Execute a repeat find motion (; or ,).
    fn execute_repeat_find(&mut self, reverse: bool) {
        let Some(last_find) = self.app.last_find else {
            // No previous find to repeat
            return;
        };

        let Some(buffer_id) = self.app.active_buffer else {
            self.set_error("No active buffer");
            return;
        };

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            self.set_error("Buffer not found");
            return;
        };

        // Get motion (same or reversed direction)
        let motion = if reverse {
            last_find.reverse_motion()
        } else {
            last_find.repeat_motion()
        };

        // Calculate and apply motion
        let buffer = buffer_arc.read();
        let target = MotionEngine::calculate(&buffer, buffer.cursor(), motion, 1);
        drop(buffer);

        if let Some(pos) = target {
            buffer_arc.write().set_position(pos);
        }
        // Note: ; and , do NOT update last_find (per Vim behavior)

        self.app.clear_pending_keys();
    }

    /// Handle an undo/redo action by applying edits from the undo registry.
    fn handle_undo_action(&mut self, action: UndoAction) {
        let Some(buffer_id) = self.app.active_buffer else {
            self.set_error("No active buffer");
            return;
        };

        match action {
            UndoAction::Undo { count } => {
                for _ in 0..count {
                    if let Some(result) = self.app.undo_registry.undo(buffer_id) {
                        self.apply_undo_result(buffer_id, result);
                    } else {
                        self.set_error("Already at oldest change");
                        break;
                    }
                }
            }
            UndoAction::Redo { count } => {
                for _ in 0..count {
                    if let Some(result) = self.app.undo_registry.redo(buffer_id) {
                        self.apply_undo_result(buffer_id, result);
                    } else {
                        self.set_error("Already at newest change");
                        break;
                    }
                }
            }
        }
    }

    /// Apply an undo/redo result to a buffer.
    fn apply_undo_result(&self, buffer_id: reovim_kernel::api::v1::BufferId, result: UndoResult) {
        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            return;
        };

        let mut buffer = buffer_arc.write();
        for edit in result.edits {
            match edit {
                Edit::Insert { position, text } => {
                    buffer.insert_at(position, &text);
                }
                Edit::Delete { position, text } => {
                    buffer.delete_at(position, text.chars().count());
                }
            }
        }
        buffer.set_position(result.cursor);
    }

    /// Read next key event.
    #[allow(clippy::unnecessary_wraps)]
    fn read_key(&mut self) -> Result<Option<KeyEvent>, EventLoopError> {
        // Will return errors when real terminal input is implemented
        Ok(self.key_reader.as_mut().and_then(|reader| reader()))
    }

    /// Render the display if needed.
    #[allow(
        clippy::unnecessary_wraps,
        clippy::unused_self,
        clippy::missing_const_for_fn
    )]
    fn render_if_needed(&self) -> Result<(), EventLoopError> {
        // TODO: Actual rendering - will use self and return errors then
        Ok(())
    }

    /// Set an error message.
    fn set_error(&mut self, msg: impl Into<String>) {
        self.last_error = Some(msg.into());
    }

    /// Get the last error message.
    #[must_use]
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Get a reference to the application state.
    #[must_use]
    pub const fn app(&self) -> &AppState {
        &self.app
    }

    /// Get a mutable reference to the application state.
    #[allow(clippy::missing_const_for_fn)] // &mut self can't be const in stable Rust
    pub fn app_mut(&mut self) -> &mut AppState {
        &mut self.app
    }

    /// Get a reference to the mode registry.
    #[must_use]
    pub const fn mode_registry(&self) -> &ModeRegistry {
        &self.mode_registry
    }

    /// Get a mutable reference to the mode registry.
    #[allow(clippy::missing_const_for_fn)] // &mut self can't be const in stable Rust
    pub fn mode_registry_mut(&mut self) -> &mut ModeRegistry {
        &mut self.mode_registry
    }

    /// Get a reference to the command registry.
    #[must_use]
    pub const fn command_registry(&self) -> &CommandRegistry {
        &self.command_registry
    }

    /// Get a mutable reference to the command registry.
    #[allow(clippy::missing_const_for_fn)] // &mut self can't be const in stable Rust
    pub fn command_registry_mut(&mut self) -> &mut CommandRegistry {
        &mut self.command_registry
    }

    /// Get a reference to the keymap registry.
    #[must_use]
    pub const fn keymap_registry(&self) -> &KeymapRegistry {
        &self.keymap_registry
    }

    /// Get a mutable reference to the keymap registry.
    #[allow(clippy::missing_const_for_fn)] // &mut self can't be const in stable Rust
    pub fn keymap_registry_mut(&mut self) -> &mut KeymapRegistry {
        &mut self.keymap_registry
    }

    // ========================================================================
    // Visual Mode Selection Helpers
    // ========================================================================

    /// Save the current visual selection if one is active.
    ///
    /// Called before exit-visual commands to preserve the selection for `gv`.
    fn save_visual_selection_if_active(&mut self) {
        let Some(buffer_id) = self.app.active_buffer else {
            return;
        };

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            return;
        };

        let buffer = buffer_arc.read();
        let selection = buffer.selection();

        // Only save if selection is active
        if !selection.is_active() {
            return;
        }

        // Capture the current mode so we can restore it on gv
        let current_mode = self.app.current_mode().clone();

        let last_selection = LastVisualSelection::new(
            buffer_id,
            selection.anchor,
            buffer.position(),
            selection.mode(),
            current_mode,
        );
        drop(buffer);

        self.app.save_visual_selection(last_selection);
    }

    /// Handle the reselect-last (gv) command.
    ///
    /// Restores the saved visual selection and enters the appropriate visual mode.
    fn handle_reselect_last(&mut self) {
        let Some(last_selection) = self.app.last_visual_selection() else {
            // No saved selection - nothing to do
            return;
        };

        // Clone values to avoid borrow issues
        let buffer_id = last_selection.buffer_id;
        let anchor = last_selection.anchor;
        let cursor = last_selection.cursor;
        let mode = last_selection.mode;
        let mode_id = last_selection.mode_id.clone();

        // Verify the buffer still exists and is the active buffer
        // (In Vim, gv only works if you're in the same buffer)
        if self.app.active_buffer != Some(buffer_id) {
            // Different buffer - switch to it first (or ignore)
            // For now, we'll only restore in the same buffer
            return;
        }

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            return;
        };

        // Restore the selection
        {
            let mut buffer = buffer_arc.write();
            buffer.selection_mut().start(anchor, mode);
            buffer.set_position(cursor);
        }

        // Transition to the stored visual mode
        // Using the stored mode_id instead of deriving from SelectionMode
        // removes hardcoded "editor" module assumption
        self.app.mode_stack.set(mode_id);
    }

    /// Handle a block insert action from visual-block I/A commands.
    ///
    /// This initiates block insert mode:
    /// 1. Stores block bounds in `AppState`
    /// 2. Enters insert mode
    /// 3. Starts insert text accumulation
    ///
    /// When insert mode exits (via Escape, Ctrl-C, etc.), the accumulated text
    /// will be applied to all lines in the block.
    fn handle_block_insert_action(&mut self, action: reovim_driver_command::BlockInsertAction) {
        use {
            reovim_driver_command::BlockInsertAction,
            reovim_kernel::api::v1::{ModeId, ModuleId},
        };

        let (start_line, end_line, column, is_append) = match action {
            BlockInsertAction::InsertStart {
                start_line,
                end_line,
                column,
            } => (start_line, end_line, column, false),
            BlockInsertAction::InsertEnd {
                start_line,
                end_line,
                column,
            } => (start_line, end_line, column, true),
        };

        // Store block insert state
        self.app
            .block_insert
            .start(start_line, end_line, column, is_append);

        // Enter insert mode (construct ModeId directly since runner doesn't depend on editor module)
        let insert_mode_id = ModeId::new(ModuleId::new("editor"), "insert");
        self.app.mode_stack.set(insert_mode_id);

        // Start accumulating insert text for later application
        self.app.repeat_state.start_accumulating();

        tracing::debug!(start_line, end_line, column, is_append, "Block insert mode started");
    }

    /// Apply accumulated block insert text to all lines in the block.
    ///
    /// Called when exiting insert mode while block insert is active.
    /// Applies the text typed on the first line to all lines in the block selection.
    fn apply_block_insert_text(&mut self) {
        // Get block insert state
        let block_insert = &self.app.block_insert;
        if !block_insert.is_active() {
            return;
        }

        let start_line = block_insert.start_line;
        let end_line = block_insert.end_line;
        let column = block_insert.column;

        // Get accumulated insert text
        let insert_text = self.app.repeat_state.insert_text.clone();
        if insert_text.is_empty() {
            // No text to apply
            self.app.block_insert.clear();
            self.app.repeat_state.stop_accumulating();
            return;
        }

        // Get active buffer
        let Some(buffer_id) = self.app.active_buffer else {
            self.app.block_insert.clear();
            self.app.repeat_state.stop_accumulating();
            return;
        };

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            self.app.block_insert.clear();
            self.app.repeat_state.stop_accumulating();
            return;
        };

        // Apply text to lines 2..=N (first line already has the text from typing)
        // Text was already inserted on start_line, so we apply to start_line+1..=end_line
        let mut buffer = buffer_arc.write();
        let lines_count = buffer.lines().len();

        for line_num in (start_line + 1)..=end_line {
            if line_num >= lines_count {
                break;
            }

            let line = buffer.lines().get(line_num).map(String::from);
            if let Some(line) = line {
                // Calculate insert position - clamp to line length
                // (for both insert and append, column is the target position)
                let insert_col = column.min(line.len());

                let pos = Position::new(line_num, insert_col);
                buffer.insert_at(pos, &insert_text);
            }
        }
        drop(buffer);

        tracing::debug!(
            start_line,
            end_line,
            column,
            text_len = insert_text.len(),
            "Applied block insert text to {} lines",
            end_line - start_line
        );

        // Clear block insert state
        self.app.block_insert.clear();
        self.app.repeat_state.stop_accumulating();
    }
}

impl<F: InputFallbackHandler<AppState>> std::fmt::Debug for EventLoop<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventLoop")
            .field("app", &self.app)
            .field("mode_registry", &self.mode_registry)
            .field("command_registry", &self.command_registry)
            .field("keymap_registry", &self.keymap_registry)
            .field("last_error", &self.last_error)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::NoOpFallback,
        reovim_driver_command::{ArgSpec, Command, CommandHandler},
        reovim_driver_input::KeyCode,
        reovim_kernel::api::v1::{CommandId, KernelContext, ModeId, ModuleId},
        std::sync::Arc,
    };

    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    fn test_command_id(name: &'static str) -> CommandId {
        CommandId::new(ModuleId::new("test"), name)
    }

    fn create_test_event_loop() -> EventLoop<NoOpFallback> {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode());
        EventLoop::new(
            app,
            ModeRegistry::new(),
            CommandRegistry::new(),
            KeymapRegistry::new(),
            NoOpFallback,
        )
    }

    // Test command that sets a flag
    struct TestCommand {
        id: CommandId,
    }

    impl Command for TestCommand {
        fn id(&self) -> CommandId {
            self.id.clone()
        }
        fn description(&self) -> &'static str {
            "Test command"
        }
        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
    }

    impl CommandHandler for TestCommand {
        fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
            CommandResult::Success
        }
    }

    // Test command that quits
    struct QuitCommand;

    impl Command for QuitCommand {
        fn id(&self) -> CommandId {
            test_command_id("quit")
        }
        fn description(&self) -> &'static str {
            "Quit"
        }
        fn args(&self) -> Vec<ArgSpec> {
            vec![]
        }
    }

    impl CommandHandler for QuitCommand {
        fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
            CommandResult::Quit
        }
    }

    #[test]
    fn test_event_loop_new() {
        let event_loop = create_test_event_loop();
        assert!(event_loop.app().is_running());
        assert!(event_loop.last_error().is_none());
    }

    #[test]
    fn test_event_loop_run_no_keys() {
        let mut event_loop = create_test_event_loop();
        // With no key reader, run() should exit immediately
        let result = event_loop.run();
        assert!(result.is_ok());
    }

    #[test]
    fn test_event_loop_with_key_reader() {
        let mut keys = vec![
            KeyEvent::new(KeyCode::Char('j')),
            KeyEvent::new(KeyCode::Char('k')),
        ]
        .into_iter();

        let mut event_loop = create_test_event_loop().with_key_reader(move || keys.next());

        // Step through keys
        assert!(event_loop.step().unwrap()); // 'j'
        assert!(event_loop.step().unwrap()); // 'k'
        assert!(!event_loop.step().unwrap()); // no more keys
    }

    #[test]
    fn test_event_loop_command_execution() {
        let mut event_loop = create_test_event_loop();

        // Register command
        let cmd = TestCommand {
            id: test_command_id("test-cmd"),
        };
        event_loop.command_registry_mut().register(Arc::new(cmd));

        // Register keybinding
        event_loop.keymap_registry_mut().register_str(
            test_mode(),
            "j",
            test_command_id("test-cmd"),
        );

        // Set up key reader
        let mut keys = vec![KeyEvent::new(KeyCode::Char('j'))].into_iter();
        event_loop.key_reader = Some(Box::new(move || keys.next()));

        // Process key
        assert!(event_loop.step().unwrap());

        // No error should be set
        assert!(event_loop.last_error().is_none());
    }

    #[test]
    fn test_event_loop_quit_command() {
        let mut event_loop = create_test_event_loop();

        // Register quit command
        event_loop
            .command_registry_mut()
            .register(Arc::new(QuitCommand));

        // Register keybinding
        event_loop
            .keymap_registry_mut()
            .register_str(test_mode(), "q", test_command_id("quit"));

        // Set up key reader
        let mut keys = vec![KeyEvent::new(KeyCode::Char('q'))].into_iter();
        event_loop.key_reader = Some(Box::new(move || keys.next()));

        // Process key
        assert!(event_loop.step().unwrap());

        // App should be stopped
        assert!(!event_loop.app().is_running());
    }

    #[test]
    fn test_event_loop_unbound_key() {
        let mut event_loop = create_test_event_loop();

        // Set up key reader with unbound key
        let mut keys = vec![KeyEvent::new(KeyCode::Char('x'))].into_iter();
        event_loop.key_reader = Some(Box::new(move || keys.next()));

        // Process key
        assert!(event_loop.step().unwrap());

        // With NoOpFallback, no error should be set
        assert!(event_loop.last_error().is_none());
    }

    #[test]
    fn test_event_loop_prefix_sequence() {
        let mut event_loop = create_test_event_loop();

        // Register `gg` command
        let cmd = TestCommand {
            id: test_command_id("goto-top"),
        };
        event_loop.command_registry_mut().register(Arc::new(cmd));
        event_loop.keymap_registry_mut().register_str(
            test_mode(),
            "gg",
            test_command_id("goto-top"),
        );

        // First 'g' should be prefix
        let mut keys = vec![
            KeyEvent::new(KeyCode::Char('g')),
            KeyEvent::new(KeyCode::Char('g')),
        ]
        .into_iter();
        event_loop.key_reader = Some(Box::new(move || keys.next()));

        // Process first 'g' - should accumulate
        assert!(event_loop.step().unwrap());
        assert_eq!(event_loop.app().pending_keys.len(), 1);

        // Process second 'g' - should execute and clear
        assert!(event_loop.step().unwrap());
        assert!(event_loop.app().pending_keys.is_empty());
    }

    #[test]
    fn test_event_loop_error_handling() {
        let mut event_loop = create_test_event_loop();

        // Manually set error
        event_loop.set_error("Test error");
        assert_eq!(event_loop.last_error(), Some("Test error"));

        // Clear error by successful command
        event_loop.handle_command_result(CommandResult::Success);
        assert!(event_loop.last_error().is_none());
    }

    #[test]
    fn test_event_loop_error_command() {
        let mut event_loop = create_test_event_loop();

        event_loop.handle_command_result(CommandResult::Error("Command failed".into()));
        assert_eq!(event_loop.last_error(), Some("Command failed"));
    }

    #[test]
    fn test_event_loop_accessors() {
        let mut event_loop = create_test_event_loop();

        // Test all accessor methods compile and work
        let _ = event_loop.app();
        let _ = event_loop.app_mut();
        let _ = event_loop.mode_registry();
        let _ = event_loop.mode_registry_mut();
        let _ = event_loop.command_registry();
        let _ = event_loop.command_registry_mut();
        let _ = event_loop.keymap_registry();
        let _ = event_loop.keymap_registry_mut();
    }

    // =========================================================================
    // Mode Transition Tests
    // =========================================================================

    // Note: mode_for_command tests and helpers removed as part of Epic #284.
    // Mode transitions are now event-driven via ModeChanged events with target_mode.
}
