//! Display line movement commands.
//!
//! Provides cursor movement based on display (visual) lines rather than buffer lines.
//! When text wraps across multiple terminal lines, gj/gk move one visual line
//! rather than one buffer line.
//!
//! These commands are unicode-aware and handle:
//! - Tab characters (expand based on `tabstop` setting)
//! - CJK double-width characters (take 2 display columns)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{BufferApi, ChangeTracker, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, OptionScopeId, Position},
};

use {super::super::display_lines, crate::ids};

/// Get the tabstop setting for a buffer.
fn get_tabstop(runtime: &SessionRuntime<'_>, buffer_id: reovim_kernel::api::v1::BufferId) -> usize {
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    runtime
        .kernel()
        .options
        .get("tabstop", OptionScopeId::Buffer(buffer_id))
        .and_then(|v| v.as_int())
        .map_or(8, |n| n.max(1) as usize)
}

/// Move cursor down one display line (gj).
///
/// When text wraps across multiple terminal lines, this moves down one
/// visual line rather than one buffer line. On unwrapped lines, behaves
/// like regular `j`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorDisplayDown;

impl Command for CursorDisplayDown {
    fn id(&self) -> CommandId {
        ids::CURSOR_DISPLAY_DOWN
    }

    fn description(&self) -> &'static str {
        "Move cursor down one display line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of display lines",
        )]
    }
}

impl CommandHandler for CursorDisplayDown {
    #[allow(clippy::cast_possible_truncation)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let old_pos = Position::new(window.cursor.line, window.cursor.column);

        let Some(line_count) = runtime.buffer_line_count(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Terminal width: use 80 as default
        // Note: In the future, this could be retrieved from session state
        // or passed through CommandContext.
        let terminal_width = 80;
        let tabstop = get_tabstop(runtime, buffer_id);

        let count = args.count().unwrap_or(1);

        // Get current line content
        let current_line = runtime
            .buffer_line(buffer_id, old_pos.line)
            .unwrap_or_default();

        // Use unicode-aware functions for proper tab and CJK handling
        let display_lines_in_current =
            display_lines::display_line_count_unicode(&current_line, terminal_width, tabstop);
        let (current_display_line, display_col) = display_lines::display_position_unicode(
            &current_line,
            old_pos.column,
            terminal_width,
            tabstop,
        );

        // Calculate how many display lines we can move within this buffer line
        let remaining_display_lines =
            display_lines_in_current.saturating_sub(current_display_line + 1);

        let new_pos = if count <= remaining_display_lines {
            // Stay on same buffer line, move to next display line
            let target_display_line = current_display_line + count;
            let target_display_col = target_display_line * terminal_width + display_col;
            let new_col = display_lines::buffer_col_from_display_col_unicode(
                &current_line,
                target_display_col,
                tabstop,
            );
            // Clamp to line length
            let line_len = current_line.chars().count();
            let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
            Position::new(old_pos.line, clamped_col)
        } else {
            // Need to move to next buffer line(s)
            let mut lines_to_move = count - remaining_display_lines;
            let mut new_line = old_pos.line + 1;

            while lines_to_move > 0 && new_line < line_count {
                let line = runtime.buffer_line(buffer_id, new_line).unwrap_or_default();
                let display_count =
                    display_lines::display_line_count_unicode(&line, terminal_width, tabstop);

                if lines_to_move <= display_count {
                    // Target is within this line
                    let target_display = lines_to_move - 1;
                    let target_display_col = target_display * terminal_width + display_col;
                    let new_col = display_lines::buffer_col_from_display_col_unicode(
                        &line,
                        target_display_col,
                        tabstop,
                    );
                    let line_len = line.chars().count();
                    let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
                    let target_pos = Position::new(new_line, clamped_col);
                    // Update cursor via per-client Window (#471)
                    if let Some(window) = runtime.windows_mut().active_mut() {
                        window.cursor = target_pos.into();
                    }

                    runtime.record_cursor_move(buffer_id);

                    return CommandResult::Success;
                }
                lines_to_move -= display_count;
                new_line += 1;
            }

            // Reached end of buffer - go to last line, last display line
            let last_line = line_count.saturating_sub(1);
            let last_content = runtime
                .buffer_line(buffer_id, last_line)
                .unwrap_or_default();
            let last_display_count =
                display_lines::display_line_count_unicode(&last_content, terminal_width, tabstop);
            let target_display = last_display_count.saturating_sub(1);
            let target_display_col = target_display * terminal_width + display_col;
            let new_col = display_lines::buffer_col_from_display_col_unicode(
                &last_content,
                target_display_col,
                tabstop,
            );
            let line_len = last_content.chars().count();
            let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
            Position::new(last_line, clamped_col)
        };

        // Update cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = new_pos.into();
        }

        runtime.record_cursor_move(buffer_id);

        CommandResult::Success
    }
}

/// Move cursor up one display line (gk).
///
/// When text wraps across multiple terminal lines, this moves up one
/// visual line rather than one buffer line. On unwrapped lines, behaves
/// like regular `k`.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorDisplayUp;

impl Command for CursorDisplayUp {
    fn id(&self) -> CommandId {
        ids::CURSOR_DISPLAY_UP
    }

    fn description(&self) -> &'static str {
        "Move cursor up one display line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of display lines",
        )]
    }
}

impl CommandHandler for CursorDisplayUp {
    #[allow(clippy::cast_possible_truncation)]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let old_pos = Position::new(window.cursor.line, window.cursor.column);

        // Terminal width: use 80 as default
        // Note: In the future, this could be retrieved from session state
        // or passed through CommandContext.
        let terminal_width = 80;
        let tabstop = get_tabstop(runtime, buffer_id);

        let count = args.count().unwrap_or(1);

        // Get current line content
        let current_line = runtime
            .buffer_line(buffer_id, old_pos.line)
            .unwrap_or_default();

        // Use unicode-aware functions for proper tab and CJK handling
        let (current_display_line, display_col) = display_lines::display_position_unicode(
            &current_line,
            old_pos.column,
            terminal_width,
            tabstop,
        );

        let new_pos = if count <= current_display_line {
            // Stay on same buffer line, move to previous display line
            let target_display_line = current_display_line - count;
            let target_display_col = target_display_line * terminal_width + display_col;
            let new_col = display_lines::buffer_col_from_display_col_unicode(
                &current_line,
                target_display_col,
                tabstop,
            );
            // Clamp to line length
            let line_len = current_line.chars().count();
            let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
            Position::new(old_pos.line, clamped_col)
        } else {
            // Need to move to previous buffer line(s)
            let mut lines_to_move = count - current_display_line;
            let mut new_line = old_pos.line;

            while lines_to_move > 0 && new_line > 0 {
                new_line -= 1;
                let line = runtime.buffer_line(buffer_id, new_line).unwrap_or_default();
                let display_count =
                    display_lines::display_line_count_unicode(&line, terminal_width, tabstop);

                if lines_to_move <= display_count {
                    // Target is within this line (from bottom)
                    let target_display = display_count - lines_to_move;
                    let target_display_col = target_display * terminal_width + display_col;
                    let new_col = display_lines::buffer_col_from_display_col_unicode(
                        &line,
                        target_display_col,
                        tabstop,
                    );
                    let line_len = line.chars().count();
                    let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
                    let target_pos = Position::new(new_line, clamped_col);
                    // Update cursor via per-client Window (#471)
                    if let Some(window) = runtime.windows_mut().active_mut() {
                        window.cursor = target_pos.into();
                    }

                    runtime.record_cursor_move(buffer_id);

                    return CommandResult::Success;
                }
                lines_to_move -= display_count;
            }

            // Reached beginning of buffer - go to first line, first display line
            let first_content = runtime.buffer_line(buffer_id, 0).unwrap_or_default();
            let new_col = display_lines::buffer_col_from_display_col_unicode(
                &first_content,
                display_col,
                tabstop,
            );
            let line_len = first_content.chars().count();
            let clamped_col = new_col.min(line_len.saturating_sub(1).max(0));
            Position::new(0, clamped_col)
        };

        // Update cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = new_pos.into();
        }

        runtime.record_cursor_move(buffer_id);

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
                EventBus, KernelContext, MarkBank, ModeId, ModeStack, ModuleId, MotionEngine,
                OptionRegistry, RegisterBank, RwLock, TextObjectEngine,
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
            Arc::new(RwLock::new(RegisterBank::new())),
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
                &mut self.mode_stack,
                &mut self.windows,
                &mut self.extensions,
                kernel,
                executor,
            )
        }
    }

    // =========================================================================
    // CursorDisplayDown tests
    // =========================================================================

    #[test]
    fn test_cursor_display_down_id() {
        let cmd = CursorDisplayDown;
        assert_eq!(cmd.id().name(), "cursor-display-down");
    }

    #[test]
    fn test_cursor_display_down_description() {
        let cmd = CursorDisplayDown;
        assert_eq!(cmd.description(), "Move cursor down one display line");
    }

    #[test]
    fn test_cursor_display_down_args() {
        let cmd = CursorDisplayDown;
        let args = cmd.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_cursor_display_down_no_buffer_returns_error() {
        let kernel = KernelContext::default();
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_cursor_display_down_no_window_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_cursor_display_down_moves_to_next_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_display_down_at_eof() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(1, 0).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    fn test_cursor_display_down_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3\nline 4");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(3));

        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 3);
    }

    #[test]
    fn test_cursor_display_down_buffer_not_found() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set("buffer_id", ArgValue::BufferId(99999));

        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    // =========================================================================
    // CursorDisplayUp tests
    // =========================================================================

    #[test]
    fn test_cursor_display_up_id() {
        let cmd = CursorDisplayUp;
        assert_eq!(cmd.id().name(), "cursor-display-up");
    }

    #[test]
    fn test_cursor_display_up_description() {
        let cmd = CursorDisplayUp;
        assert_eq!(cmd.description(), "Move cursor up one display line");
    }

    #[test]
    fn test_cursor_display_up_args() {
        let cmd = CursorDisplayUp;
        let args = cmd.args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
    }

    #[test]
    fn test_cursor_display_up_no_buffer_returns_error() {
        let kernel = KernelContext::default();
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_cursor_display_up_no_window_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &kernel,
            &executor,
        );
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_display_up_moves_to_prev_line() {
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

        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    fn test_cursor_display_up_at_top() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 0);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_display_up_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3\nline 4");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(3, 0).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));

        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_display_up_count_exceeds_remaining() {
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
        args.set("count", ArgValue::Count(100));

        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 0);
    }

    // =========================================================================
    // Wrapped line movement (within same buffer line)
    // =========================================================================

    #[test]
    fn test_cursor_display_down_within_wrapped_line() {
        // Create a line longer than 80 columns so it wraps
        let long_line: String = "a".repeat(200);
        let kernel = create_test_context();
        let buffer = Buffer::from_string(&long_line);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Move down one display line - should stay on same buffer line
        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Should still be on buffer line 0 but moved to column 80
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 80);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_display_up_within_wrapped_line() {
        // Create a line longer than 80 columns so it wraps
        let long_line: String = "a".repeat(200);
        let kernel = create_test_context();
        let buffer = Buffer::from_string(&long_line);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        // Set cursor to column 90 (display line 1, column 10)
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 90).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Move up one display line - should stay on same buffer line
        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Should still be on buffer line 0 but moved back to column 10
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 10);
    }

    #[test]
    fn test_cursor_display_down_count_exceeds_remaining() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(100));

        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Should clamp to last line
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    fn test_cursor_display_down_through_wrapped_to_next_line() {
        // A long line followed by a short one: gj past the wrapped line
        let long_line = "a".repeat(200); // 3 display lines on 80-col terminal
        let content = format!("{long_line}\nshort");
        let kernel = create_test_context();
        let buffer = Buffer::from_string(&content);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(5)); // More than display lines in first line

        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Should end up on buffer line 1 (the "short" line)
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_display_up_through_wrapped_to_prev_line() {
        // A long line followed by a short one: gk from start of second line
        let long_line = "a".repeat(200); // 3 display lines on 80-col terminal
        let content = format!("{long_line}\nshort");
        let kernel = create_test_context();
        let buffer = Buffer::from_string(&content);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        // Set cursor to start of line 1
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(1, 0).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Move up one display line - should go to last display line of prev buffer line
        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Should be on buffer line 0 (the long line), somewhere in wrapped region
        assert_eq!(window.cursor.line, 0);
        assert!(window.cursor.column >= 160); // In the last display line of the 200-char line
    }

    // =========================================================================
    // Metadata tests
    // =========================================================================

    // =========================================================================
    // Multiple wrapped lines movement tests
    // =========================================================================

    #[test]
    fn test_cursor_display_down_across_multiple_wrapped_lines() {
        // Two long lines (each wraps): moving down should cross both
        let long_line1 = "a".repeat(200); // 3 display lines
        let long_line2 = "b".repeat(200); // 3 display lines
        let content = format!("{long_line1}\n{long_line2}");
        let kernel = create_test_context();
        let buffer = Buffer::from_string(&content);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        // Move down 4 display lines: 3 in first line (remaining) + 1 into second line
        args.set("count", ArgValue::Count(4));

        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Should end up on buffer line 1 (second line)
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_display_up_across_multiple_wrapped_lines() {
        // Two long lines: cursor on second line, move up several display lines
        let long_line1 = "a".repeat(200); // 3 display lines
        let long_line2 = "b".repeat(100); // 2 display lines
        let content = format!("{long_line1}\n{long_line2}");
        let kernel = create_test_context();
        let buffer = Buffer::from_string(&content);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(1, 90).into(); // display line 1 of second buffer line
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        // Move up 3 display lines: 1 up within second line + 2 into first line
        args.set("count", ArgValue::Count(3));

        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Should be on buffer line 0 (first line)
        assert_eq!(window.cursor.line, 0);
    }

    // =========================================================================
    // Display down on single-line buffer
    // =========================================================================

    #[test]
    fn test_cursor_display_down_single_short_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("short");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Only one line, cursor stays at line 0
        assert_eq!(window.cursor.line, 0);
    }

    #[test]
    fn test_cursor_display_up_single_short_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("short");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 0);
    }

    // =========================================================================
    // Column preservation during display line movement
    // =========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_display_down_preserves_display_column() {
        // A 200-char line: cursor at col 10, move down 1 display line
        // should end up at col 90 (display line 1, display col 10 -> buffer col 80+10)
        let long_line = "a".repeat(200);
        let kernel = create_test_context();
        let buffer = Buffer::from_string(&long_line);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 10).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 90);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_display_up_preserves_display_column() {
        // A 200-char line: cursor at col 170, move up 1 display line
        // display line 2, display col 10 -> display line 1, display col 10 -> buffer col 90
        let long_line = "a".repeat(200);
        let kernel = create_test_context();
        let buffer = Buffer::from_string(&long_line);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 170).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 90);
    }

    // =========================================================================
    // No buffer_id returns error
    // =========================================================================

    #[test]
    fn test_cursor_display_down_no_buffer_id() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_cursor_display_down_within_wrapped_line_with_count() {
        // A 250-char line: move down 2 display lines within same buffer line
        let long_line: String = "a".repeat(250);
        let kernel = create_test_context();
        let buffer = Buffer::from_string(&long_line);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));

        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Should stay on buffer line 0, move to display line 2 (col 160)
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 160);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_display_up_within_wrapped_line_with_count() {
        // A 250-char line: cursor at col 200 (display line 2), move up 2 display lines
        let long_line: String = "a".repeat(250);
        let kernel = create_test_context();
        let buffer = Buffer::from_string(&long_line);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 200).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));

        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // At col 200 = display line 2, display col 40
        // Move up 2 -> display line 0, display col 40 -> buffer col 40
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 40);
    }

    #[test]
    fn test_cursor_display_down_crosses_multiple_buffer_lines() {
        // Two short lines then a long line: gj with large count crosses multiple buffer lines
        let content = "short1\nshort2\nshort3\nshort4";
        let kernel = create_test_context();
        let buffer = Buffer::from_string(content);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));

        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Should end up on buffer line 2
        assert_eq!(window.cursor.line, 2);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_display_up_crosses_multiple_buffer_lines() {
        // Four short lines: cursor at line 3, gk with count=2 crosses multiple lines
        let content = "short1\nshort2\nshort3\nshort4";
        let kernel = create_test_context();
        let buffer = Buffer::from_string(content);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(3, 0).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));

        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Should end up on buffer line 1
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_display_down_wraps_into_next_buffer_line() {
        // Line 1: 200 chars (wraps to 3 display lines), Line 2: "short"
        // Cursor at col 160 (display line 2 of line 1), gj should go to line 2
        let long_line = "a".repeat(200);
        let content = format!("{long_line}\nshort");
        let kernel = create_test_context();
        let buffer = Buffer::from_string(&content);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 165).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDisplayDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // At display line 2 (last display line of the 200-char line), gj crosses to line 1
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_display_up_wraps_into_prev_buffer_line() {
        // Line 1: "short", Line 2: 200 chars (wraps to 3 display lines)
        // Cursor at col 5 (display line 0 of line 2), gk should go to line 1
        let long_line = "b".repeat(200);
        let content = format!("short\n{long_line}");
        let kernel = create_test_context();
        let buffer = Buffer::from_string(&content);
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(1, 5).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Should end up on buffer line 0
        assert_eq!(window.cursor.line, 0);
    }

    #[test]
    fn test_cursor_display_up_no_buffer_id() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        let result = CursorDisplayUp.execute(&mut runtime, &args);
        assert!(result.is_error());
    }
}
