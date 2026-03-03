//! Change commands.
//!
//! Provides change commands that delete text and enter insert mode:
//! - `ChangeLine` (cc)
//! - `ChangeToEndOfLine` (C)
//!
//! # Epic #372 - Mode Ownership
//!
//! These commands use `VimMode::INSERT_ID` to transition to insert mode after
//! deleting, which is why they belong in the vim module.

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{BufferApi, SessionRuntime, TransitionContext, api::ModeApi},
    reovim_kernel::api::v1::{CommandId, Position, RegisterContent},
};

/// Helper to get cursor position from the active window.
fn get_cursor_position(runtime: &SessionRuntime<'_>) -> Option<Position> {
    let window = runtime.windows().active()?;
    Some(Position::new(window.cursor.line, window.cursor.column))
}

/// Helper to set cursor position on the active window.
#[cfg_attr(coverage_nightly, coverage(off))]
fn set_cursor_position(runtime: &mut SessionRuntime<'_>, pos: Position) {
    if let Some(window) = runtime.windows_mut().active_mut() {
        window.cursor = pos.into();
    }
}

use crate::{ids, modes::VimMode};

/// Change current line (cc).
///
/// Clears the content of the current line(s) and enters insert mode.
/// Unlike `dd`, this keeps the line(s) but empties their content.
/// The deleted text is stored in the register as linewise.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangeLine;

impl Command for ChangeLine {
    fn id(&self) -> CommandId {
        ids::CHANGE_LINE
    }

    fn description(&self) -> &'static str {
        "Change current line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::optional("count", ArgKind::Count, "Number of lines to change"),
            ArgSpec::optional("register", ArgKind::Register, "Target register"),
        ]
    }
}

impl CommandHandler for ChangeLine {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let count = args.count().unwrap_or(1);
        let start_line = get_cursor_position(runtime).map_or(0, |p| p.line);
        let line_count = runtime.buffer_line_count(buffer_id).unwrap_or(0);

        if line_count == 0 {
            // Empty buffer - just enter insert mode
            runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());
            return CommandResult::Success;
        }

        // Calculate lines to change
        let lines_to_change = count.min(line_count.saturating_sub(start_line));
        if lines_to_change == 0 {
            runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());
            return CommandResult::Success;
        }

        // Collect text to delete for register (include newlines between lines)
        let mut deleted_text = String::new();
        for i in 0..lines_to_change {
            let line_idx = start_line + i;
            if let Some(line) = runtime.buffer_line(buffer_id, line_idx) {
                deleted_text.push_str(&line);
            }
            if i < lines_to_change - 1 {
                deleted_text.push('\n');
            }
        }
        deleted_text.push('\n'); // Linewise content ends with newline

        // Store in register with clipboard sync (#515)
        let content = RegisterContent::linewise(deleted_text);
        let register = args.register();
        runtime.store_register_with_sync(register, content);

        // For cc: if changing multiple lines, delete all but first, then clear first
        // Single line: just clear the content

        if lines_to_change == 1 {
            // Clear the single line content
            let line_len = runtime.buffer_line_len(buffer_id, start_line).unwrap_or(0);
            if line_len > 0 {
                let delete_start = Position::new(start_line, 0);
                let delete_end = Position::new(start_line, line_len);
                runtime.delete_range(buffer_id, delete_start, delete_end);
                set_cursor_position(runtime, Position::new(start_line, 0));
            }
            runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());
            return CommandResult::Success;
        }

        // Multiple lines: delete all content from first line to end of last changed line
        // Then keep one empty line at start_line
        let last_changed_line = start_line + lines_to_change - 1;
        let last_line_len = runtime
            .buffer_line_len(buffer_id, last_changed_line)
            .unwrap_or(0);
        let end_line = start_line + lines_to_change;

        let (delete_start, delete_end) = if end_line >= line_count {
            // Changing to end of buffer - delete from start of first line to end of last line
            (Position::new(start_line, 0), Position::new(last_changed_line, last_line_len))
        } else {
            // Normal case: delete from start of first line to start of line after changed range
            (Position::new(start_line, 0), Position::new(end_line, 0))
        };

        runtime.delete_range(buffer_id, delete_start, delete_end);
        set_cursor_position(runtime, Position::new(start_line, 0));
        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Change to end of line (C).
///
/// Deletes from cursor to end of line and enters insert mode.
/// The deleted text is stored in the register as characterwise.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangeToEndOfLine;

impl Command for ChangeToEndOfLine {
    fn id(&self) -> CommandId {
        ids::CHANGE_TO_EOL
    }

    fn description(&self) -> &'static str {
        "Change to end of line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "register",
            ArgKind::Register,
            "Target register",
        )]
    }
}

impl CommandHandler for ChangeToEndOfLine {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let pos = get_cursor_position(runtime).unwrap_or_else(|| Position::new(0, 0));
        let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);

        // Nothing to delete if at or past end of line - just enter insert mode
        if pos.column >= line_len {
            runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());
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
        let delete_end = Position::new(pos.line, line_len);
        runtime.delete_range(buffer_id, pos, delete_end);

        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}

#[cfg(test)]
#[allow(clippy::significant_drop_tightening)]
mod tests {
    use {
        super::*,
        reovim_driver_command::Command,
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, SessionRuntime, WindowLayout, api::CommandExecutor,
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

    use reovim_driver_session::api::ModeApi;

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
        fn execute(
            &self,
            _: &reovim_kernel::api::v1::CommandId,
            _: &CommandContext,
            _: &KernelContext,
        ) -> Option<CommandResult> {
            Some(CommandResult::Success)
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
    fn test_change_line_command_id() {
        let cmd = ChangeLine;
        assert_eq!(cmd.id(), ids::CHANGE_LINE);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_description() {
        let cmd = ChangeLine;
        assert!(cmd.description().contains("Change"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_args() {
        let cmd = ChangeLine;
        let args = cmd.args();
        assert_eq!(args.len(), 2); // count and register
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_command_id() {
        let cmd = ChangeToEndOfLine;
        assert_eq!(cmd.id(), ids::CHANGE_TO_EOL);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_description() {
        let cmd = ChangeToEndOfLine;
        assert!(cmd.description().contains("end of line"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_args() {
        let cmd = ChangeToEndOfLine;
        let args = cmd.args();
        assert_eq!(args.len(), 1); // register only
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_debug() {
        let cmd = ChangeLine;
        let debug = format!("{cmd:?}");
        assert!(debug.contains("ChangeLine"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_debug() {
        let cmd = ChangeToEndOfLine;
        let debug = format!("{cmd:?}");
        assert!(debug.contains("ChangeToEndOfLine"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_default() {
        let _ = ChangeLine;
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_default() {
        let _ = ChangeToEndOfLine;
    }

    // ========================================================================
    // Execute tests - ChangeLine
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_no_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_single_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());

        // Line content should be cleared
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &[""]);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_multi_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("line1\nline2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", reovim_driver_command::ArgValue::Count(2));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_empty_buffer() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
    }

    // ========================================================================
    // Execute tests - ChangeToEndOfLine
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_no_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = ChangeToEndOfLine.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_from_middle() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Set cursor at column 5 (on space)
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 5).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ChangeToEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());

        // Buffer should have "hello" (everything from col 5 deleted)
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["hello"]);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_at_end_of_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Set cursor at end of line
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 5).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ChangeToEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());

        // Buffer should be unchanged (already at end of line)
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["hello"]);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_from_start() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeToEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Entire line content should be deleted
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &[""]);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_count_exceeds_buffer() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", reovim_driver_command::ArgValue::Count(99));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_last_two_lines() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("a\nb\nc");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", reovim_driver_command::ArgValue::Count(2));

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Start on line 1
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(1, 0).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    // ========================================================================
    // Additional ChangeLine execute tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_with_count_one_explicit() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", reovim_driver_command::ArgValue::Count(1));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Only first line should be changed
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        // First line should be cleared, second line should remain
        assert_eq!(buf.lines()[0], "");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_multi_line_three_lines() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("aaa\nbbb\nccc\nddd");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", reovim_driver_command::ArgValue::Count(3));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_with_register() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("test line");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("register", reovim_driver_command::ArgValue::Register('a'));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_cursor_at_middle_of_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 5).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Cursor should be at start after change
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    // ========================================================================
    // Additional ChangeToEndOfLine execute tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_empty_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeToEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_with_register() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("register", reovim_driver_command::ArgValue::Register('b'));

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 5).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ChangeToEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_past_end_of_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("abc");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Cursor past end of line
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 10).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ChangeToEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        // Should enter insert mode without deleting
        assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_single_char_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("x");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeToEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &[""]);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_multiline_only_affects_current() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 3).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ChangeToEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        // First line should be truncated, second line untouched
        assert_eq!(buf.lines()[0], "hel");
        assert_eq!(buf.lines()[1], "world");
    }

    // ========================================================================
    // Clone tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_clone() {
        let cmd = ChangeLine;
        let cloned = cmd;
        assert_eq!(cloned.id(), ids::CHANGE_LINE);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_to_eol_clone() {
        let cmd = ChangeToEndOfLine;
        let cloned = cmd;
        assert_eq!(cloned.id(), ids::CHANGE_TO_EOL);
    }

    // ========================================================================
    // ChangeLine multi-line end_line >= line_count tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_last_lines_of_buffer() {
        // Tests the branch: end_line >= line_count (changing to end of buffer)
        let ctx = create_test_context();
        let buffer = Buffer::from_string("a\nb\nc");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", reovim_driver_command::ArgValue::Count(3));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
        assert_eq!(runtime.current_mode().name(), VimMode::INSERT_ID.name());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_two_of_two_from_start() {
        // Change all lines (count=2 from line 0 of 2-line buffer)
        // This triggers: end_line (0+2=2) >= line_count (2)
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", reovim_driver_command::ArgValue::Count(2));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_multi_from_middle_to_end() {
        // Change from line 2 with count 3, but only 2 lines remain
        // This triggers: end_line >= line_count
        let ctx = create_test_context();
        let buffer = Buffer::from_string("a\nb\nc\nd");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", reovim_driver_command::ArgValue::Count(3));

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(2, 0).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_multi_not_at_end() {
        // Change 2 lines from middle (NOT at end of buffer) - the normal case branch
        let ctx = create_test_context();
        let buffer = Buffer::from_string("a\nb\nc\nd\ne");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", reovim_driver_command::ArgValue::Count(2));

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(1, 0).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = ChangeLine.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Lines b and c should be deleted, replaced with cursor at line 1
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 1);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_line_registers_deleted_text() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", reovim_driver_command::ArgValue::Count(2));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        ChangeLine.execute(&mut runtime, &args);
        drop(runtime);

        // Check the register has the deleted text
        let reg = state.registers.get();
        assert!(reg.text.contains("hello"));
        assert!(reg.text.contains("world"));
    }
}
