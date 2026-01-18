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

mod char_handler;
mod error;
mod operator_handler;
mod search_handler;
mod undotree_handler;
mod visual_handler;
mod window_handler;

pub use error::EventLoopError;

use {
    reovim_driver_command::{CommandContext, CommandResult, UndoAction},
    reovim_driver_input::{
        FallbackResult, InputFallbackHandler, KeyEvent, ModeState, ModeTransition, PopResult,
        ResolveResult,
    },
    reovim_kernel::{
        api::v1::{EventResult, ModeId, events::ModeChanged},
        profile_scope,
    },
    std::sync::{Arc, Mutex},
};

use super::{
    AppState,
    app::{FindType, PendingCharOp},
    registry::{CommandRegistry, KeyLookupResult, KeymapRegistry, ModeRegistry},
};

use reovim_module_editor::ResolverRegistry;

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
    pub(super) app: AppState,

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
    pub(super) last_error: Option<String>,

    /// Pending mode change from `ModeChanged` events.
    ///
    /// Commands emit `ModeChanged` events with `target_mode`. The event handler
    /// stores the target here, and we apply it after command execution.
    /// This enables event-driven mode transitions without hardcoded command names.
    pending_mode_change: Arc<Mutex<Option<ModeId>>>,

    /// Registry for mode key resolvers.
    ///
    /// When set, the event loop will use resolvers to handle key input
    /// instead of the traditional keymap lookup. This enables the flexible
    /// mode system where modules can define their own key handling policy.
    resolver_registry: Option<ResolverRegistry>,
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
            resolver_registry: None,
        }
    }

    /// Set a resolver registry for mode-specific key handling.
    ///
    /// When set, the event loop will use resolvers to handle key input
    /// for modes that have registered resolvers, enabling the flexible
    /// mode system.
    #[must_use]
    pub fn with_resolver_registry(mut self, registry: ResolverRegistry) -> Self {
        self.resolver_registry = Some(registry);
        self
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
    /// 1. Check for count prefix (digits in normal mode)
    /// 2. Add key to pending sequence
    /// 3. Look up in keymap
    /// 4. If found: execute command
    /// 5. If prefix: wait for more keys
    /// 6. If not found: delegate to fallback handler
    #[allow(clippy::too_many_lines)] // Key dispatch coordination - splitting would fragment logic
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

        // Try resolver-based key handling if resolver registry is configured.
        // This enables the flexible mode system where modules define key handling policy.
        //
        // When a resolver is available and handles the mode:
        // - Execute/Pending/InsertChar/ModeTransition -> handled, return
        // - NotHandled -> call fallback handler only (no legacy keymap lookup)
        //
        // Legacy keymap lookup only runs when NO resolver is registered for the mode.
        if let Some(result) = self.try_resolver(&key) {
            match result {
                ResolveResult::NotHandled => {
                    // Resolver says key is not bound - call fallback handler only.
                    // The resolver has already queried the keymap, so we don't do it again.
                    self.fallback_handler.handle_unmatched(key, &mut self.app);
                    self.app.clear_pending_keys();
                    return;
                }
                _ => {
                    // All other results are handled by handle_resolve_result
                    if self.handle_resolve_result(result, key) {
                        return;
                    }
                }
            }
        }
        // Fall through to traditional keymap lookup only if no resolver for this mode

        // --- Legacy keymap-based handling ---
        // The following code handles keys when no resolver is configured for the
        // current mode. Modes with resolvers use the resolver path above exclusively.
        //
        // Note: Count prefix and register prefix handling has been moved to the
        // Vim policy module (Epic #372). The runner is now truly policy-agnostic.

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

                // Build command context.
                // Note: Count and register prefixes are handled by the Vim resolver
                // (Epic #372 - policy-agnostic runner).
                let mut ctx = CommandContext::new();

                // Set buffer_id in context if we have an active buffer
                if let Some(buffer_id) = self.app.active_buffer {
                    ctx.set_buffer_id(buffer_id);
                }

                // Set current mode name in context for commands that need to
                // adjust behavior based on mode (e.g., motions in operator-pending)
                ctx.set_mode_name(mode.name());

                // Save visual selection if currently in visual mode (for gv command).
                // We check the current mode rather than hardcoding command names that exit visual.
                // Any command that exits visual mode will have the selection saved before execution.
                let in_visual_mode = self.mode_registry.has_selection(self.app.current_mode());
                if in_visual_mode {
                    self.save_visual_selection_if_active();
                }

                // Track current mode before command for insert mode detection
                let mode_before = self.app.current_mode().clone();

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
                    //
                    // Extract mode change from mutex before processing to avoid borrow issues
                    let new_mode = self
                        .pending_mode_change
                        .lock()
                        .ok()
                        .and_then(|mut guard| guard.take());

                    if let Some(new_mode) = new_mode {
                        // Check if entering input mode - start accumulation
                        // Uses capability-based query instead of string comparison (Epic #372)
                        if self.mode_registry.accepts_char_input(&new_mode)
                            && !self.mode_registry.accepts_char_input(&mode_before)
                        {
                            use crate::server::app::InsertEntryType;

                            // Determine entry type based on command
                            let entry_type = if cmd_id.name() == "open-line-below" {
                                InsertEntryType::OpenBelow
                            } else if cmd_id.name() == "open-line-above" {
                                InsertEntryType::OpenAbove
                            } else {
                                InsertEntryType::Inline
                            };

                            self.app
                                .repeat_state
                                .start_accumulating_with_count_and_type(1, entry_type);
                        }
                        // Check if exiting input mode - handle text repetition
                        // Uses capability-based query instead of string comparison (Epic #372)
                        else if self.mode_registry.accepts_char_input(&mode_before)
                            && !self.mode_registry.accepts_char_input(&new_mode)
                        {
                            self.handle_insert_mode_exit();
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

    /// Handle command execution result.
    #[allow(clippy::too_many_lines)]
    fn handle_command_result(&mut self, result: CommandResult) {
        use reovim_driver_command::CharWaitOp;

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
            CommandResult::Detach => {
                // Detach requests client disconnection but server continues.
                // The broadcaster will send DETACH notification to all clients.
                self.app.request_detach();
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
                // Motion/text-object returned a range for the pending operator.
                // Execute the pending operator (d, y, c) with this range.
                tracing::debug!(?start, ?end, is_linewise, "OperatorRange received");

                // Execute the pending operator
                self.execute_pending_operator(start, end, is_linewise);
                self.last_error = None;
            }
            CommandResult::ReselectVisual => {
                // The reselect-last (gv) command requests restoration of the
                // last visual selection. Handle it here instead of checking
                // the command name.
                self.handle_reselect_last();
            }
            CommandResult::BlockInsertAction(action) => {
                // Visual-block insert (I/A in visual-block mode) wants to insert
                // text across multiple lines. Full implementation TBD.
                tracing::debug!(?action, "BlockInsertAction requested");
                self.last_error = None;
            }
            CommandResult::EnterOperatorPending {
                operator_id,
                register,
            } => {
                // Enter-operator commands (d, y, c) request operator-pending mode.
                // Set pending operator state and push operator-pending mode.
                //
                // Note: Count is handled by the Vim resolver (Epic #372).
                // The legacy keymap path uses default count of 1.
                use crate::server::app::PendingOperator;

                let pending = PendingOperator::new(operator_id)
                    .with_count(1)
                    .with_register(register);
                self.app.set_pending_operator(pending);

                // Push operator-pending mode onto the stack
                let mode_id = ModeId::new(
                    reovim_kernel::api::v1::ModuleId::new("editor"),
                    "operator-pending",
                );
                self.app.mode_stack.push(mode_id);

                tracing::debug!(operator_id, ?register, "Entered operator-pending mode");
                self.last_error = None;
            }
        }
    }

    /// Handle a mode action from mode-changing commands.
    fn handle_mode_action(&mut self, action: reovim_driver_command::ModeAction) {
        use reovim_driver_command::ModeAction;

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
                // If there's a pending operator, clear it.
                // This is policy-agnostic - we check state, not mode names (Epic #372).
                let has_pending_operator = self.app.pending_operator().is_some();

                if let Some(popped) = self.app.mode_stack.pop() {
                    tracing::info!(mode = %popped, "Popped mode from stack");
                }

                // Clear pending operator state if present
                if has_pending_operator {
                    self.app.take_pending_operator();
                    tracing::debug!("Cleared pending operator state on mode pop");
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

    /// Try to resolve a key event using the resolver registry.
    ///
    /// Returns `Some(result)` if a resolver handled the key, `None` if:
    /// - No resolver registry is configured
    /// - No resolver is registered for the current mode
    ///
    /// # Mechanism vs Policy (Epic #353)
    ///
    /// This method provides resolvers with access to the keymap registry via
    /// `resolve_with_keymap`. Resolvers can query the keymap to get FACTS about
    /// what bindings exist, then apply their own POLICY to decide what to do.
    fn try_resolver(&self, key: &KeyEvent) -> Option<ResolveResult> {
        use reovim_kernel::api::v1::{CommandId, ModuleId};

        let registry = self.resolver_registry.as_ref()?;
        let mode = self.app.current_mode();
        let mut mode_state = ModeState::new(mode.clone());

        // Copy transition context if in operator-pending mode
        if let Some(pending) = self.app.pending_operator() {
            // Convert operator_id string to CommandId
            let operator_cmd = CommandId::new(ModuleId::new("editor"), pending.operator_id);
            let ctx = reovim_driver_input::TransitionContext::with_operator(operator_cmd)
                .count(pending.count)
                .register(pending.register.unwrap_or('\0'));
            mode_state.transition_context = Some(ctx);
        }

        // Use resolve_with_keymap to give resolvers access to keymap queries
        registry.resolve_with_keymap(mode, key, &mut mode_state, &self.keymap_registry)
    }

    /// Handle a resolve result from a mode key resolver.
    ///
    /// Returns `true` if the key was handled, `false` if it should fall through
    /// to the traditional keymap lookup.
    fn handle_resolve_result(&mut self, result: ResolveResult, _key: KeyEvent) -> bool {
        match result {
            ResolveResult::Execute(cmd_id, ctx) => {
                // Build command context from resolver context
                let mut cmd_ctx = CommandContext::new();

                if let Some(count) = ctx.count {
                    cmd_ctx.set("count", reovim_driver_command::ArgValue::Count(count));
                }

                if let Some(reg) = ctx.register {
                    cmd_ctx.set("register", reovim_driver_command::ArgValue::Register(reg));
                }

                if let Some(buffer_id) = self.app.active_buffer {
                    cmd_ctx.set_buffer_id(buffer_id);
                }

                let mode = self.app.current_mode();
                cmd_ctx.set_mode_name(mode.name());

                // Execute the command
                if let Some(result) =
                    self.command_registry
                        .execute(&cmd_id, &mut self.app, &cmd_ctx)
                {
                    self.handle_command_result(result);
                }

                true
            }

            ResolveResult::Pending => {
                // Key is part of a pending sequence, wait for more keys
                true
            }

            ResolveResult::InsertChar(ch) => {
                // Insert the character directly
                self.fallback_handler.handle_unmatched(
                    KeyEvent::new(reovim_driver_input::KeyCode::Char(ch)),
                    &mut self.app,
                );
                true
            }

            ResolveResult::ModeTransition(transition) => {
                self.handle_mode_transition(transition);
                true
            }

            ResolveResult::NotHandled => {
                // Fall through to traditional keymap lookup
                false
            }
        }
    }

    /// Handle a mode transition from a resolver.
    fn handle_mode_transition(&mut self, transition: ModeTransition) {
        match transition {
            ModeTransition::Push { mode, context } => {
                // Save context for the new mode (e.g., pending operator)
                if let Some(op) = context.pending_operator {
                    use crate::server::app::PendingOperator;

                    // Convert CommandId name to static str for PendingOperator
                    let op_name: &'static str = Box::leak(op.name().to_string().into_boxed_str());
                    let pending = PendingOperator::new(op_name)
                        .with_count(context.count.unwrap_or(1))
                        .with_register(context.register);
                    self.app.set_pending_operator(pending);
                }

                self.app.mode_stack.push(mode);
            }

            ModeTransition::Pop { result } => {
                // Handle the pop result
                if let Some(ref pop_result) = result {
                    self.handle_pop_result(pop_result);
                }

                // Pop the mode stack
                self.app.mode_stack.pop();

                // Clear pending operator if popping from operator-pending
                if self.app.pending_operator().is_some() {
                    self.app.take_pending_operator();
                }
            }

            ModeTransition::Set { mode, context: _ } => {
                self.app.mode_stack.set(mode);
            }
        }
    }

    /// Handle a pop result from operator-pending mode.
    fn handle_pop_result(&mut self, result: &PopResult) {
        use reovim_kernel::api::v1::{CommandId, ModuleId};

        match result {
            PopResult::Cancelled => {
                // User cancelled (Escape), just clear pending state
                self.app.take_pending_operator();
            }

            PopResult::OperatorRange {
                start: _,
                end: _,
                linewise,
            } => {
                // Execute the pending operator with the range
                if let Some(pending) = self.app.take_pending_operator()
                    && *linewise
                {
                    // Line-wise operation (dd, yy, cc)
                    // The operator should execute on the current line
                    let mut ctx = CommandContext::new();
                    ctx.set("linewise", reovim_driver_command::ArgValue::Bang(true));
                    ctx.set("count", reovim_driver_command::ArgValue::Count(pending.count));

                    if let Some(reg) = pending.register {
                        ctx.set("register", reovim_driver_command::ArgValue::Register(reg));
                    }

                    if let Some(buffer_id) = self.app.active_buffer {
                        ctx.set_buffer_id(buffer_id);
                    }

                    // Convert operator_id to CommandId for execution
                    let cmd_id = CommandId::new(ModuleId::new("operators"), pending.operator_id);

                    // Execute the operator command with linewise flag
                    if let Some(result) =
                        self.command_registry.execute(&cmd_id, &mut self.app, &ctx)
                    {
                        self.handle_command_result(result);
                    }
                }
            }

            // Other pop results can be handled as needed
            _ => {}
        }
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
    pub(super) fn set_error(&mut self, msg: impl Into<String>) {
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
    // Insert Mode Exit Handling
    // ========================================================================

    /// Handle insert mode exit (repeat accumulated text if count > 1).
    ///
    /// Behavior depends on how insert mode was entered:
    /// - `Inline` (i/a/I/A): "3ihello<Esc>" inserts "hellohellohello"
    /// - `OpenBelow` (o): "3ohello<Esc>" creates 3 lines each with "hello"
    /// - `OpenAbove` (O): "3Ohello<Esc>" creates 3 lines each with "hello"
    fn handle_insert_mode_exit(&mut self) {
        use crate::server::app::InsertEntryType;

        // Stop accumulating and get the count and entry type
        self.app.repeat_state.stop_accumulating();
        let count = self.app.repeat_state.get_insert_count();
        let entry_type = self.app.repeat_state.get_insert_entry_type();

        // If count > 1, repeat the accumulated text
        if count > 1 {
            let text = self.app.repeat_state.insert_text.clone();
            if !text.is_empty() {
                // Get the active buffer
                if let Some(buffer_id) = self.app.active_buffer
                    && let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id)
                {
                    let mut buffer = buffer_arc.write();
                    let cursor_before = buffer.position();

                    // Build the repeat text based on entry type
                    let repeat_text = match entry_type {
                        InsertEntryType::Inline => {
                            // Repeat text inline (e.g., "3itest" -> "testtesttest")
                            text.repeat(count - 1)
                        }
                        InsertEntryType::OpenBelow | InsertEntryType::OpenAbove => {
                            // Repeat text on new lines (e.g., "3otest" -> 3 lines with "test")
                            // Format: newline + text, repeated (count-1) times
                            format!("\n{text}").repeat(count - 1)
                        }
                    };

                    let edit = buffer.insert(&repeat_text);
                    let cursor_after = buffer.position();
                    drop(buffer);

                    // Record the edit for undo
                    self.app.undo_registry.record(
                        buffer_id,
                        vec![edit],
                        cursor_before,
                        cursor_after,
                    );
                }
            }
        }
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
        reovim_kernel::api::v1::{CommandId, KernelContext, Mode, ModeId, ModuleId},
        std::sync::Arc,
    };

    // Test mode implementation for mode registry tests
    const TEST_MODULE: ModuleId = ModuleId::new("test");

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(u16)]
    enum TestMode {
        Command = 0,
        Input = 1,
    }

    impl reovim_kernel::api::v1::Mode for TestMode {
        fn module() -> ModuleId {
            TEST_MODULE
        }

        fn discriminant(&self) -> u16 {
            *self as u16
        }

        fn display_name(&self) -> &'static str {
            match self {
                Self::Command => "COMMAND",
                Self::Input => "INPUT",
            }
        }

        fn cursor_style(&self) -> reovim_kernel::api::v1::CursorStyle {
            match self {
                Self::Command => reovim_kernel::api::v1::CursorStyle::Block,
                Self::Input => reovim_kernel::api::v1::CursorStyle::Bar,
            }
        }

        fn accepts_char_input(&self) -> bool {
            matches!(self, Self::Input)
        }

        fn has_selection(&self) -> bool {
            false
        }

        fn inherits_from(&self) -> Option<Self> {
            None
        }
    }

    fn test_mode() -> ModeId {
        TestMode::Command.id()
    }

    fn test_command_id(name: &'static str) -> CommandId {
        CommandId::new(ModuleId::new("test"), name)
    }

    fn create_test_event_loop() -> EventLoop<NoOpFallback> {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode());

        // Register test modes so capability queries work
        let mut mode_registry = ModeRegistry::new();
        mode_registry.register_mode(TestMode::Command);
        mode_registry.register_mode(TestMode::Input);

        EventLoop::new(
            app,
            mode_registry,
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
        let mode = test_mode();
        event_loop
            .keymap_registry_mut()
            .register_str(&mode, "j", test_command_id("test-cmd"));

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
        let mode = test_mode();
        event_loop
            .keymap_registry_mut()
            .register_str(&mode, "q", test_command_id("quit"));

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
        let mode = test_mode();
        event_loop
            .keymap_registry_mut()
            .register_str(&mode, "gg", test_command_id("goto-top"));

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

    // Note: Count prefix and register prefix tests removed as part of Epic #372.
    // Count and register handling is now the responsibility of policy modules (Vim).
}
