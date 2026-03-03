//! Delete commands.
//!
//! Provides delete commands:
//! - `DeleteChar` (x)
//! - `DeleteCharBefore` (X)
//! - `DeleteLine` (dd)
//! - `DeleteToEndOfLine` (D)
//!
//! Note: Change commands (cc, C) are in `reovim_module_vim::commands::change`
//! because they require `VimMode` constants for mode transitions.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{BufferApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, Position, RegisterContent},
};

use crate::ids;

/// Delete character under cursor (x).
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteChar;

impl Command for DeleteChar {
    fn id(&self) -> CommandId {
        ids::DELETE_CHAR
    }

    fn description(&self) -> &'static str {
        "Delete character under cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of characters to delete",
        )]
    }
}

impl CommandHandler for DeleteChar {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let count = args.count().unwrap_or(1);

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);

        // Can't delete on empty line or at end of line
        if line_len == 0 || pos.column >= line_len {
            return CommandResult::Success; // No-op
        }

        // Delete up to end of line
        let chars_to_delete = count.min(line_len - pos.column);
        if chars_to_delete > 0 {
            let end = Position::new(pos.line, pos.column + chars_to_delete);
            runtime.delete_range(buffer_id, pos, end);
        }

        CommandResult::Success
    }
}

/// Delete character before cursor (X).
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteCharBefore;

impl Command for DeleteCharBefore {
    fn id(&self) -> CommandId {
        ids::DELETE_CHAR_BEFORE
    }

    fn description(&self) -> &'static str {
        "Delete character before cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of characters to delete",
        )]
    }
}

impl CommandHandler for DeleteCharBefore {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let count = args.count().unwrap_or(1);

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Can't delete before column 0
        if pos.column == 0 {
            // In insert mode, join with previous line
            if pos.line > 0 {
                let prev_line_len = runtime
                    .buffer_line_len(buffer_id, pos.line - 1)
                    .unwrap_or(0);
                let new_pos = Position::new(pos.line - 1, prev_line_len);
                // Update cursor via per-client Window (#471)
                if let Some(window) = runtime.windows_mut().active_mut() {
                    window.cursor = new_pos.into();
                }
                // Delete the newline character (from end of prev line to start of current line)
                let end = Position::new(pos.line, 0);
                runtime.delete_range(buffer_id, new_pos, end);
            }
            return CommandResult::Success;
        }

        let chars_to_delete = count.min(pos.column);
        let new_col = pos.column - chars_to_delete;
        let delete_pos = Position::new(pos.line, new_col);

        // Update cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = delete_pos.into();
        }
        runtime.delete_range(buffer_id, delete_pos, pos);

        CommandResult::Success
    }
}

/// Delete current line (dd).
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteLine;

impl Command for DeleteLine {
    fn id(&self) -> CommandId {
        ids::DELETE_LINE
    }

    fn description(&self) -> &'static str {
        "Delete current line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::optional("count", ArgKind::Count, "Number of lines to delete"),
            ArgSpec::optional("register", ArgKind::Register, "Target register"),
        ]
    }
}

impl CommandHandler for DeleteLine {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let count = args.count().unwrap_or(1);

        // Get cursor from per-client Window (#471)
        let start_line = runtime.windows().active().map_or(0, |w| w.cursor.line);
        let line_count = runtime.buffer_line_count(buffer_id).unwrap_or(0);

        if line_count == 0 {
            return CommandResult::Success;
        }

        // Calculate lines to delete
        let lines_to_delete = count.min(line_count.saturating_sub(start_line));
        if lines_to_delete == 0 {
            return CommandResult::Success;
        }

        // Collect deleted text for register
        let mut deleted_text = String::new();
        for i in 0..lines_to_delete {
            let line_idx = start_line + i;
            if let Some(line) = runtime.buffer_line(buffer_id, line_idx) {
                deleted_text.push_str(&line);
                deleted_text.push('\n');
            }
        }

        // Store in register with clipboard sync (#515)
        let content = RegisterContent::linewise(deleted_text);
        let register = args.register();
        runtime.store_register_with_sync(register, content);

        // Calculate delete range
        let end_line = start_line + lines_to_delete;
        let last_deleted_line = end_line - 1;
        let last_deleted_line_len = runtime
            .buffer_line_len(buffer_id, last_deleted_line)
            .unwrap_or(0);

        let (delete_start, delete_end) = if end_line >= line_count && start_line > 0 {
            // Deleting to end of buffer AND not the first line - include preceding newline
            let prev_line_len = runtime
                .buffer_line_len(buffer_id, start_line - 1)
                .unwrap_or(0);
            (
                Position::new(start_line - 1, prev_line_len),
                Position::new(last_deleted_line, last_deleted_line_len),
            )
        } else if end_line < line_count {
            // Not deleting to end of buffer - include newline after last deleted line
            (Position::new(start_line, 0), Position::new(end_line, 0))
        } else {
            // Deleting to end of buffer from the first line - delete just the content
            (
                Position::new(start_line, 0),
                Position::new(last_deleted_line, last_deleted_line_len),
            )
        };

        // Perform the delete
        runtime.delete_range(buffer_id, delete_start, delete_end);

        // Move cursor to first non-blank of remaining line
        let new_line_count = runtime.buffer_line_count(buffer_id).unwrap_or(0);
        let new_line = start_line.min(new_line_count.saturating_sub(1));
        let first_non_blank = runtime
            .buffer_line(buffer_id, new_line)
            .map_or(0, |line| line.chars().position(|c| !c.is_whitespace()).unwrap_or(0));
        // Update cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = Position::new(new_line, first_non_blank).into();
        }

        CommandResult::Success
    }
}

/// Delete to end of line (D).
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteToEndOfLine;

impl Command for DeleteToEndOfLine {
    fn id(&self) -> CommandId {
        ids::DELETE_TO_EOL
    }

    fn description(&self) -> &'static str {
        "Delete to end of line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "register",
            ArgKind::Register,
            "Target register",
        )]
    }
}

impl CommandHandler for DeleteToEndOfLine {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);

        // Nothing to delete if at or past end of line
        if pos.column >= line_len {
            return CommandResult::Success;
        }

        // Get text to delete for register
        let deleted_text = runtime
            .buffer_line(buffer_id, pos.line)
            .map(|line| line[pos.column..].to_string())
            .unwrap_or_default();

        // Store in register with clipboard sync (#515)
        let content = RegisterContent::characterwise(deleted_text);
        let register = args.register();
        runtime.store_register_with_sync(register, content);

        // Delete from cursor to end of line (not including newline)
        let end = Position::new(pos.line, line_len);
        runtime.delete_range(buffer_id, pos, end);

        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::{ArgValue, CommandContext},
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
            api::CommandExecutor,
        },
        reovim_kernel::api::{
            ServiceRegistry,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, CommandId as KernelCommandId,
                EventBus, HistoryRing, KernelContext, MarkBank, ModeId, ModeStack, ModuleId,
                MotionEngine, OptionRegistry, RegisterBank, RwLock, TextObjectEngine,
            },
        },
        std::{collections::HashMap, sync::Arc},
    };

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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    struct StubExecutor;

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl CommandExecutor for StubExecutor {
        fn execute(
            &self,
            _cmd: &KernelCommandId,
            _ctx: &CommandContext,
            _kernel: &KernelContext,
        ) -> Option<CommandResult> {
            Some(CommandResult::Success)
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
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
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl TestState {
        fn with_window(buffer_id: BufferId) -> Self {
            let home_mode = test_mode();
            let mut state = Self {
                session: Session::new(ClientId::new(1), home_mode.clone()),
                mode_stack: ModeStack::new(home_mode),
                windows: WindowLayout::empty(),
                extensions: ExtensionMap::new(),
                compositor: None,
                tabs: reovim_driver_session::TabPageSet::new(),
                registers: RegisterBank::new(),
                clipboard_history: HistoryRing::new(),
                local_marks: MarkBank::new(),
            };
            let mut window = Window::new();
            window.buffer_id = Some(buffer_id);
            state.windows.add(window);
            state.session.set_active_buffer(Some(buffer_id));
            state
        }

        fn runtime<'a>(
            &'a mut self,
            kernel: &'a KernelContext,
            executor: &'a StubExecutor,
        ) -> SessionRuntime<'a> {
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
                },
                kernel,
                executor,
            )
        }
    }

    // =========================================================================
    // DeleteChar tests
    // =========================================================================

    #[test]
    fn test_delete_char_id() {
        let cmd = DeleteChar;
        assert_eq!(cmd.id().name(), "delete-char");
    }

    #[test]
    fn test_delete_char_description() {
        let cmd = DeleteChar;
        assert_eq!(cmd.description(), "Delete character under cursor");
    }

    #[test]
    fn test_delete_char_args() {
        let cmd = DeleteChar;
        let args = cmd.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_delete_char_no_buffer_returns_error() {
        let kernel = KernelContext::default();
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
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
            },
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = DeleteChar.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_delete_char_no_window_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
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
            },
            &kernel,
            &executor,
        );
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = DeleteChar.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_delete_char_single() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("ello"));
    }

    #[test]
    fn test_delete_char_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(3));

        let result = DeleteChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("lo"));
    }

    #[test]
    fn test_delete_char_count_clamped_to_eol() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hi");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(100));

        let result = DeleteChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some(""));
    }

    #[test]
    fn test_delete_char_empty_line_is_noop() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteChar.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_delete_char_at_eol_is_noop() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("ab");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 2).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("ab"));
    }

    // =========================================================================
    // DeleteCharBefore tests
    // =========================================================================

    #[test]
    fn test_delete_char_before_id() {
        let cmd = DeleteCharBefore;
        assert_eq!(cmd.id().name(), "delete-char-before");
    }

    #[test]
    fn test_delete_char_before_description() {
        let cmd = DeleteCharBefore;
        assert_eq!(cmd.description(), "Delete character before cursor");
    }

    #[test]
    fn test_delete_char_before_args() {
        let cmd = DeleteCharBefore;
        let args = cmd.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_delete_char_before_no_buffer_returns_error() {
        let kernel = KernelContext::default();
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
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
            },
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = DeleteCharBefore.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_delete_char_before_no_window_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
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
            },
            &kernel,
            &executor,
        );
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = DeleteCharBefore.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_delete_char_before_at_col_zero_first_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Cursor at (0, 0) - can't delete before first line
        let result = DeleteCharBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("hello"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_delete_char_before_joins_with_previous_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line one\nline two");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        // Set cursor to start of second line
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(1, 0).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteCharBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Lines should be joined
        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 1);
        assert_eq!(buf_read.line(0), Some("line oneline two"));
        drop(buf_read);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_delete_char_before_single() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 3).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteCharBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("helo"));

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 2);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_delete_char_before_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 4).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(3));

        let result = DeleteCharBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("ho"));

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 1);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_delete_char_before_count_clamped() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 2).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(100));

        let result = DeleteCharBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("llo"));

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    // =========================================================================
    // DeleteLine tests
    // =========================================================================

    #[test]
    fn test_delete_line_id() {
        let cmd = DeleteLine;
        assert_eq!(cmd.id().name(), "delete-line");
    }

    #[test]
    fn test_delete_line_description() {
        let cmd = DeleteLine;
        assert_eq!(cmd.description(), "Delete current line");
    }

    #[test]
    fn test_delete_line_args() {
        let cmd = DeleteLine;
        let args = cmd.args();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
        assert_eq!(args[1].name, "register");
        assert_eq!(args[1].kind, ArgKind::Register);
    }

    #[test]
    fn test_delete_line_no_buffer_returns_error() {
        let kernel = KernelContext::default();
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
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
            },
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = DeleteLine.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_delete_line_empty_buffer() {
        let kernel = create_test_context();
        let buffer = Buffer::new();
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteLine.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_delete_line_single_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("only line");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Check register has deleted content (per-client registers, #515)
        drop(runtime);
        let content = state.registers.get().clone();
        assert!(content.is_linewise());
        assert_eq!(content.text, "only line\n");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_delete_line_middle_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(1, 0).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 2);
        assert_eq!(buf_read.line(0), Some("line 1"));
        assert_eq!(buf_read.line(1), Some("line 3"));
        drop(buf_read);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_delete_line_last_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(2, 0).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 2);
        assert_eq!(buf_read.line(0), Some("line 1"));
        assert_eq!(buf_read.line(1), Some("line 2"));
        drop(buf_read);
    }

    #[test]
    fn test_delete_line_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3\nline 4");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));

        let result = DeleteLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 2);
        assert_eq!(buf_read.line(0), Some("line 3"));
        assert_eq!(buf_read.line(1), Some("line 4"));
        drop(buf_read);

        // Check register content (per-client registers, #515)
        drop(runtime);
        let content = state.registers.get().clone();
        assert!(content.is_linewise());
        assert_eq!(content.text, "line 1\nline 2\n");
    }

    #[test]
    fn test_delete_line_with_indented_remaining() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("first\n    indented\nthird");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Cursor should be at first non-blank of remaining line
        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 4); // First non-blank in "    indented"
    }

    // =========================================================================
    // DeleteToEndOfLine tests
    // =========================================================================

    #[test]
    fn test_delete_to_eol_id() {
        let cmd = DeleteToEndOfLine;
        assert_eq!(cmd.id().name(), "delete-to-eol");
    }

    #[test]
    fn test_delete_to_eol_description() {
        let cmd = DeleteToEndOfLine;
        assert_eq!(cmd.description(), "Delete to end of line");
    }

    #[test]
    fn test_delete_to_eol_args() {
        let cmd = DeleteToEndOfLine;
        let args = cmd.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "register");
        assert_eq!(args[0].kind, ArgKind::Register);
    }

    #[test]
    fn test_delete_to_eol_no_buffer_returns_error() {
        let kernel = KernelContext::default();
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
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
            },
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = DeleteToEndOfLine.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_delete_to_eol_no_window_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();
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
            },
            &kernel,
            &executor,
        );
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = DeleteToEndOfLine.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_delete_to_eol_from_start() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteToEndOfLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some(""));

        // Check register content (per-client registers, #515)
        drop(runtime);
        let content = state.registers.get().clone();
        assert!(!content.is_linewise());
        assert_eq!(content.text, "hello world");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_delete_to_eol_from_middle() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 5).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteToEndOfLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("hello"));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_delete_to_eol_at_eol_is_noop() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hi");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 2).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteToEndOfLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("hi"));
    }

    // =========================================================================
    // DeleteLine - delete all content from first line (else branch)
    // =========================================================================

    #[test]
    fn test_delete_line_all_from_first_line() {
        // This tests the else branch at lines 222-228:
        // Deleting to end of buffer from the first line - delete just the content
        let kernel = create_test_context();
        let buffer = Buffer::from_string("only line here");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Check register has the deleted content (per-client registers, #515)
        drop(runtime);
        let content = state.registers.get().clone();
        assert!(content.is_linewise());
        assert_eq!(content.text, "only line here\n");
    }

    #[test]
    fn test_delete_line_all_lines_from_first_with_count() {
        // Delete all lines from the first line using count
        let kernel = create_test_context();
        let buffer = Buffer::from_string("first\nsecond\nthird");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(10)); // More than available lines

        let result = DeleteLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        // All lines should be deleted (per-client registers, #515)
        drop(runtime);
        let content = state.registers.get().clone();
        assert!(content.is_linewise());
        assert_eq!(content.text, "first\nsecond\nthird\n");
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_delete_line_cursor_beyond_buffer_end() {
        // Cursor at line 5 but buffer has only 2 lines: lines_to_delete = 0
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(5, 0).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = DeleteLine.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Buffer should remain unchanged
        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 2);
        assert_eq!(buf_read.line(0), Some("hello"));
        assert_eq!(buf_read.line(1), Some("world"));
        drop(buf_read);
    }
}
