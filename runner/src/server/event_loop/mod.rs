//! Main event loop for the runner.
//!
//! The event loop is the heart of the runner - it reads key events and
//! dispatches them to resolvers for handling.
//!
//! # Design Philosophy
//!
//! Following the "mechanism vs policy" principle:
//! - **Mechanism** (this module): Key dispatch, command execution
//! - **Policy** (modules): Resolvers decide what to do with keys
//!
//! The event loop contains NO business logic - resolvers handle everything.

mod error;
mod runtime;

pub use {error::EventLoopError, runtime::AppStateRuntime};

use reovim_driver_session::api::StateChanges;

use {
    reovim_driver_command::{CommandContext, CommandResult},
    reovim_driver_input::{
        ArgValue as InputArgValue, KeyEvent, ModeState, ModeTransition, PopResult, ResolveResult,
    },
    reovim_kernel::profile_scope,
};

use super::{
    AppState,
    registry::{CommandRegistry, KeymapRegistry, ModeRegistry},
};

use reovim_module_editor::ResolverRegistry;

/// Main event loop for the runner.
///
/// Super simple - just dispatches keys to resolvers.
/// No fallback handler, no vim-specific state. Pure mechanism.
pub struct EventLoop {
    /// Application state (kernel + runtime).
    pub(super) app: AppState,

    /// Registry for mode metadata.
    mode_registry: ModeRegistry,

    /// Registry for command handlers.
    command_registry: CommandRegistry,

    /// Registry for keybindings.
    keymap_registry: KeymapRegistry,

    /// Callback for key input (for testing/injection).
    key_reader: Option<Box<dyn FnMut() -> Option<KeyEvent> + Send>>,

    /// Last error message (for status line).
    pub(super) last_error: Option<String>,

    /// Registry for mode key resolvers.
    resolver_registry: Option<ResolverRegistry>,
}

impl EventLoop {
    /// Create a new event loop.
    #[must_use]
    pub fn new(
        app: AppState,
        mode_registry: ModeRegistry,
        command_registry: CommandRegistry,
        keymap_registry: KeymapRegistry,
    ) -> Self {
        Self {
            app,
            mode_registry,
            command_registry,
            keymap_registry,
            key_reader: None,
            last_error: None,
            resolver_registry: None,
        }
    }

    /// Set a resolver registry for mode-specific key handling.
    #[must_use]
    pub fn with_resolver_registry(mut self, registry: ResolverRegistry) -> Self {
        self.resolver_registry = Some(registry);
        self
    }

    /// Set a custom key reader for testing.
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
    /// Returns an error if input reading fails.
    pub fn run(&mut self) -> Result<(), EventLoopError> {
        while self.app.is_running() {
            let Some(key) = self.read_key()? else {
                break;
            };
            self.handle_key(key);
        }
        Ok(())
    }

    /// Run a single iteration (for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if input reading fails.
    pub fn step(&mut self) -> Result<bool, EventLoopError> {
        let Some(key) = self.read_key()? else {
            return Ok(false);
        };
        self.handle_key(key);
        Ok(true)
    }

    /// Handle a single key event.
    ///
    /// Super simple: resolver handles everything.
    /// `StateChanges` from session API are collected but not yet broadcast (Phase 4).
    fn handle_key(&mut self, key: KeyEvent) {
        profile_scope!("handle_key", "runner::event_loop");

        if let Some((result, _changes)) = self.try_resolver(&key) {
            self.handle_resolve_result(result);

            // TODO (Phase 4): Broadcast state changes to clients
            // if changes.has_changes() {
            //     self.broadcast_state_changes(&changes);
            // }
        }
        // No resolver = key ignored (resolver handles everything)
    }

    /// Handle command execution result.
    fn handle_command_result(&mut self, result: CommandResult) {
        match result {
            CommandResult::Success => {
                self.last_error = None;
            }
            CommandResult::Error(msg) => {
                self.set_error(msg);
            }
            CommandResult::Quit | CommandResult::ForceQuit => {
                self.app.request_quit();
            }
            CommandResult::Detach => {
                self.app.request_detach();
            }
        }
    }

    /// Try to resolve a key event using the resolver registry.
    ///
    /// Returns both the `ResolveResult` and accumulated `StateChanges` from
    /// the session API. The changes can be broadcast to clients.
    fn try_resolver(&mut self, key: &KeyEvent) -> Option<(ResolveResult, StateChanges)> {
        use reovim_driver_session::ChangeTracker;

        let registry = self.resolver_registry.as_ref()?;
        let mode = self.app.current_mode().clone();
        let mut mode_state = ModeState::new(mode.clone());

        // Create session runtime adapter from individual AppState fields.
        // Extensions are passed separately to work around borrow rules.
        let mut runtime = AppStateRuntime::new(
            &mut self.app.mode_stack,
            &mut self.app.windows,
            &mut self.app.active_buffer,
            &self.app.kernel,
            &self.command_registry,
        );

        // Resolvers access session state via SessionApiDyn + extensions
        // Extensions contain module-specific state (e.g., VimSessionState)
        let result = registry.resolve_with_session(
            &mode,
            key,
            &mut mode_state,
            &self.keymap_registry,
            &mut runtime,
            &mut self.app.extensions,
        );

        // Take accumulated changes from the runtime
        let changes = runtime.take_changes();

        result.map(|r| (r, changes))
    }

    /// Handle a resolve result from a mode key resolver.
    fn handle_resolve_result(&mut self, result: ResolveResult) {
        match result {
            ResolveResult::Execute(cmd_id, ctx) => {
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

                cmd_ctx.set_mode_name(self.app.current_mode().name());

                // Transfer metadata
                for (key, value) in ctx.metadata {
                    if let Some(cmd_value) = Self::convert_arg_value(&value) {
                        let static_key: &'static str = Box::leak(key.into_boxed_str());
                        cmd_ctx.set(static_key, cmd_value);
                    }
                }

                if let Some(result) =
                    self.command_registry
                        .execute(&cmd_id, &mut self.app, &cmd_ctx)
                {
                    self.handle_command_result(result);
                }
            }

            ResolveResult::ModeTransition(transition) => {
                self.handle_mode_transition(transition);
            }

            // Pending: wait for more keys
            // InsertChar: resolvers should return Execute with insert command
            // NotHandled: key not handled, ignore
            // Completed: resolver already did everything via SessionApi, changes are tracked
            ResolveResult::Pending
            | ResolveResult::InsertChar(_)
            | ResolveResult::NotHandled
            | ResolveResult::Completed => {}
        }
    }

    /// Handle a mode transition from a resolver.
    ///
    /// SSOT: Context flows through transitions, not stored in runner.
    fn handle_mode_transition(&mut self, transition: ModeTransition) {
        match transition {
            ModeTransition::Push { mode, context: _ } => {
                // Context is handled by resolver (stored in VimSessionState)
                self.app.mode_stack.push(mode);
            }

            ModeTransition::Pop { result } => {
                if let Some(ref pop_result) = result {
                    self.handle_pop_result(pop_result);
                }
                self.app.mode_stack.pop();
            }

            ModeTransition::Set { mode, context: _ } => {
                self.app.mode_stack.set(mode);
            }
        }
    }

    /// Handle a pop result from a mode.
    ///
    /// SSOT: All operator info comes from `PopResult`, not `AppState`.
    fn handle_pop_result(&mut self, result: &PopResult) {
        if let PopResult::OperatorRange {
            operator,
            linewise: true,
            count,
            register,
            ..
        } = result
        {
            let mut ctx = CommandContext::new();
            ctx.set("linewise", reovim_driver_command::ArgValue::Bang(true));
            ctx.set("count", reovim_driver_command::ArgValue::Count(count.unwrap_or(1)));

            if let Some(reg) = register {
                ctx.set("register", reovim_driver_command::ArgValue::Register(*reg));
            }

            if let Some(buffer_id) = self.app.active_buffer {
                ctx.set_buffer_id(buffer_id);
            }

            if let Some(result) = self.command_registry.execute(operator, &mut self.app, &ctx) {
                self.handle_command_result(result);
            }
        }
    }

    /// Read next key event.
    #[allow(clippy::unnecessary_wraps)]
    fn read_key(&mut self) -> Result<Option<KeyEvent>, EventLoopError> {
        Ok(self.key_reader.as_mut().and_then(|reader| reader()))
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

    /// Convert an input driver `ArgValue` to a command driver `ArgValue`.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    fn convert_arg_value(value: &InputArgValue) -> Option<reovim_driver_command::ArgValue> {
        use reovim_driver_command::ArgValue as CmdArg;

        Some(match value {
            InputArgValue::Bool(b) => CmdArg::Bang(*b),
            InputArgValue::Int(i) => {
                if *i >= 0 {
                    CmdArg::Count(*i as usize)
                } else {
                    return None;
                }
            }
            InputArgValue::Uint(u) => CmdArg::Count(*u as usize),
            InputArgValue::String(s) => CmdArg::String(s.clone()),
            InputArgValue::Char(c) => CmdArg::Char(*c),
            InputArgValue::Float(_) | InputArgValue::Position { .. } => return None,
            InputArgValue::Range { start, end, .. } => CmdArg::Range(start.line, end.line),
        })
    }
}

impl std::fmt::Debug for EventLoop {
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
        reovim_driver_command::{ArgSpec, Command, CommandHandler},
        reovim_driver_input::KeyCode,
        reovim_driver_session::SessionRuntime,
        reovim_kernel::api::v1::{CommandId, KernelContext, Mode, ModeId, ModuleId},
        std::sync::Arc,
    };

    const TEST_MODULE: ModuleId = ModuleId::new("test");

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(u16)]
    enum TestMode {
        Command = 0,
        Input = 1,
    }

    impl Mode for TestMode {
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

    fn create_test_event_loop() -> EventLoop {
        let kernel = KernelContext::default();
        let app = AppState::new(kernel, test_mode());

        let mut mode_registry = ModeRegistry::new();
        mode_registry.register_mode(TestMode::Command);
        mode_registry.register_mode(TestMode::Input);

        EventLoop::new(app, mode_registry, CommandRegistry::new(), KeymapRegistry::new())
    }

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
        fn execute(
            &self,
            _runtime: &mut SessionRuntime<'_>,
            _args: &CommandContext,
        ) -> CommandResult {
            CommandResult::Success
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

        assert!(event_loop.step().unwrap());
        assert!(event_loop.step().unwrap());
        assert!(!event_loop.step().unwrap());
    }

    #[test]
    fn test_event_loop_command_execution() {
        let mut event_loop = create_test_event_loop();

        let cmd = TestCommand {
            id: test_command_id("test-cmd"),
        };
        event_loop.command_registry_mut().register(Arc::new(cmd));

        let mode = test_mode();
        event_loop
            .keymap_registry_mut()
            .register_str(&mode, "j", test_command_id("test-cmd"));

        let mut keys = vec![KeyEvent::new(KeyCode::Char('j'))].into_iter();
        event_loop.key_reader = Some(Box::new(move || keys.next()));

        assert!(event_loop.step().unwrap());
        assert!(event_loop.last_error().is_none());
    }

    #[test]
    fn test_event_loop_error_handling() {
        let mut event_loop = create_test_event_loop();

        event_loop.set_error("Test error");
        assert_eq!(event_loop.last_error(), Some("Test error"));

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
