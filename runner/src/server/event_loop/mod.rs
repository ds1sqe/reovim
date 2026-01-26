//! Main event loop for the runner.
//!
//! The event loop uses an **emit + process pattern** for key handling:
//!
//! 1. **Emit**: `handle_key()` converts and queues the key (non-blocking)
//! 2. **Process**: `process_events()` resolves with full `&mut self` access
//!
//! This pattern avoids borrow checker issues while maintaining clear
//! separation between event dispatch (mechanism) and key resolution (policy).
//!
//! # Design Philosophy
//!
//! Following the "mechanism vs policy" principle:
//! - **Mechanism** (this module): Key dispatch via `EventBus`
//! - **Policy** (modules): Resolvers decide what to do with keys
//!
//! The event loop contains NO business logic - resolvers handle everything.

mod error;
pub mod handlers;

pub use error::EventLoopError;

use std::sync::Arc;

use {
    reovim_driver_session::{
        ClientId, Session as DriverSession, SessionRuntime,
        api::{CommandExecutor, StateChanges},
    },
    reovim_driver_vfs::VfsDriver,
};

use {
    reovim_driver_command::{CommandContext, CommandResult},
    reovim_driver_input::{
        ArgValue as InputArgValue, KeyEvent, ModeState, ModeTransition, PopResult, ResolveResult,
    },
    reovim_kernel::{
        api::v1::{
            EventBus, EventSender, ModeId,
            events::{
                ClientId as KernelClientId, KeyCode, KeyInput, KeyPressEvent, Modifiers, SessionId,
            },
        },
        profile_scope,
    },
};

/// Channel buffer capacity for key events.
///
/// 1024 is chosen because:
/// - Far exceeds typical key input rate (< 100 keys/sec)
/// - Provides buffer for burst input (macros, paste)
/// - Memory overhead is minimal (`KeyPressEvent` is ~32 bytes)
/// - Synchronous processing ensures queue rarely exceeds 1
const KEY_EVENT_CHANNEL_CAPACITY: usize = 1024;

use super::{
    AppState, PromptType,
    registry::{CommandRegistry, KeymapRegistry, ModeRegistry},
};

use reovim_driver_session::{CmdlinePrompt, CmdlineState};

use reovim_driver_input::ResolverRegistry;

/// Main event loop for the runner.
///
/// Uses **emit + process pattern** for key handling (#409):
///
/// 1. `handle_key()` - Converts key and emits to channel (non-blocking)
/// 2. `process_events()` - Resolves with full `&mut self` access
///
/// This pattern solves the borrow checker constraint that prevented
/// handler closures from accessing mutable state.
///
/// # Architecture
///
/// ```text
/// handle_key()     → Emit only (non-blocking try_send)
/// process_events() → Full resolution with &mut self
/// ```
pub struct EventLoop {
    /// Driver session (SSOT for `mode_stack`, `active_buffer`, etc.).
    ///
    /// As of #406, this is the canonical source for session state.
    driver_session: DriverSession,

    /// Application state (kernel + runtime).
    pub(super) app: AppState,

    /// `EventBus` for key dispatch (#408).
    ///
    /// Created with channel for emit + process pattern.
    /// Keys are emitted via `sender` and processed in `process_events()`.
    #[allow(dead_code)] // Kept for future handler registration
    event_bus: EventBus,

    /// Sender for emitting key events to the channel.
    ///
    /// Used in `handle_key()` for non-blocking emit.
    sender: EventSender,

    /// Stores original `KeyEvent` (driver type) for `try_resolver()` access.
    ///
    /// # Why Single Slot is Safe
    ///
    /// This is a single `Option`, not a queue, because:
    /// 1. Processing is synchronous: `handle_key()` → `process_events()` → next key
    /// 2. No concurrent key readers exist (single-threaded event loop)
    /// 3. `process_events()` always takes the pending key before returning
    ///
    /// The kernel `KeyPressEvent` is for `EventBus` routing only; we need the
    /// original driver `KeyEvent` for resolver compatibility.
    pending_key_event: Option<KeyEvent>,

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

    /// Virtual filesystem driver for file operations.
    ///
    /// Passed to `command_registry.execute()` for context enrichment (Epic #415).
    vfs: Arc<dyn VfsDriver>,
}

impl EventLoop {
    /// Create a new event loop.
    ///
    /// Creates a driver session using the provided initial mode.
    /// The initial mode is typically Normal mode.
    ///
    /// # Emit + Process Pattern (#409)
    ///
    /// Creates an `EventBus` with channel for the emit + process pattern:
    /// - `handle_key()` emits to channel via `sender`
    /// - `process_events()` resolves with full `&mut self` access
    ///
    /// This replaces the transitional handler approach from #408.
    ///
    /// # Panics
    ///
    /// Panics if `EventBus::new_with_channel()` fails to create a sender.
    /// This should never happen as we always create the bus with a channel.
    #[must_use]
    pub fn new(
        app: AppState,
        initial_mode: ModeId,
        mode_registry: ModeRegistry,
        command_registry: CommandRegistry,
        keymap_registry: KeymapRegistry,
        vfs: Arc<dyn VfsDriver>,
    ) -> Self {
        // Create driver session with the initial mode (SSOT for mode_stack)
        let driver_session = DriverSession::new(ClientId::new(0), initial_mode);

        // Create EventBus with channel for emit + process pattern (#409)
        let event_bus = EventBus::new_with_channel(KEY_EVENT_CHANNEL_CAPACITY);

        // Get sender for non-blocking emit in handle_key()
        let sender = event_bus
            .sender()
            .expect("EventBus created with channel must have sender");

        Self {
            driver_session,
            app,
            event_bus,
            sender,
            pending_key_event: None,
            mode_registry,
            command_registry,
            keymap_registry,
            key_reader: None,
            last_error: None,
            resolver_registry: None,
            vfs,
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
    /// Uses emit + process pattern for each key:
    /// 1. `handle_key()` - Emit only (non-blocking)
    /// 2. `process_events()` - Resolve with full `&mut self`
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
            self.process_events();
        }
        Ok(())
    }

    /// Run a single iteration (for testing).
    ///
    /// Uses emit + process pattern:
    /// 1. `handle_key()` - Emit only
    /// 2. `process_events()` - Resolve with full `&mut self`
    ///
    /// # Errors
    ///
    /// Returns an error if input reading fails.
    pub fn step(&mut self) -> Result<bool, EventLoopError> {
        let Some(key) = self.read_key()? else {
            return Ok(false);
        };
        self.handle_key(key);
        self.process_events();
        Ok(true)
    }

    /// Handle a single key event (emit only).
    ///
    /// This method ONLY emits - no resolution logic here.
    /// Resolution happens in `process_events()` which has full `&mut self`.
    ///
    /// # Emit + Process Pattern (#409)
    ///
    /// ```text
    /// handle_key()     → Convert + emit (non-blocking)
    /// process_events() → Resolve with &mut self access
    /// ```
    fn handle_key(&mut self, key: KeyEvent) {
        profile_scope!("handle_key", "runner::event_loop");

        // Convert driver KeyEvent to kernel KeyInput for EventBus
        let key_input = Self::key_event_to_key_input(&key);
        let event = KeyPressEvent::new(
            key_input,
            SessionId::new(0),      // Single-session mode
            KernelClientId::new(0), // Default client
        );

        // Store original key for process_events() (resolver needs driver type)
        self.pending_key_event = Some(key);

        // Emit to channel (non-blocking, for future async/multi-session support)
        self.sender.try_send(event);
    }

    /// Process pending key events with full mutable access.
    ///
    /// Called immediately after `handle_key()` in the event loop.
    /// This method has full `&mut self` access, solving the borrow checker
    /// constraint that prevented resolution in handler closures.
    ///
    /// # Emit + Process Pattern (#409)
    ///
    /// ```text
    /// handle_key()     → Emit only (non-blocking try_send)
    /// process_events() → Full resolution with &mut self
    /// ```
    fn process_events(&mut self) {
        // Take the pending key if any
        let Some(key) = self.pending_key_event.take() else {
            return;
        };

        // Full resolution with mutable access
        if let Some((result, _changes)) = self.try_resolver(&key) {
            self.handle_resolve_result(result);

            // Sync cmdline state with current mode (#435)
            self.sync_cmdline_state();

            // TODO: Broadcast state changes to clients
            // if changes.has_changes() {
            //     self.broadcast_state_changes(&changes);
            // }
        }
        // No resolver = key ignored (resolver handles everything)
    }

    /// Sync cmdline state based on `CmdlineState` extension.
    ///
    /// Policy (modules) sets `CmdlineState.active` when entering cmdline mode.
    /// Mechanism (runner) syncs the display state accordingly.
    fn sync_cmdline_state(&mut self) {
        let ext_state = self.app.extensions.get::<CmdlineState>();

        // Check if policy has activated cmdline (via extension)
        let policy_active = ext_state.is_some_and(CmdlineState::is_active);

        if policy_active && !self.app.cmdline.is_active() {
            // Policy activated cmdline: sync prompt type
            let prompt_type = ext_state.map_or(CmdlinePrompt::Command, CmdlineState::prompt);

            let runner_prompt = match prompt_type {
                CmdlinePrompt::Command => PromptType::Command,
                CmdlinePrompt::SearchForward => PromptType::SearchForward,
                CmdlinePrompt::SearchBackward => PromptType::SearchBackward,
            };

            self.app.cmdline.enter_with_prompt(runner_prompt);
            eprintln!("[DEBUG] Activated cmdline with prompt: {runner_prompt:?}");
        } else if !policy_active && self.app.cmdline.is_active() {
            // Policy deactivated cmdline: execute any pending action, then sync
            self.execute_cmdline_action();
            self.app.cmdline.cancel();
            eprintln!("[DEBUG] Deactivated cmdline");
        }
    }

    /// Execute the cmdline action based on prompt type.
    ///
    /// For search prompts, this executes the search and moves cursor.
    fn execute_cmdline_action(&mut self) {
        use reovim_driver_search::{Direction, SearchKey, SearchProviderRegistry};

        let prompt = self.app.cmdline.prompt_type();
        let input = self.app.cmdline.input().to_string();

        if input.is_empty() {
            return;
        }

        match prompt {
            PromptType::SearchForward | PromptType::SearchBackward => {
                let direction = if prompt == PromptType::SearchForward {
                    Direction::Forward
                } else {
                    Direction::Backward
                };

                // Execute search using SearchProviderRegistry
                if let Some(search_registry) = self.app.services.get::<SearchProviderRegistry>()
                    && let Some(buffer_id) = self.driver_session.active_buffer()
                {
                    // Get cursor position from active window in driver_session
                    let cursor_pos = self
                        .driver_session
                        .windows
                        .active()
                        .map(|w| {
                            reovim_kernel::api::v1::Position::new(w.cursor.line, w.cursor.column)
                        })
                        .unwrap_or_default();

                    if let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) {
                        // Find the match position (with read lock)
                        let search_result = {
                            let buffer = buffer_arc.read();
                            search_registry.get(&SearchKey::Regex).and_then(|provider| {
                                provider
                                    .find_next(&buffer, cursor_pos, &input, direction, true)
                                    .ok()
                                    .flatten()
                            })
                        }; // Read lock dropped

                        match search_result {
                            Some(m) => {
                                // Update buffer's internal cursor position (SSOT for RPC)
                                buffer_arc.write().set_position(m.start);
                                // Update window cursor in driver_session
                                if let Some(window) = self.driver_session.windows.active_mut() {
                                    window.cursor.line = m.start.line;
                                    window.cursor.column = m.start.column;
                                }
                                eprintln!("[DEBUG] Search found at {m_start:?}", m_start = m.start);
                            }
                            None => {
                                self.set_error(format!("Pattern not found: {input}"));
                            }
                        }
                    }
                }
            }
            PromptType::Command => {
                // TODO: Execute Ex command
                eprintln!("[DEBUG] Ex command: {input}");
            }
        }
    }

    /// Convert driver `KeyEvent` to kernel `KeyInput`.
    ///
    /// The kernel has its own key types to maintain layer purity.
    /// Driver uses bitflags for modifiers, kernel uses struct with bool fields.
    #[allow(clippy::missing_const_for_fn)] // Modifiers::contains is not const
    fn key_event_to_key_input(key: &KeyEvent) -> KeyInput {
        use reovim_driver_input::{KeyCode as DriverKeyCode, Modifiers as DriverModifiers};

        // Map driver KeyCode to kernel KeyCode
        // Driver uses `Escape`, kernel uses `Esc`
        let key_code = match key.code {
            DriverKeyCode::Char(c) => KeyCode::Char(c),
            DriverKeyCode::F(n) => KeyCode::F(n),
            DriverKeyCode::Backspace => KeyCode::Backspace,
            DriverKeyCode::Enter => KeyCode::Enter,
            DriverKeyCode::Left => KeyCode::Left,
            DriverKeyCode::Right => KeyCode::Right,
            DriverKeyCode::Up => KeyCode::Up,
            DriverKeyCode::Down => KeyCode::Down,
            DriverKeyCode::Home => KeyCode::Home,
            DriverKeyCode::End => KeyCode::End,
            DriverKeyCode::PageUp => KeyCode::PageUp,
            DriverKeyCode::PageDown => KeyCode::PageDown,
            DriverKeyCode::Tab => KeyCode::Tab,
            DriverKeyCode::BackTab => KeyCode::BackTab,
            DriverKeyCode::Delete => KeyCode::Delete,
            DriverKeyCode::Insert => KeyCode::Insert,
            DriverKeyCode::Escape => KeyCode::Esc,
            // Unsupported keys map to Null
            DriverKeyCode::Null
            | DriverKeyCode::CapsLock
            | DriverKeyCode::ScrollLock
            | DriverKeyCode::NumLock
            | DriverKeyCode::PrintScreen
            | DriverKeyCode::Pause
            | DriverKeyCode::Menu
            | DriverKeyCode::KeypadBegin
            | DriverKeyCode::MediaPlay
            | DriverKeyCode::MediaPause
            | DriverKeyCode::MediaPlayPause
            | DriverKeyCode::MediaStop
            | DriverKeyCode::MediaReverse
            | DriverKeyCode::MediaFastForward
            | DriverKeyCode::MediaRewind
            | DriverKeyCode::MediaNext
            | DriverKeyCode::MediaPrevious
            | DriverKeyCode::MediaRecord
            | DriverKeyCode::MediaLowerVolume
            | DriverKeyCode::MediaRaiseVolume
            | DriverKeyCode::MediaMuteVolume
            | DriverKeyCode::LeftShift
            | DriverKeyCode::RightShift
            | DriverKeyCode::LeftCtrl
            | DriverKeyCode::RightCtrl
            | DriverKeyCode::LeftAlt
            | DriverKeyCode::RightAlt
            | DriverKeyCode::LeftSuper
            | DriverKeyCode::RightSuper
            | DriverKeyCode::LeftHyper
            | DriverKeyCode::RightHyper
            | DriverKeyCode::LeftMeta
            | DriverKeyCode::RightMeta
            | DriverKeyCode::IsoLevel3Shift
            | DriverKeyCode::IsoLevel5Shift => KeyCode::Null,
        };

        // Convert bitflags to struct (driver uses bitflags, kernel uses bool fields)
        let modifiers = Modifiers {
            ctrl: key.modifiers.contains(DriverModifiers::CTRL),
            alt: key.modifiers.contains(DriverModifiers::ALT),
            shift: key.modifiers.contains(DriverModifiers::SHIFT),
            super_key: key.modifiers.contains(DriverModifiers::SUPER),
        };

        KeyInput {
            key: key_code,
            modifiers,
        }
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
        use reovim_driver_session::api::ChangeTracker;

        let registry = self.resolver_registry.as_ref()?;
        let mode = self.driver_session.mode_stack.current().clone();
        let mut mode_state = ModeState::new(mode.clone());

        // Create SessionRuntime for session API access.
        // SessionRuntime implements all session APIs using Session.windows and Session.compositor.
        let stub_executor = StubCommandExecutor;
        let mut runtime =
            SessionRuntime::new(&mut self.driver_session, &self.app.kernel, &stub_executor);

        // Resolvers access session state via SessionApiDyn + extensions
        // Extensions contain module-specific state (e.g., VimSessionState)
        // NOTE: Uses app.extensions due to borrow checker - driver_session is already borrowed by runtime
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
        eprintln!("[DEBUG] handle_resolve_result: {result:?}");
        match result {
            ResolveResult::Execute(cmd_id, ctx) => {
                let mut cmd_ctx = CommandContext::new();

                if let Some(count) = ctx.count {
                    cmd_ctx.set("count", reovim_driver_command::ArgValue::Count(count));
                }

                if let Some(reg) = ctx.register {
                    cmd_ctx.set("register", reovim_driver_command::ArgValue::Register(reg));
                }

                if let Some(buffer_id) = self.driver_session.active_buffer() {
                    cmd_ctx.set_buffer_id(buffer_id);
                }

                cmd_ctx.set_mode_name(self.driver_session.mode_stack.current().name());

                // Transfer metadata
                for (key, value) in ctx.metadata {
                    if let Some(cmd_value) = Self::convert_arg_value(&value) {
                        let static_key: &'static str = Box::leak(key.into_boxed_str());
                        cmd_ctx.set(static_key, cmd_value);
                    }
                }

                eprintln!(
                    "[DEBUG] Executing command {} in mode {}",
                    cmd_id,
                    cmd_ctx.mode_name().unwrap_or("unknown")
                );

                if let Some(result) = self.command_registry.execute(
                    &cmd_id,
                    &mut self.driver_session,
                    &mut self.app,
                    &self.vfs,
                    &cmd_ctx,
                ) {
                    eprintln!("[DEBUG] Command result: {result:?}");

                    // Per #388: Call post-command hook on resolver to complete
                    // pending operations (e.g., operator+motion in vim).
                    // Resolver stores motion type info BEFORE dispatch, so no
                    // need for CommandResult::Motion variant.
                    if result.is_success()
                        && let Some(transition) = self.try_resolver_on_command_complete()
                    {
                        self.handle_mode_transition(transition);
                    }

                    self.handle_command_result(result);
                }
            }

            ResolveResult::ModeTransition(transition) => {
                self.handle_mode_transition(transition);
            }

            // InsertChar: insert character into current input context (cmdline or buffer)
            ResolveResult::InsertChar(ch) => {
                // Command-line mode: insert into cmdline buffer
                if self.app.cmdline.is_active() {
                    self.app.cmdline.insert_char(ch);
                    eprintln!("[DEBUG] InsertChar '{ch}' → cmdline: {}", self.app.cmdline.input());
                }
                // Note: For regular insert mode, the resolver returns Execute with insert command
            }

            // Pending: wait for more keys
            // NotHandled: key not handled, ignore
            // Completed: resolver already did everything via SessionApi, changes are tracked
            ResolveResult::Pending | ResolveResult::NotHandled | ResolveResult::Completed => {}
        }
    }

    /// Handle a mode transition from a resolver.
    ///
    /// SSOT: Mode transitions go through `driver_session` (canonical source).
    fn handle_mode_transition(&mut self, transition: ModeTransition) {
        match transition {
            ModeTransition::Push { mode, context: _ } => {
                // Context is handled by resolver (stored in VimSessionState)
                self.driver_session.mode_stack.push(mode);
            }

            ModeTransition::Pop { result } => {
                if let Some(ref pop_result) = result {
                    self.handle_pop_result(pop_result);
                }
                self.driver_session.mode_stack.pop();
            }

            ModeTransition::Set { mode, context: _ } => {
                self.driver_session.mode_stack.set(mode);
            }
        }
    }

    /// Handle a pop result from a mode.
    ///
    /// Pure mechanism: executes whatever command the mode provides.
    /// Runner has no knowledge of what the command does or why.
    fn handle_pop_result(&mut self, result: &PopResult) {
        match result {
            PopResult::ExecuteCommand { command, args } => {
                let mut ctx = CommandContext::new();

                // Transfer all arguments from the pop result
                for (key, value) in args {
                    // Leak the key to get &'static str (args come from module, long-lived)
                    let static_key: &'static str = Box::leak(key.clone().into_boxed_str());
                    ctx.set(static_key, value.clone());
                }

                // Set active buffer
                if let Some(buffer_id) = self.driver_session.active_buffer() {
                    ctx.set_buffer_id(buffer_id);
                }

                // Execute the command - runner doesn't know what it does
                if let Some(result) = self.command_registry.execute(
                    command,
                    &mut self.driver_session,
                    &mut self.app,
                    &self.vfs,
                    &ctx,
                ) {
                    self.handle_command_result(result);
                }
            }
            PopResult::Cancelled | PopResult::Data { .. } => {
                // Cancelled: nothing to execute
                // Data: parent mode handles, runner doesn't care
            }
        }
    }

    /// Call resolver's `on_command_complete` hook (Epic #415, Issue #388).
    ///
    /// After a command executes successfully, the current mode's resolver
    /// may have pending operations to complete (e.g., operator+motion in vim).
    ///
    /// This keeps vim-specific logic in the vim module - the runner just
    /// calls the hook and handles any resulting mode transition.
    fn try_resolver_on_command_complete(&mut self) -> Option<ModeTransition> {
        let registry = self.resolver_registry.as_ref()?;
        let mode = self.driver_session.mode_stack.current().clone();

        // Get the resolver for the current mode
        let resolver = registry.get(&mode)?;

        // Create SessionRuntime for session API access
        let stub_executor = StubCommandExecutor;
        let mut runtime =
            SessionRuntime::new(&mut self.driver_session, &self.app.kernel, &stub_executor);

        // Call the hook - resolver decides if there's anything to complete
        resolver.on_command_complete(&mut runtime, &mut self.app.extensions)
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

/// Stub command executor for resolver key handling.
///
/// Used when creating `SessionRuntime` for resolver operations.
/// Resolvers should return `ResolveResult::Execute` to trigger command
/// execution, not call `execute_command` directly.
struct StubCommandExecutor;

impl CommandExecutor for StubCommandExecutor {
    fn execute(
        &self,
        _cmd: &reovim_kernel::api::v1::CommandId,
        _ctx: &reovim_driver_command::CommandContext,
        _kernel: &mut reovim_kernel::api::v1::KernelContext,
    ) -> Option<CommandResult> {
        // Resolvers should return ResolveResult::Execute, not call this directly
        Some(CommandResult::Error("command execution via resolver not supported".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::{ArgSpec, Command, CommandHandler},
        reovim_driver_input::KeyCode,
        reovim_driver_session::SessionRuntime,
        reovim_driver_vfs::MockVfs,
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
        let app = AppState::new(kernel);
        let initial_mode = test_mode();
        let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());

        let mut mode_registry = ModeRegistry::new();
        mode_registry.register_mode(TestMode::Command);
        mode_registry.register_mode(TestMode::Input);

        EventLoop::new(
            app,
            initial_mode,
            mode_registry,
            CommandRegistry::new(),
            KeymapRegistry::new(),
            vfs,
        )
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

    // =========================================================================
    // Emit + Process Pattern Tests (#409)
    // =========================================================================

    #[test]
    fn test_handle_key_stores_pending_event() {
        let mut event_loop = create_test_event_loop();
        let key = KeyEvent::new(KeyCode::Char('j'));

        // Before: no pending event
        assert!(event_loop.pending_key_event.is_none());

        // Emit phase: key stored
        event_loop.handle_key(key);
        assert!(event_loop.pending_key_event.is_some());
    }

    #[test]
    fn test_process_events_clears_pending() {
        let mut event_loop = create_test_event_loop();
        let key = KeyEvent::new(KeyCode::Char('j'));

        // Store a pending event
        event_loop.pending_key_event = Some(key);
        assert!(event_loop.pending_key_event.is_some());

        // Process: event consumed
        event_loop.process_events();
        assert!(event_loop.pending_key_event.is_none());
    }

    #[test]
    fn test_emit_process_separation() {
        let mut event_loop = create_test_event_loop();
        let key = KeyEvent::new(KeyCode::Char('j'));

        // Emit phase: key stored, not processed
        event_loop.handle_key(key);
        assert!(event_loop.pending_key_event.is_some());

        // Process phase: key consumed
        event_loop.process_events();
        assert!(event_loop.pending_key_event.is_none());
    }

    #[test]
    fn test_step_executes_emit_then_process() {
        let mut keys = vec![KeyEvent::new(KeyCode::Char('j'))].into_iter();
        let mut event_loop = create_test_event_loop().with_key_reader(move || keys.next());

        // step() should emit then process, leaving no pending event
        assert!(event_loop.step().unwrap());
        assert!(event_loop.pending_key_event.is_none());
    }

    #[test]
    fn test_try_resolver_returns_none_no_registry() {
        let mut event_loop = create_test_event_loop();
        // No resolver registry set
        assert!(event_loop.resolver_registry.is_none());

        let key = KeyEvent::new(KeyCode::Char('j'));
        // Should return None gracefully
        let result = event_loop.try_resolver(&key);
        assert!(result.is_none());
    }

    #[test]
    fn test_key_event_conversion_basic() {
        use super::KeyCode as KernelKeyCode;

        // Test basic key conversion
        let key = KeyEvent::new(KeyCode::Char('a'));
        let input = EventLoop::key_event_to_key_input(&key);
        assert_eq!(input.key, KernelKeyCode::Char('a'));

        let key = KeyEvent::new(KeyCode::Enter);
        let input = EventLoop::key_event_to_key_input(&key);
        assert_eq!(input.key, KernelKeyCode::Enter);

        let key = KeyEvent::new(KeyCode::Escape);
        let input = EventLoop::key_event_to_key_input(&key);
        assert_eq!(input.key, KernelKeyCode::Esc);
    }

    #[test]
    fn test_key_event_conversion_modifiers() {
        use reovim_driver_input::Modifiers as DriverMods;

        // Ctrl modifier
        let mut key = KeyEvent::new(KeyCode::Char('a'));
        key.modifiers = DriverMods::CTRL;
        let input = EventLoop::key_event_to_key_input(&key);
        assert!(input.modifiers.ctrl);
        assert!(!input.modifiers.alt);

        // Alt modifier
        let mut key = KeyEvent::new(KeyCode::Char('a'));
        key.modifiers = DriverMods::ALT;
        let input = EventLoop::key_event_to_key_input(&key);
        assert!(!input.modifiers.ctrl);
        assert!(input.modifiers.alt);

        // Combined modifiers
        let mut key = KeyEvent::new(KeyCode::Char('a'));
        key.modifiers = DriverMods::CTRL | DriverMods::SHIFT;
        let input = EventLoop::key_event_to_key_input(&key);
        assert!(input.modifiers.ctrl);
        assert!(input.modifiers.shift);
    }

    // =========================================================================
    // Command Result Tests
    // =========================================================================

    #[test]
    fn test_command_result_quit() {
        let mut event_loop = create_test_event_loop();
        assert!(event_loop.app().is_running());

        event_loop.handle_command_result(CommandResult::Quit);
        assert!(!event_loop.app().is_running());
    }

    #[test]
    fn test_command_result_force_quit() {
        let mut event_loop = create_test_event_loop();
        assert!(event_loop.app().is_running());

        event_loop.handle_command_result(CommandResult::ForceQuit);
        assert!(!event_loop.app().is_running());
    }

    // =========================================================================
    // Arg Value Conversion Tests
    // =========================================================================

    #[test]
    fn test_arg_value_conversion_all_types() {
        use {reovim_driver_command::ArgValue as CmdArg, reovim_kernel::api::v1::Position};

        // Bool -> Bang
        let result = EventLoop::convert_arg_value(&InputArgValue::Bool(true));
        assert!(matches!(result, Some(CmdArg::Bang(true))));

        // Positive Int -> Count
        let result = EventLoop::convert_arg_value(&InputArgValue::Int(42));
        assert!(matches!(result, Some(CmdArg::Count(42))));

        // Uint -> Count
        let result = EventLoop::convert_arg_value(&InputArgValue::Uint(100));
        assert!(matches!(result, Some(CmdArg::Count(100))));

        // String -> String
        let result = EventLoop::convert_arg_value(&InputArgValue::String("test".to_string()));
        assert!(matches!(result, Some(CmdArg::String(s)) if s == "test"));

        // Char -> Char
        let result = EventLoop::convert_arg_value(&InputArgValue::Char('x'));
        assert!(matches!(result, Some(CmdArg::Char('x'))));

        // Range -> Range
        let result = EventLoop::convert_arg_value(&InputArgValue::Range {
            start: Position::new(0, 0),
            end: Position::new(10, 0),
            linewise: true,
        });
        assert!(matches!(result, Some(CmdArg::Range(0, 10))));

        // Float -> None (not supported)
        let result = EventLoop::convert_arg_value(&InputArgValue::Float(1.5));
        assert!(result.is_none());

        // Position -> None (not supported)
        let result = EventLoop::convert_arg_value(&InputArgValue::Position(Position::new(0, 0)));
        assert!(result.is_none());
    }

    #[test]
    fn test_arg_value_conversion_negative_int_returns_none() {
        // Negative Int -> None
        let result = EventLoop::convert_arg_value(&InputArgValue::Int(-1));
        assert!(result.is_none());

        let result = EventLoop::convert_arg_value(&InputArgValue::Int(-100));
        assert!(result.is_none());
    }
}
