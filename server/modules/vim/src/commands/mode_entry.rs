//! Mode entry commands.
//!
//! Provides specialized commands for entering insert mode:
//! - `EnterInsertFirstNonBlank` (I)
//! - `EnterInsertEndOfLine` (A)
//! - `OpenLineBelow` (o)
//! - `OpenLineAbove` (O)
//!
//! # Epic #372 - Mode Ownership
//!
//! These commands use `VimMode::INSERT_ID` to transition to insert mode,
//! which is why they belong in the vim module.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{BufferApi, SessionRuntime, TransitionContext, api::ModeApi},
    reovim_driver_undo::{UndoKey, UndoProviderRegistry},
    reovim_kernel::api::v1::{BufferId, CommandId, OptionScopeId, Position},
};

use {
    crate::{ids, modes::VimMode},
    reovim_module_editor::command::get_line_indent,
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

/// Start undo batching for insert mode.
#[cfg_attr(coverage_nightly, coverage(off))]
fn begin_insert_batch(runtime: &SessionRuntime<'_>, buffer_id: BufferId) {
    if let Some(pos) = get_cursor_position(runtime)
        && let Some(undo_registry) = runtime.kernel().services.get::<UndoProviderRegistry>()
        && let Some(undo_provider) = undo_registry.get(&UndoKey::Buffer)
    {
        undo_provider.begin_batch(buffer_id, pos);
    }
}

/// Enter insert mode at first non-blank character (I).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertFirstNonBlank;

impl Command for EnterInsertFirstNonBlank {
    fn id(&self) -> CommandId {
        ids::ENTER_INSERT_BOL
    }

    fn description(&self) -> &'static str {
        "Enter insert mode at first non-blank character"
    }
}

impl CommandHandler for EnterInsertFirstNonBlank {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        if let Some(buffer_id) = args.buffer_id() {
            if let Some(pos) = get_cursor_position(runtime) {
                // Find first non-blank character on current line
                let first_non_blank = runtime
                    .buffer_line(buffer_id, pos.line)
                    .map_or(0, |line| line.chars().position(|c| !c.is_whitespace()).unwrap_or(0));

                set_cursor_position(runtime, Position::new(pos.line, first_non_blank));
            }
            // Start undo batching for insert mode
            begin_insert_batch(runtime, buffer_id);
        }

        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Enter insert mode at end of line (A).
#[derive(Debug, Clone, Copy, Default)]
pub struct EnterInsertEndOfLine;

impl Command for EnterInsertEndOfLine {
    fn id(&self) -> CommandId {
        ids::ENTER_INSERT_EOL
    }

    fn description(&self) -> &'static str {
        "Enter insert mode at end of line"
    }
}

impl CommandHandler for EnterInsertEndOfLine {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        if let Some(buffer_id) = args.buffer_id() {
            if let Some(pos) = get_cursor_position(runtime) {
                // Move cursor to end of current line
                let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);
                set_cursor_position(runtime, Position::new(pos.line, line_len));
            }
            // Start undo batching for insert mode
            begin_insert_batch(runtime, buffer_id);
        }

        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Open line below and enter insert mode (o).
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenLineBelow;

impl Command for OpenLineBelow {
    fn id(&self) -> CommandId {
        ids::OPEN_LINE_BELOW
    }

    fn description(&self) -> &'static str {
        "Open line below and enter insert mode"
    }
}

impl CommandHandler for OpenLineBelow {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Check autoindent option (escape hatch - OptionsApi not yet available)
        let autoindent = runtime
            .kernel()
            .options
            .get("autoindent", OptionScopeId::Buffer(buffer_id))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let Some(pos) = get_cursor_position(runtime) else {
            return CommandResult::error("Failed to get buffer position");
        };

        // Get indent from current line if autoindent is enabled
        let indent = if autoindent {
            runtime
                .buffer_line(buffer_id, pos.line)
                .map(|line| get_line_indent(&line).to_owned())
                .unwrap_or_default()
        } else {
            String::new()
        };

        // Get end of current line position
        let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);
        let insert_pos = Position::new(pos.line, line_len);

        // Insert newline + indent at end of current line
        let insert_text = format!("\n{indent}");
        runtime.insert_text(buffer_id, insert_pos, &insert_text);

        // Position cursor at end of indent on new line
        let indent_len = indent.chars().count();
        set_cursor_position(runtime, Position::new(pos.line + 1, indent_len));

        // Start undo batching for insert mode
        begin_insert_batch(runtime, buffer_id);

        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Open line above and enter insert mode (O).
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenLineAbove;

impl Command for OpenLineAbove {
    fn id(&self) -> CommandId {
        ids::OPEN_LINE_ABOVE
    }

    fn description(&self) -> &'static str {
        "Open line above and enter insert mode"
    }
}

impl CommandHandler for OpenLineAbove {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Check autoindent option (escape hatch - OptionsApi not yet available)
        let autoindent = runtime
            .kernel()
            .options
            .get("autoindent", OptionScopeId::Buffer(buffer_id))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let Some(pos) = get_cursor_position(runtime) else {
            return CommandResult::error("Failed to get buffer position");
        };

        // Get indent from current line if autoindent is enabled
        let indent = if autoindent {
            runtime
                .buffer_line(buffer_id, pos.line)
                .map(|line| get_line_indent(&line).to_owned())
                .unwrap_or_default()
        } else {
            String::new()
        };

        // Insert indent + newline at start of current line
        let insert_pos = Position::new(pos.line, 0);
        let insert_text = format!("{indent}\n");
        runtime.insert_text(buffer_id, insert_pos, &insert_text);

        // Position cursor at end of indent on the new line (which is now at pos.line)
        let indent_len = indent.chars().count();
        set_cursor_position(runtime, Position::new(pos.line, indent_len));

        // Start undo batching for insert mode
        begin_insert_batch(runtime, buffer_id);

        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}

#[cfg(test)]
#[allow(clippy::uninlined_format_args, clippy::significant_drop_tightening)]
mod tests {
    use super::*;

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_get_line_indent_spaces() {
        assert_eq!(get_line_indent("    hello"), "    ");
        assert_eq!(get_line_indent("  world"), "  ");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_get_line_indent_tabs() {
        assert_eq!(get_line_indent("\t\thello"), "\t\t");
        assert_eq!(get_line_indent("\tworld"), "\t");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_get_line_indent_mixed() {
        assert_eq!(get_line_indent("\t  hello"), "\t  ");
        assert_eq!(get_line_indent("  \thello"), "  \t");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_get_line_indent_no_indent() {
        assert_eq!(get_line_indent("hello"), "");
        assert_eq!(get_line_indent("world"), "");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_get_line_indent_empty() {
        assert_eq!(get_line_indent(""), "");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_get_line_indent_whitespace_only() {
        assert_eq!(get_line_indent("    "), "    ");
        assert_eq!(get_line_indent("\t\t"), "\t\t");
    }

    // ========================================================================
    // Command metadata tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_first_non_blank_id() {
        use reovim_driver_command::Command;
        let cmd = EnterInsertFirstNonBlank;
        assert_eq!(cmd.id(), ids::ENTER_INSERT_BOL);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_first_non_blank_description() {
        use reovim_driver_command::Command;
        let cmd = EnterInsertFirstNonBlank;
        assert!(cmd.description().contains("first non-blank"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_end_of_line_id() {
        use reovim_driver_command::Command;
        let cmd = EnterInsertEndOfLine;
        assert_eq!(cmd.id(), ids::ENTER_INSERT_EOL);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_end_of_line_description() {
        use reovim_driver_command::Command;
        let cmd = EnterInsertEndOfLine;
        assert!(cmd.description().contains("end of line"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_below_id() {
        use reovim_driver_command::Command;
        let cmd = OpenLineBelow;
        assert_eq!(cmd.id(), ids::OPEN_LINE_BELOW);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_below_description() {
        use reovim_driver_command::Command;
        let cmd = OpenLineBelow;
        assert!(cmd.description().contains("below"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_above_id() {
        use reovim_driver_command::Command;
        let cmd = OpenLineAbove;
        assert_eq!(cmd.id(), ids::OPEN_LINE_ABOVE);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_above_description() {
        use reovim_driver_command::Command;
        let cmd = OpenLineAbove;
        assert!(cmd.description().contains("above"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_all_mode_entry_commands_debug() {
        let debug = format!("{:?}", EnterInsertFirstNonBlank);
        assert!(debug.contains("EnterInsertFirstNonBlank"));

        let debug = format!("{:?}", EnterInsertEndOfLine);
        assert!(debug.contains("EnterInsertEndOfLine"));

        let debug = format!("{:?}", OpenLineBelow);
        assert!(debug.contains("OpenLineBelow"));

        let debug = format!("{:?}", OpenLineAbove);
        assert!(debug.contains("OpenLineAbove"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_all_mode_entry_commands_default() {
        let _ = EnterInsertFirstNonBlank;
        let _ = EnterInsertEndOfLine;
        let _ = OpenLineBelow;
        let _ = OpenLineAbove;
    }

    // ========================================================================
    // Additional get_line_indent tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_get_line_indent_unicode() {
        // Unicode content but ascii indent
        assert_eq!(get_line_indent("  \u{1f600}hello"), "  ");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_get_line_indent_single_space() {
        assert_eq!(get_line_indent(" x"), " ");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_get_line_indent_single_tab() {
        assert_eq!(get_line_indent("\tx"), "\t");
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
                Buffer, BufferError, BufferId, BufferManager, CommandId, EventBus, HistoryRing,
                KernelContext, MarkBank, MotionEngine, OptionRegistry, OptionScope, OptionSpec,
                OptionValue, RegisterBank, RwLock, ServiceRegistry, TextObjectEngine,
            },
        },
        std::{collections::HashMap, sync::Arc},
    };

    use reovim_driver_session::api::ModeApi;

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
            let home_mode = VimMode::NORMAL_ID;
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

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn create_test_context() -> KernelContext {
        let options = Arc::new(OptionRegistry::default());
        let _ = options.register(
            OptionSpec::new("autoindent", "Auto indent new lines", OptionValue::bool(true))
                .with_scope(OptionScope::Buffer),
        );

        KernelContext::new(
            Arc::new(EventBus::new()),
            Arc::new(TestBufferManager::new()),
            Arc::new(MotionEngine),
            Arc::new(TextObjectEngine),
            Arc::new(RwLock::new(MarkBank::new())),
            options,
            Arc::new(ServiceRegistry::new()),
        )
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_first_non_blank_execute() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("    hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);

        // Cursor should be at column 4 (first non-blank)
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 4);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_first_non_blank_no_indent() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        // Cursor should be at column 0 (no indent)
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_first_non_blank_without_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_end_of_line_execute() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);

        // Cursor should be at column 5 (end of "hello")
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 5);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_end_of_line_empty_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_below_execute() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineBelow.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);

        // Cursor should be on line 1 (new line below)
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 1);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_below_without_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineBelow.execute(&mut runtime, &args);
        assert!(matches!(result, reovim_driver_command::CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_below_with_indent() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("    hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineBelow.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        // Cursor should be at end of indent on the new line
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 4); // 4 spaces indent
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_above_execute() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineAbove.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);

        // Cursor should be on line 0 (new line above)
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_above_without_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineAbove.execute(&mut runtime, &args);
        assert!(matches!(result, reovim_driver_command::CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_above_with_indent() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("    hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineAbove.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        // Cursor should be at end of indent on the new line (line 0)
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 4); // 4 spaces indent
    }

    // ========================================================================
    // Additional get_line_indent edge cases
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_get_line_indent_many_spaces() {
        assert_eq!(get_line_indent("        deep"), "        ");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_get_line_indent_only_newline_chars() {
        // \n is not considered whitespace by char::is_whitespace in this context
        // since we parse line by line (no newlines in a line)
        assert_eq!(get_line_indent("text"), "");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_get_line_indent_form_feed() {
        // \x0c is a whitespace character
        assert_eq!(get_line_indent("\x0chello"), "\x0c");
    }

    // ========================================================================
    // Additional execute tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_first_non_blank_all_whitespace() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("    ");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        // When all whitespace, cursor goes to column 0
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_first_non_blank_tab_indent() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("\t\thello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 2); // 2 tab chars
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_end_of_line_multiline() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Cursor on second line
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(1, 0).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 5); // end of "world"
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_end_of_line_without_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();

        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_below_empty_buffer() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineBelow.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_below_tab_indent() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("\thello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineBelow.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 1); // 1 tab char
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_above_empty_buffer() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineAbove.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_above_tab_indent() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("\thello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineAbove.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 1); // 1 tab char
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_below_multiline_at_middle() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("aaa\n  bbb\nccc");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Cursor on second line (which has indent)
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(1, 0).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineBelow.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 2);
        assert_eq!(window.cursor.column, 2); // 2 spaces from "  bbb"
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_above_multiline_at_middle() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("aaa\n  bbb\nccc");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Cursor on second line (which has indent)
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(1, 0).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineAbove.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 2); // 2 spaces from "  bbb"
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_first_non_blank_on_second_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\n    world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Cursor on second line
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(1, 0).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 4); // first non-blank on second line
    }

    // ========================================================================
    // Undo batching tests (exercises begin_insert_batch path)
    // ========================================================================

    use {
        reovim_driver_undo::{UndoKey, UndoPersistError, UndoProvider, UndoProviderRegistry},
        reovim_driver_vfs::VfsDriver,
        reovim_kernel::api::v1::{Edit, UndoResult, UndoTree},
    };

    struct MockUndoProvider {
        batch_begins: RwLock<Vec<(BufferId, Position)>>,
    }

    impl MockUndoProvider {
        fn new() -> Self {
            Self {
                batch_begins: RwLock::new(Vec::new()),
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
        fn record(&self, _: BufferId, _: Vec<Edit>, _: Position, _: Position) {}
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

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_first_non_blank_calls_begin_batch() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("  hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let begins = mock_undo.batch_begins.read();
        assert_eq!(begins.len(), 1);
        assert_eq!(begins[0].0, buffer_id);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_end_of_line_calls_begin_batch() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = EnterInsertEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let begins = mock_undo.batch_begins.read();
        assert_eq!(begins.len(), 1);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_below_calls_begin_batch() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineBelow.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let begins = mock_undo.batch_begins.read();
        assert_eq!(begins.len(), 1);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_above_calls_begin_batch() {
        let (ctx, mock_undo) = create_test_context_with_undo();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineAbove.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let begins = mock_undo.batch_begins.read();
        assert_eq!(begins.len(), 1);
    }

    // ========================================================================
    // Autoindent disabled tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_below_autoindent_disabled() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("    hello");
        let buffer_id = ctx.buffers.register(buffer);

        ctx.options
            .set("autoindent", OptionValue::bool(false), OptionScopeId::Buffer(buffer_id))
            .expect("autoindent option should be settable");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineBelow.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        // With autoindent disabled, new line should have NO indent
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 0); // No indent
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_above_autoindent_disabled() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("    hello");
        let buffer_id = ctx.buffers.register(buffer);

        ctx.options
            .set("autoindent", OptionValue::bool(false), OptionScopeId::Buffer(buffer_id))
            .expect("autoindent option should be settable");

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineAbove.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        // With autoindent disabled, new line should have NO indent
        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 0); // No indent
    }

    // ========================================================================
    // No-window edge cases (exercises get_cursor_position returning None)
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_below_no_active_window() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Create state with no active window
        let home_mode = VimMode::NORMAL_ID;
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

        let result = OpenLineBelow.execute(&mut runtime, &args);
        // Should fail because no cursor position available
        assert!(matches!(result, reovim_driver_command::CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_above_no_active_window() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Create state with no active window
        let home_mode = VimMode::NORMAL_ID;
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

        let result = OpenLineAbove.execute(&mut runtime, &args);
        // Should fail because no cursor position available
        assert!(matches!(result, reovim_driver_command::CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_first_non_blank_no_active_window() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("  hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Create state with no active window
        let home_mode = VimMode::NORMAL_ID;
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

        // Should still succeed (enters insert mode) but skips cursor movement
        let result = EnterInsertFirstNonBlank.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_enter_insert_end_of_line_no_active_window() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        // Create state with no active window
        let home_mode = VimMode::NORMAL_ID;
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

        // Should still succeed but skip cursor movement
        let result = EnterInsertEndOfLine.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);
        assert_eq!(runtime.current_mode(), &VimMode::INSERT_ID);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_below_multiline_last_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\n  world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.cursor = Position::new(1, 0).into();
        }
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineBelow.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 2);
        assert_eq!(window.cursor.column, 2); // indent from "  world"
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_open_line_above_multiline_first_line_with_indent() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("  hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let mut state = TestState::with_buffer(Some(buffer_id));
        // Cursor on first line
        let mut runtime = state.runtime(&ctx);
        let result = OpenLineAbove.execute(&mut runtime, &args);
        assert_eq!(result, reovim_driver_command::CommandResult::Success);

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 2); // indent from "  hello"
    }
}
