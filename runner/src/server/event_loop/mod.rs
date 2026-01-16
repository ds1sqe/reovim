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
mod search_handler;
mod undotree_handler;
mod visual_handler;
mod window_handler;

pub use error::EventLoopError;

use {
    reovim_driver_command::{CommandContext, CommandResult, UndoAction},
    reovim_driver_input::{FallbackResult, InputFallbackHandler, KeyEvent},
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
                    if let Ok(mut guard) = self.pending_mode_change.lock()
                        && let Some(new_mode) = guard.take()
                    {
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
                // Visual-block insert (I/A in visual-block mode) wants to insert
                // text across multiple lines. Full implementation TBD.
                tracing::debug!(?action, "BlockInsertAction requested");
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
