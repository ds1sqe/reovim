//! Visual mode exit commands.
//!
//! Provides commands to exit visual mode and return to normal mode.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        BufferApi, SessionRuntime, TransitionContext,
        api::{ChangeTracker, ModeApi},
    },
    reovim_kernel::api::v1::CommandId,
};

use crate::{ids, modes::VimMode};

/// Exit visual mode and return to normal mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExitVisualMode;

impl Command for ExitVisualMode {
    fn id(&self) -> CommandId {
        ids::EXIT_VISUAL
    }

    fn description(&self) -> &'static str {
        "Exit visual mode and return to normal mode"
    }
}

impl CommandHandler for ExitVisualMode {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Clear selection on the active window
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = None;
        }

        // #474: Notify other clients that selection was cleared
        if let Some(buffer_id) = runtime.active_buffer() {
            runtime.record_selection_change(buffer_id);
        }

        // Change to normal mode
        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}

#[cfg(test)]
#[allow(clippy::significant_drop_tightening, clippy::uninlined_format_args)]
mod tests {
    use {super::*, reovim_driver_command::Command};

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_visual_mode_id() {
        let cmd = ExitVisualMode;
        assert_eq!(cmd.id(), ids::EXIT_VISUAL);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_visual_mode_description() {
        let cmd = ExitVisualMode;
        assert!(cmd.description().contains("visual"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_visual_mode_debug() {
        let debug = format!("{:?}", ExitVisualMode);
        assert!(debug.contains("ExitVisualMode"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_visual_mode_default() {
        let _ = ExitVisualMode;
    }

    // ========================================================================
    // Execute tests
    // ========================================================================

    use {
        reovim_driver_command::CommandHandler,
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, SessionRuntime, WindowLayout,
            api::{CommandExecutor, Selection},
        },
        reovim_kernel::api::{
            ModeStack,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, EventBus, HistoryRing, KernelContext,
                MarkBank, ModeId, ModuleId, MotionEngine, OptionRegistry, Position, RegisterBank,
                RwLock, ServiceRegistry, TextObjectEngine,
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
        fn execute(
            &self,
            _: &CommandId,
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
        registers: RegisterBank,
        clipboard_history: HistoryRing,
        local_marks: MarkBank,
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

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_visual_mode_execute_clears_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));

        // Set up a selection first
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
        }

        let mut runtime = state.runtime(&ctx);
        let result = ExitVisualMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Selection should be cleared
        let window = runtime.windows().active().unwrap();
        assert!(window.selection.is_none());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_visual_mode_execute_no_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        // Even without selection, exit should succeed
        let result = ExitVisualMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert!(window.selection.is_none());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_visual_mode_execute_clears_line_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = ExitVisualMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert!(window.selection.is_none());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_visual_mode_execute_clears_block_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::block(Position::new(0, 1), Position::new(1, 3)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = ExitVisualMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert!(window.selection.is_none());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_visual_mode_sets_normal_mode() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 3)));
        }
        let mut runtime = state.runtime(&ctx);

        ExitVisualMode.execute(&mut runtime, &args);

        assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::NORMAL_ID.name());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_visual_mode_clone_copy() {
        let cmd = ExitVisualMode;
        let _: ExitVisualMode = cmd;
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_visual_mode_description_contains_normal() {
        let cmd = ExitVisualMode;
        assert!(cmd.description().contains("normal"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_visual_mode_preserves_cursor() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let args = CommandContext::new();
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 5).into();
            w.selection = Some(Selection::character(Position::new(0, 2), Position::new(0, 8)));
        }
        let mut runtime = state.runtime(&ctx);

        ExitVisualMode.execute(&mut runtime, &args);

        // Cursor should be preserved at its position
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 5);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_exit_visual_mode_no_window() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let home_mode = ModeId::new(ModuleId::new("test"), "normal");
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
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
                registers: &mut registers,
                clipboard_history: &mut clipboard_history,
                local_marks: &mut local_marks,
            },
            &ctx,
            &StubExecutor,
        );

        // Even without a window, exit should succeed
        let result = ExitVisualMode.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }
}
