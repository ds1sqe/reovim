//! Find-char motion execution command.
//!
//! This command executes find-char motions (f, F, t, T) with the target
//! character passed via command context. It's part of Epic #385's resolver-based
//! approach where the vim resolver handles `pending_char` state and calls this
//! command to execute the actual motion.
//!
//! # Architecture
//!
//! ```text
//! Key press → VimNormalResolver → Sets vim.pending_char → Returns Pending
//! Next char → VimNormalResolver → Returns Execute(EXECUTE_FIND_CHAR, ctx_with_char)
//! Runner    → Calls this command with char in context
//! Command   → Calculates motion, moves cursor, returns Success
//! ```
//!
//! # Note
//!
//! This command does NOT update `last_find` because commands don't have
//! access to `VimSessionState` via extensions yet. That will be added in
//! a future phase when commands have API support (#394).

use {
    reovim_driver_command::{ArgValue, Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{SessionRuntime, api::ChangeTracker},
    reovim_kernel::api::v1::{CommandId, Cursor, Motion, MotionEngine, Position},
};

use crate::ids::EXECUTE_FIND_CHAR;

/// Execute a find-char motion (f, F, t, T) with character from context.
///
/// Reads the target character and direction from command context metadata
/// and moves the cursor to the appropriate position.
///
/// # Arguments (via `CommandContext`)
///
/// - `find_char`: The target character (required)
/// - `find_direction`: "forward" or "backward" (default: "forward")
/// - `find_inclusive`: true for f/F (on char), false for t/T (before char) (default: true)
/// - `count`: Number of occurrences to skip (default: 1)
pub struct ExecuteFindChar;

impl Command for ExecuteFindChar {
    fn id(&self) -> CommandId {
        EXECUTE_FIND_CHAR
    }

    fn description(&self) -> &'static str {
        "Execute a find-char motion (f, F, t, T) with the target character from context"
    }
}

impl CommandHandler for ExecuteFindChar {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Get the target character (required)
        let Some(target_char) = args.char("find_char") else {
            return CommandResult::error("find_char argument required");
        };

        // Get direction (default: forward)
        let forward = args.string("find_direction") != Some("backward");

        // Get inclusive flag (default: true for find, false for till).
        let inclusive = match args.get("find_inclusive") {
            Some(ArgValue::Bang(b)) => *b,
            _ => true,
        };

        // Get count (default: 1)
        let count = args.count().unwrap_or(1);

        // Get active buffer
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get cursor position from active window
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let cursor_line = window.cursor.line;
        let cursor_col = window.cursor.column;

        let cursor = Cursor::new(Position::new(cursor_line, cursor_col));
        let motion = Motion::FindChar {
            char: target_char,
            direction: if forward {
                reovim_kernel::api::v1::Direction::Forward
            } else {
                reovim_kernel::api::v1::Direction::Backward
            },
            till: !inclusive,
        };

        let target = runtime.with_buffer_read(buffer_id, |buffer| {
            MotionEngine::calculate(buffer, &cursor, motion, count)
        });

        let Some(target) = target else {
            return CommandResult::error("Buffer not found");
        };

        // Apply motion if target found
        if let Some(pos) = target {
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = pos.into();
            }
            // #474: Record cursor move so notification pipeline and
            // centralized selection extension are triggered.
            runtime.record_cursor_move(buffer_id);
        }
        // Character not found - this is not an error, just don't move
        // (Vim behavior: cursor stays in place, no beep)
        CommandResult::Success
    }
}

#[cfg(test)]
#[allow(clippy::significant_drop_tightening)]
mod tests {
    use {
        super::*,
        reovim_driver_command::CommandHandler,
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, SessionRuntime, WindowLayout,
            api::{CommandExecutor, CommandHandle},
        },
        reovim_kernel::api::{
            ModeStack,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, EventBus, HistoryRing, KernelContext,
                MarkBank, ModeId, ModuleId, MotionEngine, OptionRegistry, RegisterBank, RwLock,
                ServiceRegistry, TextObjectEngine,
            },
        },
        std::{collections::HashMap, sync::Arc},
    };

    // ========================================================================
    // Test infrastructure
    // ========================================================================

    struct TestBufferManager {
        buffers: RwLock<HashMap<BufferId, Arc<RwLock<Buffer>>>>,
    }

    impl TestBufferManager {
        fn new() -> Self {
            Self {
                buffers: RwLock::new(HashMap::new()),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl BufferManager for TestBufferManager {
        fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>> {
            self.buffers.read().get(&id).cloned()
        }

        fn create(&self) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(Buffer::new()));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn register(&self, buffer: Buffer) -> BufferId {
            let id = BufferId::new();
            let buffer = Arc::new(RwLock::new(buffer));
            self.buffers.write().insert(id, buffer);
            id
        }

        fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError> {
            self.buffers
                .write()
                .remove(&id)
                .map_or(Err(BufferError::NotFound(id)), |arc_buffer| {
                    Arc::try_unwrap(arc_buffer)
                        .map_or_else(|arc| Ok(arc.read().clone()), |rwlock| Ok(rwlock.into_inner()))
                })
        }

        fn list(&self) -> Vec<BufferId> {
            self.buffers.read().keys().copied().collect()
        }

        fn count(&self) -> usize {
            self.buffers.read().len()
        }
    }

    struct StubExecutor;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl CommandExecutor for StubExecutor {
        fn get_handle(&self, _id: &CommandId) -> Option<std::sync::Arc<dyn CommandHandle>> {
            None
        }
    }

    struct TestState {
        session: Session,
        mode_stack: ModeStack,
        windows: WindowLayout,
        extensions: ExtensionMap,
        compositor: Option<Box<dyn reovim_driver_display::layout::RootCompositor>>,
        tabs: reovim_driver_session::TabPageSet,
        registers: RegisterBank,
        clipboard_history: HistoryRing,
        local_marks: MarkBank,
        active_buffer: Option<BufferId>,
        terminal_size: (u16, u16),
    }

    impl TestState {
        fn with_buffer(buffer_id: Option<BufferId>) -> Self {
            let home_mode = ModeId::new(ModuleId::new("test"), "normal");
            let session = Session::new(ClientId::new(1), home_mode.clone());
            let mode_stack = ModeStack::new(home_mode);
            let mut windows = WindowLayout::empty();
            let extensions = ExtensionMap::new();

            let mut window = reovim_driver_session::Window::new();
            if let Some(buffer_id) = buffer_id {
                window.buffer_id = Some(buffer_id);
            }
            windows.add(window);

            Self {
                session,
                mode_stack,
                windows,
                extensions,
                compositor: None,
                tabs: reovim_driver_session::TabPageSet::new(),
                registers: RegisterBank::new(),
                clipboard_history: HistoryRing::new(),
                local_marks: MarkBank::new(),
                active_buffer: None,
                terminal_size: (80, 24),
            }
        }

        fn runtime<'a>(&'a mut self, kernel: &'a KernelContext) -> SessionRuntime<'a> {
            SessionRuntime::new(
                &mut self.session,
                reovim_driver_session::ClientContext {
                    mode_stack: &mut self.mode_stack,
                    windows: &mut self.windows,
                    extensions: &mut self.extensions,
                    compositor: &mut self.compositor,
                    tabs: &mut self.tabs,
                    registers: &mut self.registers,
                    clipboard_history: &mut self.clipboard_history,
                    local_marks: &mut self.local_marks,
                    active_buffer: &mut self.active_buffer,
                    terminal_size: &mut self.terminal_size,
                },
                kernel,
                &StubExecutor,
            )
        }
    }

    fn create_test_context() -> KernelContext {
        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            Arc::new(ServiceRegistry::new()),
        )
    }

    // ========================================================================
    // Metadata tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_command_metadata() {
        let cmd = ExecuteFindChar;
        assert_eq!(cmd.id(), EXECUTE_FIND_CHAR);
        assert!(cmd.description().contains("find-char"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_command_id_module() {
        let cmd = ExecuteFindChar;
        assert_eq!(cmd.id().module().as_str(), "vim");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_command_id_name() {
        let cmd = ExecuteFindChar;
        assert_eq!(cmd.id().name(), "execute-find-char");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_command_description_not_empty() {
        let cmd = ExecuteFindChar;
        assert!(!cmd.description().is_empty());
    }

    // ========================================================================
    // Execute tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_no_find_char_arg() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_no_buffer() {
        let ctx = create_test_context();
        let mut args = CommandContext::new();
        args.set("find_char", ArgValue::Char('x'));

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_forward() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('o'));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Cursor should move to 'o' in "hello" (column 4)
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 4);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_not_found() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('z'));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        // Should succeed (not an error) but cursor doesn't move
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_backward() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('h'));
        args.set("find_direction", ArgValue::String("backward".to_string()));

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Start at column 5
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 5).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_till() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('o'));
        args.set("find_inclusive", ArgValue::Bang(false)); // till mode

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Cursor should be one before 'o' (column 3)
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 3);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_default_direction_is_forward() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("abcdef");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('d'));
        // No find_direction set - should default to forward

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 3);
    }

    // ========================================================================
    // Additional execute tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_last_char_on_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("abcde");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('e'));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 4);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_first_char_on_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("abcde");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('a'));
        args.set("find_direction", ArgValue::String("backward".to_string()));

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 4).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_with_count() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("abcabc");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('a'));
        // count=1 should find the next 'a' after cursor (at col 3)
        args.set("count", reovim_driver_command::ArgValue::Count(1));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Next 'a' after cursor at col 0 is at col 3
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 3);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_till_backward() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('h'));
        args.set("find_direction", ArgValue::String("backward".to_string()));
        args.set("find_inclusive", ArgValue::Bang(false)); // till mode

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 5).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Till backward to 'h' means stop one position after 'h'
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 1);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_forward_direction_string() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("abcdef");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('f'));
        args.set("find_direction", ArgValue::String("forward".to_string()));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 5);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_space_character() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char(' '));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 5); // Space is at col 5
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_inclusive_default() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("abcdef");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('c'));
        // No find_inclusive set - should default to true (inclusive/find mode)

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 2); // On 'c' (inclusive)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_on_empty_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('x'));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Cursor stays at 0 (nothing found)
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_no_active_window() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("find_char", ArgValue::Char('o'));

        // Create runtime with no windows at all
        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty(); // No windows
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
        let mut active_buffer = None;
        let mut terminal_size = (80u16, 24u16);
        let mut runtime = SessionRuntime::new(
            &mut session,
            reovim_driver_session::ClientContext {
                mode_stack: &mut mode_stack,
                windows: &mut windows,
                extensions: &mut extensions,
                compositor: &mut compositor,
                tabs: &mut tabs,
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
                active_buffer: &mut active_buffer,
                terminal_size: &mut terminal_size,
            },
            &ctx,
            &StubExecutor,
        );

        let result = ExecuteFindChar.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_execute_find_char_buffer_not_found() {
        let ctx = create_test_context();
        // Register a buffer_id in args but don't put the actual buffer in the manager
        let fake_buffer_id = BufferId::from_raw(999);

        let mut args = CommandContext::new();
        args.set_buffer_id(fake_buffer_id);
        args.set("find_char", ArgValue::Char('o'));

        let mut state = TestState::with_buffer(Some(fake_buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ExecuteFindChar.execute(&mut runtime, &args);
        // Buffer not found returns error
        assert!(matches!(result, CommandResult::Error(_)));
    }
}
