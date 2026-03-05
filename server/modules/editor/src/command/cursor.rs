//! Cursor movement commands.
//!
//! Provides basic cursor movement: up, down, left, right.
//!
//! # Cursor Movement Philosophy
//!
//! Cursor movement commands follow Vim semantics:
//! - j/k (down/up) preserve the "preferred column" - the column the user
//!   intended, even if shorter lines force temporary repositioning
//! - h/l (left/right) clear the preferred column
//! - Movements clamp to valid positions (no-op at boundaries)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{ChangeTracker, SessionRuntime, api::BufferApi},
    reovim_kernel::api::v1::{CommandId, Position},
};

use crate::ids;

/// Move cursor up.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorUp;

impl Command for CursorUp {
    fn id(&self) -> CommandId {
        ids::CURSOR_UP
    }

    fn description(&self) -> &'static str {
        "Move cursor up"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines",
        )]
    }
}

impl CommandHandler for CursorUp {
    #[allow(clippy::cast_possible_truncation)] // Line/column numbers won't exceed u32::MAX
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

        let count = args.count().unwrap_or(1);

        // Calculate new line (saturating sub to handle boundary)
        let new_line = old_pos.line.saturating_sub(count);

        // If already at top, no-op
        if new_line == old_pos.line && old_pos.line == 0 {
            return CommandResult::Success;
        }

        // Get line length for column clamping
        let line_len = runtime.buffer_line_len(buffer_id, new_line).unwrap_or(0);
        let new_col = old_pos.column.min(line_len);
        let new_pos = Position::new(new_line, new_col);

        // In operator-pending mode, return range for the operator
        // j/k motions are linewise
        if args.is_operator_pending() {
            // k moves up, so new_pos.line < old_pos.line
            // TODO(#394): Return operator range via different mechanism (escape hatch until API supports this)
            let _ = (new_pos, old_pos); // Suppress unused warnings
            return CommandResult::Success;
        }

        // Normal mode: move cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = new_pos.into();
        }

        // Record cursor move via ChangeTracker
        // #474: Selection extension is centralized in SessionRuntime::record_cursor_move
        runtime.record_cursor_move(buffer_id);

        CommandResult::Success
    }
}

/// Move cursor down.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorDown;

impl Command for CursorDown {
    fn id(&self) -> CommandId {
        ids::CURSOR_DOWN
    }

    fn description(&self) -> &'static str {
        "Move cursor down"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of lines",
        )]
    }
}

impl CommandHandler for CursorDown {
    #[allow(clippy::cast_possible_truncation)] // Line/column numbers won't exceed u32::MAX
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

        let count = args.count().unwrap_or(1);
        let Some(line_count) = runtime.buffer_line_count(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Calculate new line (clamped to last line)
        let max_line = line_count.saturating_sub(1);
        let new_line = (old_pos.line + count).min(max_line);

        // If already at bottom, no-op
        if new_line == old_pos.line && old_pos.line == max_line {
            return CommandResult::Success;
        }

        // Get line length for column clamping
        let line_len = runtime.buffer_line_len(buffer_id, new_line).unwrap_or(0);
        let new_col = old_pos.column.min(line_len);
        let new_pos = Position::new(new_line, new_col);

        // In operator-pending mode, return range for the operator
        // j/k motions are linewise
        if args.is_operator_pending() {
            // j moves down, so old_pos.line < new_pos.line
            // TODO(#394): Return operator range via different mechanism (escape hatch until API supports this)
            let _ = (old_pos, new_pos); // Suppress unused warnings
            return CommandResult::Success;
        }

        // Normal mode: move cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = new_pos.into();
        }

        // Record cursor move via ChangeTracker
        // #474: Selection extension is centralized in SessionRuntime::record_cursor_move
        runtime.record_cursor_move(buffer_id);

        CommandResult::Success
    }
}

/// Move cursor left.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorLeft;

impl Command for CursorLeft {
    fn id(&self) -> CommandId {
        ids::CURSOR_LEFT
    }

    fn description(&self) -> &'static str {
        "Move cursor left"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of columns",
        )]
    }
}

impl CommandHandler for CursorLeft {
    #[allow(clippy::cast_possible_truncation)] // Line/column numbers won't exceed u32::MAX
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

        let count = args.count().unwrap_or(1);

        // Calculate new column (saturating sub to handle boundary)
        let new_col = old_pos.column.saturating_sub(count);

        // If already at left edge, no-op
        if new_col == old_pos.column && old_pos.column == 0 {
            return CommandResult::Success;
        }

        let new_pos = Position::new(old_pos.line, new_col);

        // In operator-pending mode, return range for the operator
        // h/l motions are characterwise
        if args.is_operator_pending() {
            // h moves left, so new_pos.column < old_pos.column
            // TODO(#394): Return operator range via different mechanism (escape hatch until API supports this)
            let _ = (new_pos, old_pos); // Suppress unused warnings
            return CommandResult::Success;
        }

        // Normal mode: move cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = new_pos.into();
        }

        // Record cursor move via ChangeTracker
        // #474: Selection extension is centralized in SessionRuntime::record_cursor_move
        runtime.record_cursor_move(buffer_id);

        CommandResult::Success
    }
}

/// Move cursor right.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorRight;

impl Command for CursorRight {
    fn id(&self) -> CommandId {
        ids::CURSOR_RIGHT
    }

    fn description(&self) -> &'static str {
        "Move cursor right"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "count",
            ArgKind::Count,
            "Number of columns",
        )]
    }
}

impl CommandHandler for CursorRight {
    #[allow(clippy::cast_possible_truncation)] // Line/column numbers won't exceed u32::MAX
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

        let count = args.count().unwrap_or(1);

        // Get current line length for boundary check
        let line_len = runtime
            .buffer_line_len(buffer_id, old_pos.line)
            .unwrap_or(0);

        // In normal mode, cursor can't go past the last character.
        // For a line of length N, valid columns are 0..N-1.
        // An empty line has max_col 0, but we can still be at col 0.
        let max_col = line_len.saturating_sub(1);

        // Calculate new column (clamped to max valid position)
        let new_col = (old_pos.column + count).min(max_col);

        // If already at right edge, no-op
        if new_col == old_pos.column && old_pos.column == max_col {
            return CommandResult::Success;
        }

        let new_pos = Position::new(old_pos.line, new_col);

        // In operator-pending mode, return range for the operator
        // h/l motions are characterwise
        if args.is_operator_pending() {
            // l moves right, so old_pos.column < new_pos.column
            // TODO(#394): Return operator range via different mechanism (escape hatch until API supports this)
            let _ = (old_pos, new_pos); // Suppress unused warnings
            return CommandResult::Success;
        }

        // Normal mode: move cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = new_pos.into();
        }

        // Record cursor move via ChangeTracker
        // #474: Selection extension is centralized in SessionRuntime::record_cursor_move
        runtime.record_cursor_move(buffer_id);

        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::{ArgKind, ArgValue, Command, CommandContext},
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
            api::{CommandExecutor, Selection, SelectionMode},
        },
        reovim_kernel::api::{
            ServiceRegistry,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, CommandId as KernelCommandId,
                EventBus, HistoryRing, KernelContext, MarkBank, ModeId, ModeStack, ModuleId,
                MotionEngine, OptionRegistry, Position, RegisterBank, RwLock, TextObjectEngine,
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
            executor: &'a dyn CommandExecutor,
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
    // No-window error tests
    // =========================================================================

    #[test]
    fn test_cursor_up_no_window_returns_error() {
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
        args.set_buffer_id(buffer_id);
        assert!(CursorUp.execute(&mut runtime, &args).is_error());
    }

    #[test]
    fn test_cursor_down_no_window_returns_error() {
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
        args.set_buffer_id(buffer_id);
        assert!(CursorDown.execute(&mut runtime, &args).is_error());
    }

    #[test]
    fn test_cursor_left_no_window_returns_error() {
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
        args.set_buffer_id(buffer_id);
        assert!(CursorLeft.execute(&mut runtime, &args).is_error());
    }

    #[test]
    fn test_cursor_right_no_window_returns_error() {
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
        args.set_buffer_id(buffer_id);
        assert!(CursorRight.execute(&mut runtime, &args).is_error());
    }

    // =========================================================================
    // Buffer not found (CursorDown)
    // =========================================================================

    #[test]
    fn test_cursor_down_buffer_not_found() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set("buffer_id", ArgValue::BufferId(99999));
        assert!(CursorDown.execute(&mut runtime, &args).is_error());
    }

    // =========================================================================
    // Operator-pending mode tests
    // =========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_up_operator_pending() {
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
        args.set_mode_name("operator-pending");
        assert!(CursorUp.execute(&mut runtime, &args).is_success());
    }

    #[test]
    fn test_cursor_down_operator_pending() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set_mode_name("operator-pending");
        assert!(CursorDown.execute(&mut runtime, &args).is_success());
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_left_operator_pending() {
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
        args.set_mode_name("operator-pending");
        assert!(CursorLeft.execute(&mut runtime, &args).is_success());
    }

    #[test]
    fn test_cursor_right_operator_pending() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set_mode_name("operator-pending");
        assert!(CursorRight.execute(&mut runtime, &args).is_success());
    }

    // =========================================================================
    // Visual mode selection extension tests
    // =========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_up_visual_mode_extends_selection() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(1, 0).into();
            window.selection = Some(Selection::new(
                Position::new(0, 0),
                Position::new(1, 1),
                SelectionMode::Character,
            ));
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        assert!(CursorUp.execute(&mut runtime, &args).is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 0);
        let sel = window.selection.as_ref().unwrap();
        // sel.end = Position::new(new_pos.line=0, new_pos.column=0 + 1=1)
        assert_eq!(sel.end, Position::new(0, 1));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_down_visual_mode_extends_selection() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.selection = Some(Selection::new(
                Position::new(0, 0),
                Position::new(0, 1),
                SelectionMode::Character,
            ));
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        assert!(CursorDown.execute(&mut runtime, &args).is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
        let sel = window.selection.as_ref().unwrap();
        // sel.end = Position::new(new_pos.line=1, new_pos.column=0 + 1=1)
        assert_eq!(sel.end, Position::new(1, 1));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_left_visual_mode_extends_selection() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 5).into();
            window.selection = Some(Selection::new(
                Position::new(0, 0),
                Position::new(0, 6),
                SelectionMode::Character,
            ));
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        assert!(CursorLeft.execute(&mut runtime, &args).is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 4);
        let sel = window.selection.as_ref().unwrap();
        // sel.end = Position::new(0, 4 + 1 = 5)
        assert_eq!(sel.end, Position::new(0, 5));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_right_visual_mode_extends_selection() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.selection = Some(Selection::new(
                Position::new(0, 0),
                Position::new(0, 1),
                SelectionMode::Character,
            ));
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        assert!(CursorRight.execute(&mut runtime, &args).is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 1);
        let sel = window.selection.as_ref().unwrap();
        // sel.end = Position::new(0, 1 + 1 = 2)
        assert_eq!(sel.end, Position::new(0, 2));
    }

    // =========================================================================
    // Description and args tests
    // =========================================================================

    #[test]
    fn test_cursor_descriptions() {
        assert_eq!(CursorUp.description(), "Move cursor up");
        assert_eq!(CursorDown.description(), "Move cursor down");
        assert_eq!(CursorLeft.description(), "Move cursor left");
        assert_eq!(CursorRight.description(), "Move cursor right");
    }

    #[test]
    fn test_cursor_args_all_have_count() {
        let commands: Vec<Box<dyn CommandHandler>> = vec![
            Box::new(CursorUp),
            Box::new(CursorDown),
            Box::new(CursorLeft),
            Box::new(CursorRight),
        ];
        for cmd in &commands {
            let args = cmd.args();
            assert_eq!(args.len(), 1);
            assert_eq!(args[0].name, "count");
            assert_eq!(args[0].kind, ArgKind::Count);
        }
    }

    // =========================================================================
    // Cursor movement with count
    // =========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_up_with_count() {
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

        let result = CursorUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_up_count_exceeds_lines_clamps_to_zero() {
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

        let result = CursorUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 0);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_left_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 8).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(3));

        let result = CursorLeft.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 5);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_left_count_exceeds_column_clamps_to_zero() {
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

        let result = CursorLeft.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    // =========================================================================
    // Cursor up/down column clamping to shorter lines
    // =========================================================================

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_down_clamps_column_to_shorter_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("long line here\nhi");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 10).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
        // "hi" has len 2, so column clamped to min(10, 2) = 2
        assert_eq!(window.cursor.column, 2);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_cursor_up_clamps_column_to_shorter_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hi\nlong line here");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(1, 10).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorUp.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 0);
        // "hi" has len 2, so column clamped to min(10, 2) = 2
        assert_eq!(window.cursor.column, 2);
    }

    // =========================================================================
    // Cursor down with count
    // =========================================================================

    #[test]
    fn test_cursor_down_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3\nline 4");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(3));

        let result = CursorDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 3);
    }

    #[test]
    fn test_cursor_down_count_exceeds_lines_clamps_to_last() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(100));

        let result = CursorDown.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
    }

    // =========================================================================
    // Cursor right with count
    // =========================================================================

    #[test]
    fn test_cursor_right_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(5));

        let result = CursorRight.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 5);
    }

    #[test]
    fn test_cursor_right_count_exceeds_line_clamps_to_last_char() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hi");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(100));

        let result = CursorRight.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // "hi" has len 2, max_col = 1 (last char index)
        assert_eq!(window.cursor.column, 1);
    }

    // =========================================================================
    // No buffer_id tests
    // =========================================================================

    #[test]
    fn test_cursor_up_no_buffer_id_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        assert!(CursorUp.execute(&mut runtime, &args).is_error());
    }

    #[test]
    fn test_cursor_down_no_buffer_id_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        assert!(CursorDown.execute(&mut runtime, &args).is_error());
    }

    #[test]
    fn test_cursor_left_no_buffer_id_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        assert!(CursorLeft.execute(&mut runtime, &args).is_error());
    }

    #[test]
    fn test_cursor_right_no_buffer_id_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let args = CommandContext::new();
        assert!(CursorRight.execute(&mut runtime, &args).is_error());
    }

    // =========================================================================
    // Cursor IDs
    // =========================================================================

    #[test]
    fn test_cursor_ids() {
        assert_eq!(CursorUp.id().name(), "cursor-up");
        assert_eq!(CursorDown.id().name(), "cursor-down");
        assert_eq!(CursorLeft.id().name(), "cursor-left");
        assert_eq!(CursorRight.id().name(), "cursor-right");
    }

    // =========================================================================
    // Empty line handling for cursor right
    // =========================================================================

    #[test]
    fn test_cursor_right_on_empty_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorRight.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    // =========================================================================
    // Cursor down on empty buffer
    // =========================================================================

    #[test]
    fn test_cursor_down_empty_buffer() {
        let kernel = create_test_context();
        let buffer = Buffer::new();
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = CursorDown.execute(&mut runtime, &args);
        assert!(result.is_success());
    }
}
