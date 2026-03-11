//! Replace commands.
//!
//! Provides replace and repeat commands:
//! - `ReplaceCharStart` (r) - signals waiting for char
//! - `ReplaceChar` - performs the actual replacement
//! - `RepeatDot` (.)
//! - `JoinLines` (J)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{
            SessionRuntime, api::BufferApi},
    reovim_kernel::api::v1::{CommandId, Position},
};

use crate::ids;

/// Start replace char operation (r).
///
/// This command signals that the next character typed should replace
/// the character(s) under the cursor. Returns `WaitingForChar` with
/// the `ReplaceChar` operation type.
///
/// Unlike `R` (replace mode), `r` is a single-character replacement:
/// - `rx` replaces the char under cursor with 'x'
/// - `3rx` replaces the next 3 chars with 'x'
#[derive(Debug, Clone, Copy, Default)]
pub struct ReplaceCharStart;

impl Command for ReplaceCharStart {
    fn id(&self) -> CommandId {
        ids::REPLACE_CHAR_START
    }

    fn description(&self) -> &'static str {
        "Replace character under cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of characters to replace",
        )]
    }
}

impl CommandHandler for ReplaceCharStart {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        // Will signal waiting for replace character with count
        let _count = args.count().unwrap_or(1);
        CommandResult::Success
    }
}

/// Replace character under cursor (r{char}).
///
/// The resolver intercepts `r`, waits for the next character, then
/// dispatches this command with `replace_char` in the context metadata.
///
/// Behavior:
/// - `rx` replaces the char under cursor with 'x'
/// - `3rx` replaces the next 3 chars with 'x'
/// - At end of line: replaces only available chars up to line end
#[derive(Debug, Clone, Copy, Default)]
pub struct ReplaceChar;

impl Command for ReplaceChar {
    fn id(&self) -> CommandId {
        ids::REPLACE_CHAR
    }

    fn description(&self) -> &'static str {
        "Replace character(s) under cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::required("replace_char", ArgKind::Char, "Replacement character"),
            ArgSpec::optional("count", ArgKind::Count, "Number of characters to replace"),
        ]
    }
}

impl CommandHandler for ReplaceChar {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(replacement) = args.char("replace_char") else {
            return CommandResult::error("No replacement character");
        };

        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let cursor = Position::new(window.cursor.line, window.cursor.column);

        let count = args.count().unwrap_or(1);

        let line_len = runtime.buffer_line_len(buffer_id, cursor.line).unwrap_or(0);
        if line_len == 0 {
            return CommandResult::Success;
        }

        // Clamp count to remaining chars on the line
        let available = line_len.saturating_sub(cursor.column);
        let actual_count = count.min(available);
        if actual_count == 0 {
            return CommandResult::Success;
        }

        // Delete `actual_count` chars at cursor
        let delete_end = Position::new(cursor.line, cursor.column + actual_count);
        runtime.delete_range(buffer_id, cursor, delete_end);

        // Insert `actual_count` copies of replacement char
        let replacement_text: String = std::iter::repeat_n(replacement, actual_count).collect();
        runtime.insert_text(buffer_id, cursor, &replacement_text);

        // Cursor stays at original position (Vim behavior: cursor doesn't move on `r`)
        CommandResult::Success
    }
}

/// Repeat the last repeatable command (.).
///
/// This command returns `RepeatAction` which signals the runner to
/// replay the last repeatable command from `repeat_state`. Repeatable
/// commands include text-modifying operations like insert, delete, change.
///
/// # Vim Behavior
///
/// - `.` repeats the last change command
/// - `3.` repeats the last change 3 times
/// - Insert mode text is recorded and replayed
#[derive(Debug, Clone, Copy, Default)]
pub struct RepeatDot;

impl Command for RepeatDot {
    fn id(&self) -> CommandId {
        ids::REPEAT_DOT
    }

    fn description(&self) -> &'static str {
        "Repeat last change"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of times to repeat",
        )]
    }
}

impl CommandHandler for RepeatDot {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        // Will signal intent to repeat last change
        CommandResult::Success
    }
}

/// Join current line with next line (J).
#[derive(Debug, Clone, Copy, Default)]
pub struct JoinLines;

impl Command for JoinLines {
    fn id(&self) -> CommandId {
        ids::JOIN_LINES
    }

    fn description(&self) -> &'static str {
        "Join current line with next line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines to join",
        )]
    }
}

impl CommandHandler for JoinLines {
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
        let current_line = pos.line;

        let Some(mut line_count) = runtime.buffer_line_count(buffer_id) else {
            return CommandResult::error("Failed to get line count");
        };

        let mut joined_any = false;

        for _ in 0..count {
            // Can't join if on last line
            if current_line + 1 >= line_count {
                break;
            }

            // Get line length via BufferApi
            let line_len = runtime
                .buffer_line_len(buffer_id, current_line)
                .unwrap_or(0);

            // Delete newline (joins the lines) - delete from end of current line to start of next
            let newline_start = Position::new(current_line, line_len);
            let newline_end = Position::new(current_line + 1, 0);
            runtime.delete_range(buffer_id, newline_start, newline_end);

            // Get the joined line content to find leading whitespace
            let joined_line = runtime
                .buffer_line(buffer_id, current_line)
                .unwrap_or_default();
            let after_join = if joined_line.len() > line_len {
                &joined_line[line_len..]
            } else {
                ""
            };
            let leading_ws = after_join.chars().take_while(|c| c.is_whitespace()).count();

            // Delete leading whitespace of what was the next line
            if leading_ws > 0 {
                let ws_start = Position::new(current_line, line_len);
                let ws_end = Position::new(current_line, line_len + leading_ws);
                runtime.delete_range(buffer_id, ws_start, ws_end);
            }

            // Insert single space between joined content (Vim behavior)
            // Only if there's content after the join point
            let new_line_len = runtime
                .buffer_line_len(buffer_id, current_line)
                .unwrap_or(0);
            if new_line_len > line_len {
                let insert_pos = Position::new(current_line, line_len);
                runtime.insert_text(buffer_id, insert_pos, " ");
            }

            // Update line count for next iteration
            line_count = runtime.buffer_line_count(buffer_id).unwrap_or(line_count);
            joined_any = true;
        }

        if !joined_any {
            return CommandResult::Success;
        }

        // Position cursor at join point
        let final_line_len = runtime
            .buffer_line_len(buffer_id, current_line)
            .unwrap_or(0);
        let cursor_pos =
            Position::new(current_line, pos.column.min(final_line_len.saturating_sub(1)));
        // Update cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = cursor_pos.into();
        }

        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use {
        reovim_kernel::testing::{create_test_context, test_mode},
        super::*,
        reovim_driver_command::{ArgValue, CommandContext},
        reovim_driver_session::{
            testing::StubExecutor,
            ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
        },
        reovim_kernel::api::v1::{
                Buffer, BufferId, HistoryRing, KernelContext, MarkBank, ModeStack, RegisterBank,
            },
    };

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
                active_buffer: None,
                terminal_size: (80, 24),
            };
            let mut window = Window::new();
            window.buffer_id = Some(buffer_id);
            state.windows.add(window);
            state.active_buffer = Some(buffer_id);
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
                    active_buffer: &mut self.active_buffer,
                    terminal_size: &mut self.terminal_size,
                },
                kernel,
                executor,
            )
        }
    }

    // =========================================================================
    // ReplaceCharStart tests
    // =========================================================================

    #[test]
    fn test_replace_char_start_id() {
        let cmd = ReplaceCharStart;
        assert_eq!(cmd.id().name(), "replace-char-start");
    }

    #[test]
    fn test_replace_char_start_description() {
        let cmd = ReplaceCharStart;
        assert_eq!(cmd.description(), "Replace character under cursor");
    }

    #[test]
    fn test_replace_char_start_args() {
        let cmd = ReplaceCharStart;
        let args = cmd.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    // =========================================================================
    // RepeatDot tests
    // =========================================================================

    #[test]
    fn test_repeat_dot_id() {
        let cmd = RepeatDot;
        assert_eq!(cmd.id().name(), "repeat-dot");
    }

    #[test]
    fn test_repeat_dot_description() {
        let cmd = RepeatDot;
        assert_eq!(cmd.description(), "Repeat last change");
    }

    #[test]
    fn test_repeat_dot_args() {
        let cmd = RepeatDot;
        let args = cmd.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    // =========================================================================
    // JoinLines tests
    // =========================================================================

    #[test]
    fn test_join_lines_id() {
        let cmd = JoinLines;
        assert_eq!(cmd.id().name(), "join-lines");
    }

    #[test]
    fn test_join_lines_description() {
        let cmd = JoinLines;
        assert_eq!(cmd.description(), "Join current line with next line");
    }

    #[test]
    fn test_join_lines_args() {
        let cmd = JoinLines;
        let args = cmd.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_join_lines_no_buffer_returns_error() {
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
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_join_lines_no_window_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2");
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
            &kernel,
            &executor,
        );
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_join_lines_two_lines() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 1);
        assert_eq!(buf_read.line(0), Some("hello world"));
        drop(buf_read);
    }

    #[test]
    fn test_join_lines_strips_leading_whitespace() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello\n    world");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 1);
        assert_eq!(buf_read.line(0), Some("hello world"));
        drop(buf_read);
    }

    #[test]
    fn test_join_lines_on_last_line_is_noop() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("only line");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 1);
        assert_eq!(buf_read.line(0), Some("only line"));
        drop(buf_read);
    }

    #[test]
    fn test_join_lines_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3\nline 4");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));

        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 2);
        // First two joins happened: "line 1" + "line 2" + "line 3"
        assert_eq!(buf_read.line(0), Some("line 1 line 2 line 3"));
        assert_eq!(buf_read.line(1), Some("line 4"));
        drop(buf_read);
    }

    #[test]
    fn test_join_lines_count_exceeds_remaining() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(10));

        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 1);
        assert_eq!(buf_read.line(0), Some("line 1 line 2"));
        drop(buf_read);
    }

    #[test]
    fn test_join_lines_buffer_not_found() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        // Use a fake buffer_id that doesn't exist
        let mut args = CommandContext::new();
        args.set("buffer_id", ArgValue::BufferId(99999));

        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_join_lines_no_content_after_join_point() {
        // Tests the branch at line 179: new_line_len > line_len check
        // where there's no content after joining (empty next line)
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello\n");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 1);
        // Empty second line joined, no space should be inserted
        assert_eq!(buf_read.line(0), Some("hello"));
        drop(buf_read);
    }

    #[test]
    fn test_replace_char_start_execute() {
        // Execute returns Success even though it's a stub
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        let result = ReplaceCharStart.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_repeat_dot_execute() {
        // Execute returns Success even though it's a stub
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        let result = RepeatDot.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    // =========================================================================
    // Metadata/Default tests
    // =========================================================================

    // =========================================================================
    // ReplaceCharStart with count
    // =========================================================================

    #[test]
    fn test_replace_char_start_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(3));
        let result = ReplaceCharStart.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    // =========================================================================
    // JoinLines cursor position after join
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_join_lines_cursor_position() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 3).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line(0), Some("hello world"));
        drop(buf_read);

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Cursor should be at min(original_col, final_line_len - 1)
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 3);
    }

    // =========================================================================
    // JoinLines with multiple empty lines
    // =========================================================================

    #[test]
    fn test_join_lines_with_empty_next_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello\n\nworld");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 2);
        // Empty line joined with no trailing space (no content after join)
        assert_eq!(buf_read.line(0), Some("hello"));
        assert_eq!(buf_read.line(1), Some("world"));
        drop(buf_read);
    }

    // =========================================================================
    // JoinLines with multiple joins (count=3 on 4 lines)
    // =========================================================================

    #[test]
    fn test_join_lines_all_lines() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("a\nb\nc\nd");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(3));

        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 1);
        assert_eq!(buf_read.line(0), Some("a b c d"));
        drop(buf_read);
    }

    // =========================================================================
    // JoinLines from non-first line
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_join_lines_from_middle() {
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

        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 2);
        assert_eq!(buf_read.line(0), Some("line 1"));
        assert_eq!(buf_read.line(1), Some("line 2 line 3"));
        drop(buf_read);
    }

    // =========================================================================
    // JoinLines on last line of multi-line buffer
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_join_lines_on_last_line_multiline() {
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

        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Should be no-op since cursor is on last line
        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 3);
        drop(buf_read);
    }

    // =========================================================================
    // RepeatDot with count
    // =========================================================================

    #[test]
    fn test_repeat_dot_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set("count", ArgValue::Count(5));
        let result = RepeatDot.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    // =========================================================================
    // JoinLines with tab-indented next line
    // =========================================================================

    #[test]
    fn test_join_lines_strips_tab_whitespace() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello\n\t\tworld");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = JoinLines.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 1);
        assert_eq!(buf_read.line(0), Some("hello world"));
        drop(buf_read);
    }

    // =========================================================================
    // ReplaceChar tests
    // =========================================================================

    #[test]
    fn test_replace_char_id() {
        let cmd = ReplaceChar;
        assert_eq!(cmd.id().name(), "replace-char");
    }

    #[test]
    fn test_replace_char_description() {
        let cmd = ReplaceChar;
        assert_eq!(cmd.description(), "Replace character(s) under cursor");
    }

    #[test]
    fn test_replace_char_args() {
        let cmd = ReplaceChar;
        let args = cmd.args();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].name, "replace_char");
        assert_eq!(args[0].kind, ArgKind::Char);
        assert_eq!(args[1].name, "count");
        assert_eq!(args[1].kind, ArgKind::Count);
    }

    #[test]
    fn test_replace_char_no_char_arg_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = ReplaceChar.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_replace_char_no_buffer_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set("replace_char", ArgValue::Char('x'));
        let result = ReplaceChar.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_replace_char_no_window_error() {
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
            &kernel,
            &executor,
        );
        let mut args = CommandContext::new();
        args.set("replace_char", ArgValue::Char('x'));
        args.set_buffer_id(buffer_id);
        let result = ReplaceChar.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_replace_char_single() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("replace_char", ArgValue::Char('x'));

        let result = ReplaceChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line(0), Some("xello"));
        drop(buf_read);
    }

    #[test]
    fn test_replace_char_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("replace_char", ArgValue::Char('z'));
        args.set("count", ArgValue::Count(3));

        let result = ReplaceChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line(0), Some("zzzlo"));
        drop(buf_read);
    }

    #[test]
    fn test_replace_char_count_clamps_to_line_end() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hi");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("replace_char", ArgValue::Char('x'));
        args.set("count", ArgValue::Count(10));

        let result = ReplaceChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        // Count clamped to 2 (line length)
        assert_eq!(buf_read.line(0), Some("xx"));
        drop(buf_read);
    }

    #[test]
    fn test_replace_char_on_empty_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("replace_char", ArgValue::Char('x'));

        let result = ReplaceChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        // No change on empty buffer (line_len == 0 early return)
        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 0);
        drop(buf_read);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_replace_char_at_column_offset() {
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
        args.set("replace_char", ArgValue::Char('X'));

        let result = ReplaceChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line(0), Some("helXo"));
        drop(buf_read);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_replace_char_at_end_of_line() {
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
        args.set("replace_char", ArgValue::Char('!'));

        let result = ReplaceChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line(0), Some("hell!"));
        drop(buf_read);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_replace_char_cursor_past_end() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hi");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 5).into(); // Past end
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("replace_char", ArgValue::Char('x'));

        let result = ReplaceChar.execute(&mut runtime, &args);
        assert!(result.is_success());

        // No change since cursor is past end
        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line(0), Some("hi"));
        drop(buf_read);
    }
}
