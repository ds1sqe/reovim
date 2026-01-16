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
    reovim_kernel::api::v1::{Edit, Motion, MotionEngine, UndoResult},
};

use super::{
    AppState,
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
        Self {
            app,
            mode_registry,
            command_registry,
            keymap_registry,
            fallback_handler,
            key_reader: None,
            last_error: None,
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
        // Check for char-wait state (f/F/t/T waiting for character)
        if self.app.char_wait.is_some() {
            self.handle_char_wait(key);
            return;
        }

        // Add to pending sequence
        self.app.pending_keys.push(key);

        // Get current mode
        let mode = self.app.current_mode().clone();

        // Look up in keymap
        match self.keymap_registry.lookup(&mode, &self.app.pending_keys) {
            KeyLookupResult::Found(cmd_id) => {
                // Build command context (TODO: parse count/register from pending keys)
                let ctx = CommandContext::new();

                // Execute command
                if let Some(result) = self.command_registry.execute(&cmd_id, &mut self.app, &ctx) {
                    self.handle_command_result(result);
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

    /// Handle a key event when in char-wait state (f/F/t/T pending).
    ///
    /// The key provides the character argument for the find-char motion.
    /// Escape cancels the wait without executing any motion.
    fn handle_char_wait(&mut self, key: KeyEvent) {
        use super::app::LastFind;

        // Take the char-wait state
        let char_wait = self.app.char_wait.take().expect("char_wait should be Some");

        // Handle escape - cancel char-wait
        if key.code == KeyCode::Escape {
            // Clear any pending keys and return without motion
            self.app.clear_pending_keys();
            return;
        }

        // Extract character from key event
        let KeyCode::Char(c) = key.code else {
            // Non-character keys cancel char-wait (like escape)
            self.app.clear_pending_keys();
            return;
        };

        // Build and execute the find-char motion
        let motion = Motion::FindChar {
            char: c,
            direction: char_wait.find_type.direction(),
            till: char_wait.find_type.is_till(),
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
            self.app.last_find = Some(LastFind::new(c, char_wait.find_type));
        }
        // Note: If target is None (char not found), cursor doesn't move,
        // and last_find is NOT updated (per Vim behavior)

        self.app.clear_pending_keys();
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
                // The undotree module provides the command infrastructure and
                // rendering logic. Full panel integration pending layout/window
                // system completion.
                //
                // Current state: Types and rendering are complete, panel
                // management deferred to runner enhancement (#249).
                tracing::info!(?action, "Undotree action requested");
                self.last_error = None;
            }
            CommandResult::WaitingForChar(ctx) => {
                // Find-char command (f/F/t/T) needs a character argument.
                // Set char-wait state so next key press completes the motion.
                use super::app::{CharWaitState, FindType};

                let find_type = match ctx.find_type {
                    reovim_driver_command::FindType::FindForward => FindType::FindForward,
                    reovim_driver_command::FindType::FindBackward => FindType::FindBackward,
                    reovim_driver_command::FindType::TillForward => FindType::TillForward,
                    reovim_driver_command::FindType::TillBackward => FindType::TillBackward,
                };

                self.app.char_wait = Some(CharWaitState::new(find_type, ctx.start_position));
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
        }
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
}
