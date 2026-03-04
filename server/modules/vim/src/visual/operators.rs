//! Visual mode operator commands.
//!
//! Provides commands that operate on the visual selection:
//! - `d` - Delete selection
//! - `y` - Yank (copy) selection
//! - `c` - Change selection (delete + insert mode)
//! - `>` - Indent selection
//! - `<` - Dedent selection

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{
        BufferApi, SessionRuntime, TransitionContext,
        api::{ChangeTracker, ModeApi, RegisterContent, Selection, SelectionMode},
    },
    reovim_kernel::api::v1::{CommandId, Position},
};

use crate::{ids, modes::VimMode};

/// Calculate the expanded range for a selection.
///
/// This converts an API Selection into start/end positions suitable for
/// text extraction and deletion, taking selection mode into account:
/// - Character mode: End is already exclusive, use as-is
/// - Line mode: Expand to full lines including trailing newline
/// - Block mode: End is already exclusive, use as-is
///
/// Phase 8 (#465): Selection.end is EXCLUSIVE (like Rust ranges).
/// The selection (0,0) to (0,5) means columns 0..5 = "hello" (5 chars).
///
/// Returns `(start, end, is_linewise)`.
fn expand_selection_range(
    selection: &Selection,
    end_line_len: Option<usize>,
    total_lines: usize,
) -> (Position, Position, bool) {
    let start = selection.start;
    let end = selection.end;

    match selection.mode {
        SelectionMode::Line => {
            // Expand to full lines, including the trailing newline
            let start = Position::new(start.line, 0);
            // For line mode, end.line is already the exclusive end line
            // For non-last lines, extend to start of end line (includes previous line's newline)
            // For last line, end at line length
            let end_line_len = end_line_len.unwrap_or(0);
            let end = if end.line < total_lines {
                Position::new(end.line, 0)
            } else {
                // End is past buffer, cap at last line's length
                Position::new(end.line - 1, end_line_len)
            };
            (start, end, true)
        }
        // Character and Block modes: End is already exclusive - use as-is
        SelectionMode::Character | SelectionMode::Block => (start, end, false),
    }
}

/// Delete selection (d in visual mode).
///
/// Deletes the selected text and stores it in the register.
/// Returns to Normal mode after execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeleteSelection;

impl Command for DeleteSelection {
    fn id(&self) -> CommandId {
        ids::DELETE_SELECTION
    }

    fn description(&self) -> &'static str {
        "Delete visual selection"
    }
}

impl CommandHandler for DeleteSelection {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection from active window
        let Some(selection) = runtime.windows().active().and_then(|w| w.selection.clone()) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line info for expanding selection
        let end_line_len = runtime.buffer_line_len(buffer_id, selection.end.line);
        let total_lines = runtime.buffer_line_count(buffer_id).unwrap_or(1);

        // Expand selection to deletion range
        let (start, end, is_linewise) =
            expand_selection_range(&selection, end_line_len, total_lines);
        let cursor_pos = start;

        // Extract text for register with clipboard sync (#515)
        if let Some(text) = runtime.buffer_text_range(buffer_id, start, end) {
            let content = if is_linewise {
                RegisterContent::linewise(&text)
            } else {
                RegisterContent::characterwise(&text)
            };
            runtime.store_register_with_sync(args.register(), content);
        }

        // Delete the range
        runtime.delete_range(buffer_id, start, end);

        // Clear selection and set cursor
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = None;
            window.cursor = cursor_pos.into();
        }

        // #474: Notify other clients that selection was cleared
        runtime.record_selection_change(buffer_id);

        // Mode transition to Normal
        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Yank selection (y in visual mode).
///
/// Copies the selected text to the register without deleting it.
/// Returns to Normal mode after execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct YankSelection;

impl Command for YankSelection {
    fn id(&self) -> CommandId {
        ids::YANK_SELECTION
    }

    fn description(&self) -> &'static str {
        "Yank visual selection"
    }
}

impl CommandHandler for YankSelection {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection from active window
        let Some(selection) = runtime.windows().active().and_then(|w| w.selection.clone()) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line info for expanding selection
        let end_line_len = runtime.buffer_line_len(buffer_id, selection.end.line);
        let total_lines = runtime.buffer_line_count(buffer_id).unwrap_or(1);

        // Expand selection to yank range
        let (start, end, is_linewise) =
            expand_selection_range(&selection, end_line_len, total_lines);

        // Extract text for register with clipboard sync (#515)
        if let Some(text) = runtime.buffer_text_range(buffer_id, start, end) {
            let content = if is_linewise {
                RegisterContent::linewise(&text)
            } else {
                RegisterContent::characterwise(&text)
            };
            runtime.store_register_with_sync(args.register(), content);
        }

        // Clear selection (yank doesn't delete text or move cursor)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = None;
        }

        // #474: Notify other clients that selection was cleared
        runtime.record_selection_change(buffer_id);

        // Mode transition to Normal
        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Change selection (c in visual mode).
///
/// Deletes the selected text and enters Insert mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct ChangeSelection;

impl Command for ChangeSelection {
    fn id(&self) -> CommandId {
        ids::CHANGE_SELECTION
    }

    fn description(&self) -> &'static str {
        "Change visual selection (delete and enter insert mode)"
    }
}

impl CommandHandler for ChangeSelection {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection from active window
        let Some(selection) = runtime.windows().active().and_then(|w| w.selection.clone()) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line info for expanding selection
        let end_line_len = runtime.buffer_line_len(buffer_id, selection.end.line);
        let total_lines = runtime.buffer_line_count(buffer_id).unwrap_or(1);

        // Expand selection to deletion range
        let (start, end, is_linewise) =
            expand_selection_range(&selection, end_line_len, total_lines);
        let cursor_pos = start;

        // Extract text for register with clipboard sync (#515)
        if let Some(text) = runtime.buffer_text_range(buffer_id, start, end) {
            let content = if is_linewise {
                RegisterContent::linewise(&text)
            } else {
                RegisterContent::characterwise(&text)
            };
            runtime.store_register_with_sync(args.register(), content);
        }

        // Delete the range
        runtime.delete_range(buffer_id, start, end);

        // Clear selection and set cursor
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = None;
            window.cursor = cursor_pos.into();
        }

        // #474: Notify other clients that selection was cleared
        runtime.record_selection_change(buffer_id);

        // Mode transition to Insert (change = delete + insert mode)
        runtime.set_mode(VimMode::INSERT_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Indent selection (> in visual mode).
///
/// Increases indentation of selected lines.
/// Returns to Normal mode after execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct IndentSelection;

impl Command for IndentSelection {
    fn id(&self) -> CommandId {
        ids::INDENT_SELECTION
    }

    fn description(&self) -> &'static str {
        "Indent visual selection"
    }
}

impl CommandHandler for IndentSelection {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection from active window
        let Some(selection) = runtime.windows().active().and_then(|w| w.selection.clone()) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line range from normalized selection
        // Phase 8 (#465): Selection.end is EXCLUSIVE (like Rust ranges)
        let start_line = selection.start.line;
        let end_line = selection.end.line; // exclusive

        // Indent each line (add tab/spaces at start)
        // Using 4 spaces as default indent
        let indent = "    ";
        for line_idx in start_line..end_line {
            runtime.insert_text(buffer_id, Position::new(line_idx, 0), indent);
        }

        // Clear selection
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = None;
        }

        // #474: Notify other clients that selection was cleared
        runtime.record_selection_change(buffer_id);

        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}

/// Dedent selection (< in visual mode).
///
/// Decreases indentation of selected lines.
/// Returns to Normal mode after execution.
#[derive(Debug, Clone, Copy, Default)]
pub struct DedentSelection;

impl Command for DedentSelection {
    fn id(&self) -> CommandId {
        ids::DEDENT_SELECTION
    }

    fn description(&self) -> &'static str {
        "Dedent visual selection"
    }
}

impl CommandHandler for DedentSelection {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get selection from active window
        let Some(selection) = runtime.windows().active().and_then(|w| w.selection.clone()) else {
            return CommandResult::Success; // No selection - no-op
        };

        // Get line range from normalized selection
        // Phase 8 (#465): Selection.end is EXCLUSIVE (like Rust ranges)
        let start_line = selection.start.line;
        let end_line = selection.end.line; // exclusive

        // Dedent each line (remove leading whitespace, up to 4 chars or one tab)
        for line_idx in start_line..end_line {
            if let Some(line) = runtime.buffer_line(buffer_id, line_idx) {
                let mut chars_to_remove = 0;
                for (i, c) in line.chars().enumerate() {
                    if c == '\t' {
                        chars_to_remove = i + 1;
                        break;
                    } else if c == ' ' && i < 4 {
                        chars_to_remove = i + 1;
                    } else {
                        break;
                    }
                }
                if chars_to_remove > 0 {
                    let start = Position::new(line_idx, 0);
                    let end = Position::new(line_idx, chars_to_remove);
                    runtime.delete_range(buffer_id, start, end);
                }
            }
        }

        // Clear selection
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.selection = None;
        }

        // #474: Notify other clients that selection was cleared
        runtime.record_selection_change(buffer_id);

        runtime.set_mode(VimMode::NORMAL_ID, TransitionContext::new());

        CommandResult::Success
    }
}

#[cfg(test)]
#[allow(clippy::significant_drop_tightening, clippy::uninlined_format_args)]
mod tests {
    use {super::*, reovim_driver_command::Command, reovim_driver_session::api::Selection};

    // ========================================================================
    // Command metadata tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_delete_selection_id() {
        let cmd = DeleteSelection;
        assert_eq!(cmd.id(), ids::DELETE_SELECTION);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_delete_selection_description() {
        let cmd = DeleteSelection;
        assert!(cmd.description().contains("Delete"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_yank_selection_id() {
        let cmd = YankSelection;
        assert_eq!(cmd.id(), ids::YANK_SELECTION);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_yank_selection_description() {
        let cmd = YankSelection;
        assert!(cmd.description().contains("Yank"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_selection_id() {
        let cmd = ChangeSelection;
        assert_eq!(cmd.id(), ids::CHANGE_SELECTION);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_selection_description() {
        let cmd = ChangeSelection;
        assert!(cmd.description().contains("Change"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_indent_selection_id() {
        let cmd = IndentSelection;
        assert_eq!(cmd.id(), ids::INDENT_SELECTION);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_indent_selection_description() {
        let cmd = IndentSelection;
        assert!(cmd.description().contains("Indent"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_dedent_selection_id() {
        let cmd = DedentSelection;
        assert_eq!(cmd.id(), ids::DEDENT_SELECTION);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_dedent_selection_description() {
        let cmd = DedentSelection;
        assert!(cmd.description().contains("Dedent"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_all_operator_commands_debug() {
        assert!(format!("{:?}", DeleteSelection).contains("DeleteSelection"));
        assert!(format!("{:?}", YankSelection).contains("YankSelection"));
        assert!(format!("{:?}", ChangeSelection).contains("ChangeSelection"));
        assert!(format!("{:?}", IndentSelection).contains("IndentSelection"));
        assert!(format!("{:?}", DedentSelection).contains("DedentSelection"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_all_operator_commands_default() {
        let _ = DeleteSelection;
        let _ = YankSelection;
        let _ = ChangeSelection;
        let _ = IndentSelection;
        let _ = DedentSelection;
    }

    // ========================================================================
    // expand_selection_range tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_expand_character_mode_passthrough() {
        let sel = Selection::character(Position::new(0, 2), Position::new(0, 8));
        let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
        assert_eq!(start, Position::new(0, 2));
        assert_eq!(end, Position::new(0, 8));
        assert!(!is_linewise);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_expand_block_mode_passthrough() {
        let sel = Selection::block(Position::new(1, 3), Position::new(3, 7));
        let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
        assert_eq!(start, Position::new(1, 3));
        assert_eq!(end, Position::new(3, 7));
        assert!(!is_linewise);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_expand_line_mode_expands_to_full_lines() {
        let sel = Selection::line(Position::new(0, 3), Position::new(2, 0));
        let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
        assert_eq!(start, Position::new(0, 0));
        assert_eq!(end, Position::new(2, 0));
        assert!(is_linewise);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_expand_line_mode_end_past_buffer() {
        let sel = Selection::line(Position::new(0, 0), Position::new(5, 0));
        let (start, end, is_linewise) = expand_selection_range(&sel, Some(8), 3);
        assert_eq!(start, Position::new(0, 0));
        assert_eq!(end, Position::new(4, 8));
        assert!(is_linewise);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_expand_line_mode_end_at_buffer_boundary() {
        let sel = Selection::line(Position::new(0, 0), Position::new(3, 0));
        let (start, end, is_linewise) = expand_selection_range(&sel, Some(5), 3);
        assert_eq!(start, Position::new(0, 0));
        assert_eq!(end, Position::new(2, 5));
        assert!(is_linewise);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_expand_line_mode_within_buffer() {
        let sel = Selection::line(Position::new(1, 0), Position::new(3, 0));
        let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 10);
        assert_eq!(start, Position::new(1, 0));
        assert_eq!(end, Position::new(3, 0));
        assert!(is_linewise);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_expand_line_mode_none_end_line_len() {
        let sel = Selection::line(Position::new(0, 0), Position::new(5, 0));
        let (start, end, is_linewise) = expand_selection_range(&sel, None, 3);
        assert_eq!(start, Position::new(0, 0));
        // end_line_len is None => 0
        assert_eq!(end, Position::new(4, 0));
        assert!(is_linewise);
    }

    // ========================================================================
    // Execute tests (require SessionRuntime)
    // ========================================================================

    use {
        reovim_driver_command::CommandHandler,
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, SessionRuntime, WindowLayout,
            api::{CommandExecutor, RegisterApi},
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

    // --- DeleteSelection execute ---

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_delete_selection_execute_no_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new(); // No buffer_id set
        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        let result = DeleteSelection.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_delete_selection_execute_no_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        // No selection - should be no-op
        let result = DeleteSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_delete_selection_execute_character_mode() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        // Set character selection for "hello" (columns 0-5, exclusive end)
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = DeleteSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Selection should be cleared
        let window = runtime.windows().active().unwrap();
        assert!(window.selection.is_none());

        // Cursor should be at start of deletion
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 0);

        // Register should contain deleted text
        let reg = runtime.get_register(None);
        assert!(reg.is_some());
        let reg = reg.unwrap();
        assert!(reg.is_characterwise());
        assert_eq!(reg.text, "hello");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_delete_selection_execute_clears_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("abcdef");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::character(Position::new(0, 1), Position::new(0, 4)));
        }
        let mut runtime = state.runtime(&ctx);

        DeleteSelection.execute(&mut runtime, &args);

        let window = runtime.windows().active().unwrap();
        assert!(window.selection.is_none());
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 1);
    }

    // --- YankSelection execute ---

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_yank_selection_execute_no_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();
        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        let result = YankSelection.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_yank_selection_execute_no_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        let result = YankSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_yank_selection_execute_character_mode() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = YankSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Selection should be cleared
        let window = runtime.windows().active().unwrap();
        assert!(window.selection.is_none());

        // Register should contain yanked text
        let reg = runtime.get_register(None);
        assert!(reg.is_some());
        let reg = reg.unwrap();
        assert!(reg.is_characterwise());
        assert_eq!(reg.text, "hello");
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_yank_selection_execute_does_not_delete_text() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
        }
        let mut runtime = state.runtime(&ctx);

        YankSelection.execute(&mut runtime, &args);

        // Buffer should still contain original text (yank doesn't delete)
        let line = runtime.buffer_line(buffer_id, 0);
        assert_eq!(line, Some("hello world".to_string()));
    }

    // --- ChangeSelection execute ---

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_selection_execute_no_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();
        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        let result = ChangeSelection.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_selection_execute_no_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        let result = ChangeSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_selection_execute_character_mode() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = ChangeSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Selection should be cleared
        let window = runtime.windows().active().unwrap();
        assert!(window.selection.is_none());

        // Cursor at start of deletion
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 0);

        // Register should contain deleted text
        let reg = runtime.get_register(None);
        assert!(reg.is_some());
        let reg = reg.unwrap();
        assert!(reg.is_characterwise());
        assert_eq!(reg.text, "hello");
    }

    // --- IndentSelection execute ---

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_indent_selection_execute_no_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();
        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        let result = IndentSelection.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_indent_selection_execute_no_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        let result = IndentSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_indent_selection_execute_indents_lines() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld\nfoo");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        // Select lines 0..2 (first two lines)
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = IndentSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Lines 0 and 1 should be indented with 4 spaces
        let line0 = runtime.buffer_line(buffer_id, 0);
        assert_eq!(line0, Some("    hello".to_string()));
        let line1 = runtime.buffer_line(buffer_id, 1);
        assert_eq!(line1, Some("    world".to_string()));
        // Line 2 should be unchanged
        let line2 = runtime.buffer_line(buffer_id, 2);
        assert_eq!(line2, Some("foo".to_string()));

        // Selection should be cleared
        let window = runtime.windows().active().unwrap();
        assert!(window.selection.is_none());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_indent_selection_execute_single_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::line(Position::new(1, 0), Position::new(2, 0)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = IndentSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Only line 1 should be indented
        let line0 = runtime.buffer_line(buffer_id, 0);
        assert_eq!(line0, Some("hello".to_string()));
        let line1 = runtime.buffer_line(buffer_id, 1);
        assert_eq!(line1, Some("    world".to_string()));
    }

    // --- DedentSelection execute ---

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_dedent_selection_execute_no_buffer() {
        let ctx = create_test_context();
        let args = CommandContext::new();
        let mut state = TestState::with_buffer(None);
        let mut runtime = state.runtime(&ctx);

        let result = DedentSelection.execute(&mut runtime, &args);
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_dedent_selection_execute_no_selection() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("    hello\n    world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        let mut runtime = state.runtime(&ctx);

        let result = DedentSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_dedent_selection_execute_removes_spaces() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("    hello\n    world\nfoo");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = DedentSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Lines should be dedented (4 spaces removed)
        let line0 = runtime.buffer_line(buffer_id, 0);
        assert_eq!(line0, Some("hello".to_string()));
        let line1 = runtime.buffer_line(buffer_id, 1);
        assert_eq!(line1, Some("world".to_string()));
        // Line 2 unchanged
        let line2 = runtime.buffer_line(buffer_id, 2);
        assert_eq!(line2, Some("foo".to_string()));

        // Selection should be cleared
        let window = runtime.windows().active().unwrap();
        assert!(window.selection.is_none());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_dedent_selection_execute_removes_tab() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("\thello\n\tworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = DedentSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Tabs should be removed
        let line0 = runtime.buffer_line(buffer_id, 0);
        assert_eq!(line0, Some("hello".to_string()));
        let line1 = runtime.buffer_line(buffer_id, 1);
        assert_eq!(line1, Some("world".to_string()));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_dedent_selection_execute_no_leading_whitespace() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = DedentSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Lines with no leading whitespace should remain unchanged
        let line0 = runtime.buffer_line(buffer_id, 0);
        assert_eq!(line0, Some("hello".to_string()));
        let line1 = runtime.buffer_line(buffer_id, 1);
        assert_eq!(line1, Some("world".to_string()));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_dedent_selection_execute_partial_spaces() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("  hello\n      world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = DedentSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // "  hello" has 2 spaces -> removed, "      world" has 6 spaces -> 4 removed
        let line0 = runtime.buffer_line(buffer_id, 0);
        assert_eq!(line0, Some("hello".to_string()));
        let line1 = runtime.buffer_line(buffer_id, 1);
        assert_eq!(line1, Some("  world".to_string()));
    }

    // ========================================================================
    // Additional expand_selection_range edge cases
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_expand_character_mode_single_char() {
        let sel = Selection::character(Position::new(0, 0), Position::new(0, 1));
        let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
        assert_eq!(start, Position::new(0, 0));
        assert_eq!(end, Position::new(0, 1));
        assert!(!is_linewise);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_expand_line_mode_single_line() {
        let sel = Selection::line(Position::new(2, 0), Position::new(3, 0));
        let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
        assert_eq!(start, Position::new(2, 0));
        assert_eq!(end, Position::new(3, 0));
        assert!(is_linewise);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_expand_block_mode_single_cell() {
        let sel = Selection::block(Position::new(0, 0), Position::new(0, 1));
        let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
        assert_eq!(start, Position::new(0, 0));
        assert_eq!(end, Position::new(0, 1));
        assert!(!is_linewise);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_expand_character_mode_multiline() {
        let sel = Selection::character(Position::new(0, 3), Position::new(2, 5));
        let (start, end, is_linewise) = expand_selection_range(&sel, Some(10), 5);
        assert_eq!(start, Position::new(0, 3));
        assert_eq!(end, Position::new(2, 5));
        assert!(!is_linewise);
    }

    // ========================================================================
    // Additional operator execute edge cases
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_delete_selection_middle_of_line() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::character(Position::new(0, 5), Position::new(0, 11)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = DeleteSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let line = runtime.buffer_line(buffer_id, 0);
        assert_eq!(line, Some("hello".to_string()));

        let window = runtime.windows().active().unwrap();
        assert_eq!(window.cursor.column, 5);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_yank_selection_line_mode() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld\nfoo");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = YankSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Buffer should be unchanged
        assert_eq!(runtime.buffer_line(buffer_id, 0), Some("hello".to_string()));
        assert_eq!(runtime.buffer_line(buffer_id, 1), Some("world".to_string()));

        // Register should contain yanked lines
        let reg = runtime.get_register(None);
        assert!(reg.is_some());
        let reg = reg.unwrap();
        assert!(reg.is_linewise());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_selection_enters_insert_mode() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
        }
        let mut runtime = state.runtime(&ctx);

        ChangeSelection.execute(&mut runtime, &args);

        // Should enter insert mode
        assert_eq!(runtime.current_mode().name(), crate::modes::VimMode::INSERT_ID.name());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_indent_selection_already_indented() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string("    hello\n    world");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = IndentSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Lines should get additional 4 spaces
        let line0 = runtime.buffer_line(buffer_id, 0);
        assert_eq!(line0, Some("        hello".to_string()));
        let line1 = runtime.buffer_line(buffer_id, 1);
        assert_eq!(line1, Some("        world".to_string()));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_dedent_selection_single_space() {
        let ctx = create_test_context();
        let buffer = Buffer::from_string(" hello");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::line(Position::new(0, 0), Position::new(1, 0)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = DedentSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        let line0 = runtime.buffer_line(buffer_id, 0);
        assert_eq!(line0, Some("hello".to_string()));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_delete_selection_clones() {
        let _: DeleteSelection = DeleteSelection;
        let _: YankSelection = YankSelection;
        let _: ChangeSelection = ChangeSelection;
        let _: IndentSelection = IndentSelection;
        let _: DedentSelection = DedentSelection;
    }

    // ========================================================================
    // Coverage: None-path and linewise tests
    // ========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_delete_selection_nonexistent_buffer() {
        // Exercise the None path of buffer_text_range (line 107)
        let ctx = create_test_context();
        let fake_id = BufferId::from_raw(9999);

        let mut args = CommandContext::new();
        args.set_buffer_id(fake_id);
        let mut state = TestState::with_buffer(Some(fake_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = DeleteSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_yank_selection_nonexistent_buffer() {
        // Exercise the None path of buffer_text_range (line 169)
        let ctx = create_test_context();
        let fake_id = BufferId::from_raw(9999);

        let mut args = CommandContext::new();
        args.set_buffer_id(fake_id);
        let mut state = TestState::with_buffer(Some(fake_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = YankSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_selection_nonexistent_buffer() {
        // Exercise the None path of buffer_text_range (line 227)
        let ctx = create_test_context();
        let fake_id = BufferId::from_raw(9999);

        let mut args = CommandContext::new();
        args.set_buffer_id(fake_id);
        let mut state = TestState::with_buffer(Some(fake_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::character(Position::new(0, 0), Position::new(0, 5)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = ChangeSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_change_selection_linewise() {
        // Exercise the is_linewise branch in ChangeSelection (line 222)
        let ctx = create_test_context();
        let buffer = Buffer::from_string("hello\nworld\nfoo");
        let buffer_id = ctx.buffers.register(buffer);

        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let mut state = TestState::with_buffer(Some(buffer_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::line(Position::new(0, 0), Position::new(2, 0)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = ChangeSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);

        // Register should contain linewise content
        let reg = runtime.get_register(None);
        assert!(reg.is_some());
        let reg = reg.unwrap();
        assert!(reg.is_linewise());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_dedent_selection_nonexistent_buffer() {
        // Exercise the None path of buffer_line (line 348)
        let ctx = create_test_context();
        let fake_id = BufferId::from_raw(9999);

        let mut args = CommandContext::new();
        args.set_buffer_id(fake_id);
        let mut state = TestState::with_buffer(Some(fake_id));
        if let Some(w) = state.windows.active_mut() {
            w.selection = Some(Selection::line(Position::new(0, 0), Position::new(3, 0)));
        }
        let mut runtime = state.runtime(&ctx);

        let result = DedentSelection.execute(&mut runtime, &args);
        assert_eq!(result, CommandResult::Success);
    }
}
