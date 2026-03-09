//! Visual mode entry commands.
//!
//! Provides commands to enter the various visual selection modes:
//! - `v` - Character-wise selection
//! - `V` - Line-wise selection
//! - `Ctrl-V` - Block (rectangular) selection

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        BufferApi, SessionRuntime, TransitionContext,
        api::{ChangeTracker, ModeApi, Selection},
    },
    reovim_kernel::api::v1::{CommandId, Position},
};

use crate::{ids, modes::VimMode};

/// Enter visual mode (character-wise selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualMode;

impl Command for EnterVisualMode {
    fn id(&self) -> CommandId {
        ids::ENTER_VISUAL
    }

    fn description(&self) -> &'static str {
        "Enter visual mode (character-wise)"
    }
}

impl CommandHandler for EnterVisualMode {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Get current cursor position from per-client window
        let Some(window) = runtime.windows().active() else {
            tracing::warn!("EnterVisualMode: No active window");
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Start character-wise selection at current cursor position.
        // Phase 8 (#465): Use exclusive end semantics - to include the character
        // at the cursor, end must be cursor + 1.
        let selection = Selection::character(pos, Position::new(pos.line, pos.column + 1));
        tracing::debug!(?pos, ?selection, "EnterVisualMode: Setting selection");

        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = Some(selection);
        }

        // #474: Notify other clients about new selection
        if let Some(buffer_id) = runtime.active_buffer() {
            runtime.record_selection_change(buffer_id);
        }

        // Verify selection was set
        let sel_check = runtime.windows().active().and_then(|w| w.selection.clone());
        tracing::debug!(?sel_check, "EnterVisualMode: Selection after set");

        // Change to visual mode
        runtime.set_mode(VimMode::VISUAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Enter visual line mode (line-wise selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualLineMode;

impl Command for EnterVisualLineMode {
    fn id(&self) -> CommandId {
        ids::ENTER_VISUAL_LINE
    }

    fn description(&self) -> &'static str {
        "Enter visual line mode"
    }
}

impl CommandHandler for EnterVisualLineMode {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Get current cursor position from per-client window
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Start line-wise selection at current cursor position.
        // Phase 8 (#465): Use exclusive end semantics - to select the current line,
        // end.line must be pos.line + 1.
        let selection = Selection::line(Position::new(pos.line, 0), Position::new(pos.line + 1, 0));

        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = Some(selection);
        }

        // #474: Notify other clients about new selection
        if let Some(buffer_id) = runtime.active_buffer() {
            runtime.record_selection_change(buffer_id);
        }

        // Change to visual line mode
        runtime.set_mode(VimMode::VISUAL_LINE_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Enter visual block mode (rectangular selection).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterVisualBlockMode;

impl Command for EnterVisualBlockMode {
    fn id(&self) -> CommandId {
        ids::ENTER_VISUAL_BLOCK
    }

    fn description(&self) -> &'static str {
        "Enter visual block mode"
    }
}

impl CommandHandler for EnterVisualBlockMode {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Get current cursor position from per-client window
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Start block selection at current cursor position.
        // Phase 8 (#465): Use exclusive end semantics - to include the character
        // at the cursor, end must be cursor + 1.
        let selection = Selection::block(pos, Position::new(pos.line, pos.column + 1));

        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = Some(selection);
        }

        // #474: Notify other clients about new selection
        if let Some(buffer_id) = runtime.active_buffer() {
            runtime.record_selection_change(buffer_id);
        }

        // Change to visual block mode
        runtime.set_mode(VimMode::VISUAL_BLOCK_ID, TransitionContext::new());

        CommandResult::Success
    }
}

#[cfg(test)]
#[allow(clippy::significant_drop_tightening, clippy::uninlined_format_args)]
mod tests {
    use {super::*, reovim_driver_command::Command};

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_mode_id() {
        let cmd = EnterVisualMode;
        assert_eq!(cmd.id(), ids::ENTER_VISUAL);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_mode_description() {
        let cmd = EnterVisualMode;
        assert!(cmd.description().contains("visual"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_line_mode_id() {
        let cmd = EnterVisualLineMode;
        assert_eq!(cmd.id(), ids::ENTER_VISUAL_LINE);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_line_mode_description() {
        let cmd = EnterVisualLineMode;
        assert!(cmd.description().contains("visual line"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_block_mode_id() {
        let cmd = EnterVisualBlockMode;
        assert_eq!(cmd.id(), ids::ENTER_VISUAL_BLOCK);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_block_mode_description() {
        let cmd = EnterVisualBlockMode;
        assert!(cmd.description().contains("visual block"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_all_visual_entry_debug() {
        assert!(format!("{:?}", EnterVisualMode).contains("EnterVisualMode"));
        assert!(format!("{:?}", EnterVisualLineMode).contains("EnterVisualLineMode"));
        assert!(format!("{:?}", EnterVisualBlockMode).contains("EnterVisualBlockMode"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_all_visual_entry_default() {
        let _ = EnterVisualMode;
        let _ = EnterVisualLineMode;
        let _ = EnterVisualBlockMode;
    }

    // ========================================================================
    // Execute tests
    // ========================================================================

    use {
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
        #[cfg_attr(coverage_nightly, coverage(off))]
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

    // --- EnterVisualMode execute ---

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_mode_execute_sets_character_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        let result = EnterVisualMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Selection should be set to character mode at cursor position (0,0) to (0,1)
        let window = runtime.windows().active().unwrap();
        let sel = window.selection.as_ref().unwrap();
        assert_eq!(sel.start, Position::new(0, 0));
        assert_eq!(sel.end, Position::new(0, 1));
        assert_eq!(sel.mode, reovim_driver_session::api::SelectionMode::Character);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_mode_execute_at_nonzero_cursor() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        // Move cursor to column 5
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 5).into();
        }
        let mut runtime = state.runtime(&ctx);

        let result = EnterVisualMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        let sel = window.selection.as_ref().unwrap();
        assert_eq!(sel.start, Position::new(0, 5));
        assert_eq!(sel.end, Position::new(0, 6));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_mode_execute_no_active_window() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty(); // No windows at all
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

        let result = EnterVisualMode.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    // --- EnterVisualLineMode execute ---

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_line_mode_execute_sets_line_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld\nfoo");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        let result = EnterVisualLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        let sel = window.selection.as_ref().unwrap();
        assert_eq!(sel.start, Position::new(0, 0));
        assert_eq!(sel.end, Position::new(1, 0));
        assert_eq!(sel.mode, reovim_driver_session::api::SelectionMode::Line);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_line_mode_execute_at_line_2() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld\nfoo");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(2, 1).into();
        }
        let mut runtime = state.runtime(&ctx);

        let result = EnterVisualLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        let sel = window.selection.as_ref().unwrap();
        assert_eq!(sel.start, Position::new(2, 0));
        assert_eq!(sel.end, Position::new(3, 0));
        assert_eq!(sel.mode, reovim_driver_session::api::SelectionMode::Line);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_line_mode_execute_no_active_window() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
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
            &ctx,
            &StubExecutor,
        );

        let result = EnterVisualLineMode.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    // --- EnterVisualBlockMode execute ---

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_block_mode_execute_sets_block_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        let result = EnterVisualBlockMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        let sel = window.selection.as_ref().unwrap();
        assert_eq!(sel.start, Position::new(0, 0));
        assert_eq!(sel.end, Position::new(0, 1));
        assert_eq!(sel.mode, reovim_driver_session::api::SelectionMode::Block);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_block_mode_execute_at_nonzero_cursor() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(1, 3).into();
        }
        let mut runtime = state.runtime(&ctx);

        let result = EnterVisualBlockMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        let sel = window.selection.as_ref().unwrap();
        assert_eq!(sel.start, Position::new(1, 3));
        assert_eq!(sel.end, Position::new(1, 4));
        assert_eq!(sel.mode, reovim_driver_session::api::SelectionMode::Block);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_block_mode_execute_no_active_window() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
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
            &ctx,
            &StubExecutor,
        );

        let result = EnterVisualBlockMode.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_mode_sets_mode_to_visual() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        EnterVisualMode.execute(&mut runtime, &args);

        assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::VISUAL_ID.name());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_line_mode_sets_mode() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        EnterVisualLineMode.execute(&mut runtime, &args);

        assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::VISUAL_LINE_ID.name());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_block_mode_sets_mode() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        EnterVisualBlockMode.execute(&mut runtime, &args);

        assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::VISUAL_BLOCK_ID.name());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_mode_at_line_2_col_3() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld\nfoo");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(2, 1).into();
        }
        let mut runtime = state.runtime(&ctx);

        let result = EnterVisualMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        let sel = window.selection.as_ref().unwrap();
        assert_eq!(sel.start, Position::new(2, 1));
        assert_eq!(sel.end, Position::new(2, 2));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_block_mode_at_line_1() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(1, 0).into();
        }
        let mut runtime = state.runtime(&ctx);

        let result = EnterVisualBlockMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        let sel = window.selection.as_ref().unwrap();
        assert_eq!(sel.start, Position::new(1, 0));
        assert_eq!(sel.end, Position::new(1, 1));
        assert_eq!(sel.mode, reovim_driver_session::api::SelectionMode::Block);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_visual_line_mode_selection_starts_at_col_0() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("  hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        // Cursor at column 5 but line selection always starts at col 0
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 5).into();
        }
        let mut runtime = state.runtime(&ctx);

        let result = EnterVisualLineMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        let sel = window.selection.as_ref().unwrap();
        assert_eq!(sel.start, Position::new(0, 0));
        assert_eq!(sel.end, Position::new(1, 0));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_all_entry_commands_clone_copy() {
        let _: EnterVisualMode = EnterVisualMode;
        let _: EnterVisualLineMode = EnterVisualLineMode;
        let _: EnterVisualBlockMode = EnterVisualBlockMode;
    }
}
