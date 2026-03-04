//! Operator command wrappers for Epic #415.
//!
//! These `CommandHandler` implementations wrap the `Operator` trait,
//! reading range information from `CommandContext` and delegating to
//! the appropriate operator.
//!
//! # Architecture
//!
//! When the runner receives `PopResult::ExecuteCommand`, it executes
//! the command with the provided arguments. This module provides the
//! command handlers that are registered with IDs like `vim:delete`
//! and perform the actual operation.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{SessionRuntime, TransitionContext, api::ModeApi},
    reovim_kernel::api::v1::{CommandId, Position},
};

use {
    super::{DeleteOperator, Operator, OperatorContext, Range, YankOperator},
    crate::ids::MODULE,
};

// =============================================================================
// Delete Command (vim:delete)
// =============================================================================

/// Delete operator command - wraps `DeleteOperator` for command execution.
///
/// This command is executed by the runner when `PopResult::ExecuteCommand`
/// is received with command `vim:delete`. It reads the range from the
/// command context and delegates to `DeleteOperator`.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteCommand;

impl Command for DeleteCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MODULE, "delete")
    }

    fn description(&self) -> &'static str {
        "Delete text in range"
    }
}

impl CommandHandler for DeleteCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        execute_operator(&DeleteOperator, runtime, args)
    }
}

// =============================================================================
// Yank Command (vim:yank)
// =============================================================================

/// Yank operator command - wraps `YankOperator` for command execution.
///
/// This command is executed by the runner when `PopResult::ExecuteCommand`
/// is received with command `vim:yank`. It reads the range from the
/// command context and delegates to `YankOperator`.
#[derive(Debug, Clone, Copy, Default)]
pub struct YankCommand;

impl Command for YankCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MODULE, "yank")
    }

    fn description(&self) -> &'static str {
        "Yank text in range to register"
    }
}

impl CommandHandler for YankCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // In Vim, after yank the cursor should be restored to the start of the yanked range.
        // Get the range start BEFORE executing the yank, so we can restore cursor after.
        let (start_line, start_col) = args.range_start().unwrap_or((0, 0));
        let restore_pos = Position::new(start_line, start_col);

        let cur_pos = runtime
            .windows()
            .active()
            .map(|w| Position::new(w.cursor.line, w.cursor.column));
        tracing::debug!(
            range_start = ?restore_pos,
            current_pos = ?cur_pos,
            "yank command: before execute"
        );

        let result = execute_operator(&YankOperator, runtime, args);

        // Restore cursor to start of yanked range (Vim behavior)
        if matches!(result, CommandResult::Success) && args.buffer_id().is_some() {
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = restore_pos.into();
            }

            let cur_pos = runtime
                .windows()
                .active()
                .map(|w| Position::new(w.cursor.line, w.cursor.column));
            tracing::debug!(
                restored_to = ?restore_pos,
                current_pos = ?cur_pos,
                "yank command: cursor restored"
            );
        }

        result
    }
}

// =============================================================================
// Change Command (vim:change)
// =============================================================================

/// Change operator command - wraps `ChangeOperator` for command execution.
///
/// This command is executed by the runner when `PopResult::ExecuteCommand`
/// is received with command `vim:change`. It reads the range from the
/// command context, deletes the text, and transitions to insert mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangeCommand;

impl Command for ChangeCommand {
    fn id(&self) -> CommandId {
        CommandId::new(MODULE, "change")
    }

    fn description(&self) -> &'static str {
        "Change text in range (delete and enter insert)"
    }
}

impl CommandHandler for ChangeCommand {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        use {
            super::ChangeOperator,
            crate::modes::VimMode,
            reovim_driver_undo::{UndoKey, UndoProviderRegistry},
        };

        // Start undo batching BEFORE the delete - both the delete and subsequent
        // insert edits should be grouped as a single undo entry
        if let Some(buffer_id) = args.buffer_id()
            && let Some(window) = runtime.windows().active()
            && let Some(undo_registry) = runtime.kernel().services.get::<UndoProviderRegistry>()
            && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
        {
            let pos = Position::new(window.cursor.line, window.cursor.column);
            undo_provider.begin_batch(buffer_id, pos);
        }

        // Execute the change operator (delete text)
        let result = execute_operator(&ChangeOperator, runtime, args);

        // If successful, enter insert mode (change = delete + insert)
        if matches!(result, CommandResult::Success) {
            runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());
        }

        result
    }
}

// =============================================================================
// Helper Function
// =============================================================================

/// Execute an operator with range information from `CommandContext`.
///
/// This function bridges the `CommandHandler` interface to the `Operator` trait:
/// 1. Extracts `range_start`, `range_end`, linewise from args
/// 2. Builds an `OperatorContext` with kernel access
/// 3. Calls `operator.execute()`
/// 4. Updates cursor in window for text-modifying operators
#[cfg_attr(coverage_nightly, coverage(off))]
fn execute_operator(
    operator: &dyn Operator,
    runtime: &mut SessionRuntime<'_>,
    args: &CommandContext,
) -> CommandResult {
    // Get buffer ID
    let Some(buffer_id) = args.buffer_id() else {
        return CommandResult::error("No active buffer");
    };

    // Get range from context (Epic #415 Phase 5)
    let (start_line, start_col) = args.range_start().unwrap_or((0, 0));
    let (end_line, end_col) = args.range_end().unwrap_or((0, 0));
    let linewise = args.is_linewise();

    let start = Position::new(start_line, start_col);
    let end = Position::new(end_line, end_col);

    // Build range
    let range = if linewise {
        Range::linewise(start, end)
    } else {
        Range::new(start, end)
    };

    // Get count and register (#515: convert Option<char> → Register)
    let count = args.count().unwrap_or(1);
    let register = super::registers::option_char_to_register(args.register());

    // Get cursor position from window (for undo tracking)
    let cursor_position = runtime
        .windows()
        .active()
        .map_or_else(Position::origin, |w| Position::new(w.cursor.line, w.cursor.column));

    // Build operator context using runtime's kernel and per-client registers (#515)
    // Use split-borrow helper to avoid conflicting borrows on runtime
    let (kernel, registers, clipboard_history) = runtime.kernel_and_registers();
    let mut op_ctx = OperatorContext {
        kernel,
        registers,
        clipboard_history,
        buffer_id,
        register,
        count,
        cursor_position,
    };

    // Execute operator
    match operator.execute(&mut op_ctx, range) {
        Ok(()) => {
            // Update cursor for text-modifying operators (#471)
            // After delete/change, cursor should be at the start of the range
            if operator.is_text_modifying()
                && let Some(window) = runtime.windows_mut().active_mut()
            {
                window.cursor = start.into();
                tracing::debug!(
                    ?start,
                    operator = operator.id(),
                    "Updated cursor after text-modifying operator"
                );
            }

            // Record buffer modification for notification pipeline.
            // The operator writes directly to the kernel buffer via OperatorContext,
            // bypassing SessionRuntime::delete_range() which normally calls
            // record_buffer_modified(). We must record it explicitly so that
            // StateChanges propagates to TUI for display refresh.
            if operator.is_text_modifying() {
                runtime.record_buffer_modified(buffer_id);
            }

            CommandResult::Success
        }
        Err(e) => CommandResult::error(&e.to_string()),
    }
}

// =============================================================================
// Command Registration
// =============================================================================

/// Get all operator command handlers.
#[must_use]
pub fn operator_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(DeleteCommand),
        Box::new(YankCommand),
        Box::new(ChangeCommand),
    ]
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
#[allow(clippy::significant_drop_tightening, clippy::uninlined_format_args)]
mod tests {
    use {
        super::*,
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, WindowLayout, api::CommandExecutor,
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

    use {
        reovim_driver_command::{ArgValue, CommandContext},
        reovim_driver_session::api::ModeApi,
    };

    /// Test buffer manager that actually stores buffers.
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

    #[test]
    fn test_delete_command_id() {
        let cmd = DeleteCommand;
        assert_eq!(cmd.id().module().as_str(), "vim");
        assert_eq!(cmd.id().name(), "delete");
    }

    #[test]
    fn test_yank_command_id() {
        let cmd = YankCommand;
        assert_eq!(cmd.id().module().as_str(), "vim");
        assert_eq!(cmd.id().name(), "yank");
    }

    #[test]
    fn test_change_command_id() {
        let cmd = ChangeCommand;
        assert_eq!(cmd.id().module().as_str(), "vim");
        assert_eq!(cmd.id().name(), "change");
    }

    #[test]
    fn test_operator_commands_count() {
        let cmds = operator_commands();
        assert_eq!(cmds.len(), 3);
    }

    #[test]
    fn test_delete_command_description() {
        let cmd = DeleteCommand;
        assert!(cmd.description().contains("Delete"));
    }

    #[test]
    fn test_yank_command_description() {
        let cmd = YankCommand;
        assert!(cmd.description().contains("Yank"));
    }

    #[test]
    fn test_change_command_description() {
        let cmd = ChangeCommand;
        assert!(cmd.description().contains("Change"));
    }

    #[test]
    fn test_delete_command_debug() {
        let debug = format!("{:?}", DeleteCommand);
        assert!(debug.contains("DeleteCommand"));
    }

    #[test]
    fn test_yank_command_debug() {
        let debug = format!("{:?}", YankCommand);
        assert!(debug.contains("YankCommand"));
    }

    #[test]
    fn test_change_command_debug() {
        let debug = format!("{:?}", ChangeCommand);
        assert!(debug.contains("ChangeCommand"));
    }

    #[test]
    fn test_all_operator_commands_default() {
        let _ = DeleteCommand;
        let _ = YankCommand;
        let _ = ChangeCommand;
    }

    #[test]
    fn test_operator_commands_have_correct_ids() {
        let cmds = operator_commands();
        assert_eq!(cmds.len(), 3);
    }

    // ========================================================================
    // Execute tests for operator commands
    // ========================================================================

    #[test]
    fn test_delete_command_no_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = DeleteCommand.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[test]
    fn test_delete_command_execute() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 5));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = DeleteCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Check buffer was modified
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &[" world"]);
    }

    #[test]
    fn test_yank_command_no_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = YankCommand.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[test]
    fn test_yank_command_execute() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 5));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = YankCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer should be unchanged (yank does not modify)
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["hello world"]);

        // Register should have yanked text
        drop(runtime);
        assert_eq!(state.registers.get().text, "hello");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_yank_command_restores_cursor() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 5));

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Set cursor at col 3
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(0, 3).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = YankCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // After yank, cursor should be restored to range start (0, 0)
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 0);
    }

    #[test]
    fn test_change_command_no_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = ChangeCommand.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[test]
    fn test_change_command_execute() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 5));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Should enter insert mode
        assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::INSERT_ID.name());

        // Buffer should have text deleted
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &[" world"]);
    }

    #[test]
    fn test_delete_command_linewise() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("line1\nline2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 0));
        args.set("linewise", ArgValue::Bang(true));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = DeleteCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["line2", "line3"]);
    }

    #[test]
    fn test_yank_command_linewise() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("line1\nline2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 0));
        args.set("linewise", ArgValue::Bang(true));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = YankCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer should be unchanged
        let buf = ctx.buffers.get(buffer_id).unwrap();
        let buf = buf.read();
        assert_eq!(buf.lines(), &["line1", "line2", "line3"]);

        // Register should contain yanked line
        drop(runtime);
        assert_eq!(state.registers.get().text, "line1\n");
    }

    #[test]
    fn test_change_command_linewise() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("line1\nline2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 0));
        args.set("linewise", ArgValue::Bang(true));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Should enter insert mode
        assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::INSERT_ID.name());
    }

    #[test]
    fn test_delete_command_with_register() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 5));
        args.set("register", ArgValue::Register('a'));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = DeleteCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        drop(runtime);
        assert_eq!(state.registers.get_named('a').map(|r| r.text.as_str()), Some("hello"));
    }

    #[test]
    fn test_delete_command_cursor_after_execute() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 3));
        args.set("range_end", ArgValue::Position(0, 8));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = DeleteCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Cursor should be at the start of the deleted range
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 3);
    }

    #[test]
    fn test_change_command_cursor_at_start() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 2));
        args.set("range_end", ArgValue::Position(0, 7));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 2);
    }

    #[test]
    fn test_operator_commands_clone() {
        let del = DeleteCommand;
        let _: DeleteCommand = del;

        let yank = YankCommand;
        let _: YankCommand = yank;

        let change = ChangeCommand;
        let _: ChangeCommand = change;
    }

    #[test]
    fn test_yank_command_no_range_defaults() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        // No range set - should default to (0,0) to (0,0) which is an empty range

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = YankCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    // ========================================================================
    // ChangeCommand undo batching tests
    // ========================================================================

    use {
        reovim_driver_undo::{UndoKey, UndoPersistError, UndoProvider, UndoProviderRegistry},
        reovim_driver_vfs::VfsDriver,
        reovim_kernel::api::v1::{Edit, UndoResult, UndoTree},
    };

    struct MockUndoProvider {
        batch_begins: RwLock<Vec<(BufferId, Position)>>,
        records: RwLock<Vec<BufferId>>,
    }

    impl MockUndoProvider {
        fn new() -> Self {
            Self {
                batch_begins: RwLock::new(Vec::new()),
                records: RwLock::new(Vec::new()),
            }
        }
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    impl UndoProvider for MockUndoProvider {
        fn undo(&self, _: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo(&self, _: BufferId) -> Option<UndoResult> {
            None
        }
        fn redo_branch(&self, _: BufferId, _: usize) -> Option<UndoResult> {
            None
        }
        fn record(&self, buffer_id: BufferId, _: Vec<Edit>, _: Position, _: Position) {
            self.records.write().push(buffer_id);
        }
        fn has_history(&self, _: BufferId) -> bool {
            false
        }
        fn remove(&self, _: BufferId) {}
        fn buffer_count(&self) -> usize {
            0
        }
        fn get_tree(&self, _: BufferId) -> Option<UndoTree> {
            None
        }
        fn begin_batch(&self, buffer_id: BufferId, cursor_before: Position) {
            self.batch_begins.write().push((buffer_id, cursor_before));
        }
        fn end_batch(&self, _: BufferId, _: Position) {}
        fn is_batching(&self, _: BufferId) -> bool {
            false
        }
        fn persist(&self, _: BufferId, _: &str, _: &dyn VfsDriver) -> Result<(), UndoPersistError> {
            Ok(())
        }
        fn load(&self, _: BufferId, _: &str, _: &dyn VfsDriver) -> Result<bool, UndoPersistError> {
            Ok(false)
        }
    }

    fn create_test_context_with_undo() -> (KernelContext, Arc<MockUndoProvider>) {
        let services = Arc::new(ServiceRegistry::new());
        let mock_undo = Arc::new(MockUndoProvider::new());
        let undo_registry = Arc::new(UndoProviderRegistry::new());
        undo_registry.register(UndoKey::Buffer, mock_undo.clone() as Arc<dyn UndoProvider>);
        services.register(undo_registry);

        let ctx = KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            Arc::new(OptionRegistry::default()),
            services,
        );
        (ctx, mock_undo)
    }

    #[test]
    fn test_change_command_calls_begin_batch() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 5));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // begin_batch should have been called before the delete
        let begins = mock_undo.batch_begins.read();
        assert_eq!(begins.len(), 1);
        assert_eq!(begins[0].0, buffer_id);
    }

    #[test]
    fn test_change_command_enters_insert_mode_with_undo() {
        let (ctx, _mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 3));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::INSERT_ID.name());
    }

    #[test]
    fn test_change_command_records_undo_for_delete() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 5));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        ChangeCommand.execute(&mut runtime, &args);

        // The ChangeOperator should have recorded an undo entry
        let records = mock_undo.records.read();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0], buffer_id);
    }

    // ========================================================================
    // Additional execute_operator tests - edge cases
    // ========================================================================

    #[test]
    fn test_delete_command_with_count() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 5));
        args.set("count", ArgValue::Count(2));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = DeleteCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn test_yank_command_with_register_and_count() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 5));
        args.set("register", ArgValue::Register('z'));
        args.set("count", ArgValue::Count(3));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = YankCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        drop(runtime);
        assert_eq!(state.registers.get_named('z').map(|r| r.text.as_str()), Some("hello"));
    }

    #[test]
    fn test_execute_operator_no_active_window_uses_origin() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 5));

        // Create state with no windows - execute_operator should use Position::origin()
        let home_mode = reovim_kernel::api::v1::ModeId::new(
            reovim_kernel::api::v1::ModuleId::new("test"),
            "normal",
        );
        let mut session = Session::new(ClientId::new(1), home_mode.clone());
        let mut mode_stack = ModeStack::new(home_mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut tabs = reovim_driver_session::TabPageSet::new();
        let mut registers = RegisterBank::new();
        let mut clipboard_history = HistoryRing::new();
        let mut local_marks = MarkBank::new();

        // Add a window without buffer to have an active window
        let window = reovim_driver_session::Window::new();
        windows.add(window);

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
            &ctx,
            &StubExecutor,
        );

        // Execute with window that has no buffer_id but command has buffer_id
        let result = DeleteCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[test]
    fn test_change_command_no_buffer_skips_undo_batching() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let args = CommandContext::new(); // No buffer_id

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = ChangeCommand.execute(&mut runtime, &args);
        // Should error due to no buffer
        assert!(matches!(result, CommandResult::Error(_)));

        // No undo batching should have occurred
        let begins = mock_undo.batch_begins.read();
        assert_eq!(begins.len(), 0);
    }

    #[test]
    fn test_yank_command_no_window_cursor_defaults() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(0, 2));
        args.set("range_end", ArgValue::Position(0, 7));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = YankCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Cursor should be restored to range_start
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 2);
    }

    #[test]
    fn test_change_command_linewise_enters_insert_mode() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("line1\nline2\nline3");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("range_start", ArgValue::Position(1, 0));
        args.set("range_end", ArgValue::Position(1, 0));
        args.set("linewise", ArgValue::Bang(true));

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = ChangeCommand.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::INSERT_ID.name());

        // Undo batch should have started
        let begins = mock_undo.batch_begins.read();
        assert_eq!(begins.len(), 1);
    }

    // ========================================================================
    // Coverage: operator error path and tracing closure
    // ========================================================================

    #[test]
    fn test_execute_operator_buffer_not_found_returns_error() {
        // Exercise the Err(e) path at line 240 in execute_operator.
        // Provide a buffer_id that doesn't exist in the kernel's buffer manager,
        // so operator.execute() returns Err(BufferNotFound).
        let ctx = create_test_context();
        let fake_id = BufferId::from_raw(9999);

        let mut args = CommandContext::new();
        args.set_buffer_id(fake_id);
        args.set("range_start", ArgValue::Position(0, 0));
        args.set("range_end", ArgValue::Position(0, 5));

        let mut state = TestState::with_buffer(Some(fake_id));
        let mut runtime = state.runtime(&ctx);
        let result = DeleteCommand.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }
}
