//! Paste commands.
//!
//! Provides paste commands:
//! - `PasteAfter` (p)
//! - `PasteBefore` (P)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{SessionRuntime, api::BufferApi},
    reovim_kernel::api::v1::{CommandId, Position},
};

use crate::ids;

/// Paste from register after cursor (p).
///
/// - Linewise paste: insert below current line
/// - Characterwise paste: insert after cursor position
#[derive(Debug, Clone, Copy, Default)]
pub struct PasteAfter;

impl Command for PasteAfter {
    fn id(&self) -> CommandId {
        ids::PASTE_AFTER
    }

    fn description(&self) -> &'static str {
        "Paste after cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::optional("count", ArgKind::Count, "Number of times to paste"),
            ArgSpec::optional("register", ArgKind::Register, "Source register"),
        ]
    }
}

impl CommandHandler for PasteAfter {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let count = args.count().unwrap_or(1);
        let register = args.register();

        // Get register content with clipboard fallback for +/* (#515)
        let content = runtime.get_register_with_clipboard(register);

        let Some(content) = content else {
            return CommandResult::Success; // Empty register
        };

        if content.is_empty() {
            return CommandResult::Success; // Nothing to paste
        }

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        if content.is_linewise() {
            // Paste below current line
            // Get line count via BufferApi
            let line_count = runtime.buffer_line_count(buffer_id).unwrap_or(0);

            // Build paste text (repeated count times, strip trailing newline for clean insert)
            let paste_text = content.text.repeat(count);
            let paste_text = paste_text.trim_end_matches('\n');

            if line_count == 0 {
                // Empty buffer: just insert the content at origin
                runtime.insert_text(buffer_id, Position::new(0, 0), paste_text);
                // Update cursor via per-client Window (#471)
                if let Some(window) = runtime.windows_mut().active_mut() {
                    window.cursor = Position::new(0, 0).into();
                }
                return CommandResult::Success;
            }

            // Get line length via BufferApi
            let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);

            // Insert newline then content at end of current line
            let insert_pos = Position::new(pos.line, line_len);
            let insert_text = format!("\n{paste_text}");
            runtime.insert_text(buffer_id, insert_pos, &insert_text);

            // Position cursor on first character of first pasted line
            let new_line = pos.line + 1;
            // Update cursor via per-client Window (#471)
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = Position::new(new_line, 0).into();
            }
        } else {
            // Characterwise: paste after cursor
            // Get line length via BufferApi
            let line_len = runtime.buffer_line_len(buffer_id, pos.line).unwrap_or(0);
            let insert_col = if line_len == 0 {
                0
            } else {
                (pos.column + 1).min(line_len)
            };

            let insert_pos = Position::new(pos.line, insert_col);
            let paste_text = content.text.repeat(count);
            runtime.insert_text(buffer_id, insert_pos, &paste_text);

            // Calculate final cursor position (at end of pasted text - 1 for Vim behavior)
            // Count lines in pasted text
            let lines_in_paste: Vec<&str> = paste_text.lines().collect();
            let cursor_after = if lines_in_paste.len() > 1 {
                // Multi-line paste: cursor at end of last line
                let last_line_len = lines_in_paste.last().map_or(0, |l| l.chars().count());
                let final_line = insert_pos.line + lines_in_paste.len() - 1;
                let col = if last_line_len > 0 {
                    last_line_len - 1
                } else {
                    0
                };
                Position::new(final_line, col)
            } else {
                // Single-line paste: cursor at end of pasted text - 1
                let text_len = paste_text.chars().count();
                let final_col = insert_col + text_len;
                let col = if final_col > 0 { final_col - 1 } else { 0 };
                Position::new(pos.line, col)
            };
            // Update cursor via per-client Window (#471)
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = cursor_after.into();
            }
        }

        CommandResult::Success
    }
}

/// Paste from register before cursor (P).
///
/// - Linewise paste: insert above current line
/// - Characterwise paste: insert at cursor position
#[derive(Debug, Clone, Copy, Default)]
pub struct PasteBefore;

impl Command for PasteBefore {
    fn id(&self) -> CommandId {
        ids::PASTE_BEFORE
    }

    fn description(&self) -> &'static str {
        "Paste before cursor"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::optional("count", ArgKind::Count, "Number of times to paste"),
            ArgSpec::optional("register", ArgKind::Register, "Source register"),
        ]
    }
}

impl CommandHandler for PasteBefore {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let count = args.count().unwrap_or(1);
        let register = args.register();

        // Get register content with clipboard fallback for +/* (#515)
        let content = runtime.get_register_with_clipboard(register);

        let Some(content) = content else {
            return CommandResult::Success; // Empty register
        };

        if content.is_empty() {
            return CommandResult::Success; // Nothing to paste
        }

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Build paste text (repeated count times)
        let paste_text = content.text.repeat(count);

        if content.is_linewise() {
            // Paste above current line
            // Strip trailing newline for clean insert
            let paste_text = paste_text.trim_end_matches('\n');

            // Insert content then newline at start of current line
            let insert_pos = Position::new(pos.line, 0);
            let insert_text = format!("{paste_text}\n");
            runtime.insert_text(buffer_id, insert_pos, &insert_text);

            // Position cursor on first character of first pasted line
            // Update cursor via per-client Window (#471)
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = Position::new(pos.line, 0).into();
            }
        } else {
            // Characterwise: paste at cursor position (before)
            runtime.insert_text(buffer_id, pos, &paste_text);

            // Calculate final cursor position (at end of pasted text - 1 for Vim behavior)
            let lines_in_paste: Vec<&str> = paste_text.lines().collect();
            let cursor_after = if lines_in_paste.len() > 1 {
                // Multi-line paste: cursor at end of last line
                let last_line_len = lines_in_paste.last().map_or(0, |l| l.chars().count());
                let final_line = pos.line + lines_in_paste.len() - 1;
                let col = if last_line_len > 0 {
                    last_line_len - 1
                } else {
                    0
                };
                Position::new(final_line, col)
            } else {
                // Single-line paste: cursor at end of pasted text - 1
                let text_len = paste_text.chars().count();
                let final_col = pos.column + text_len;
                let col = if final_col > 0 { final_col - 1 } else { 0 };
                Position::new(pos.line, col)
            };
            // Update cursor via per-client Window (#471)
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor = cursor_after.into();
            }
        }

        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::{ArgKind, ArgValue, CommandContext},
        reovim_driver_session::{
            ClientId, ExtensionMap, RegisterContent, Session, SessionRuntime, Window, WindowLayout,
            api::{CommandExecutor, CommandHandle},
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
        fn get_handle(&self, _id: &KernelCommandId) -> Option<std::sync::Arc<dyn CommandHandle>> {
            None
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
    // PasteAfter tests
    // =========================================================================

    #[test]
    fn test_paste_after_id() {
        assert_eq!(PasteAfter.id().name(), "paste-after");
    }

    #[test]
    fn test_paste_after_description() {
        assert_eq!(PasteAfter.description(), "Paste after cursor");
    }

    #[test]
    fn test_paste_after_args() {
        let args = PasteAfter.args();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
        assert_eq!(args[1].name, "register");
        assert_eq!(args[1].kind, ArgKind::Register);
    }

    #[test]
    fn test_paste_after_no_buffer_returns_error() {
        let kernel = create_test_context();
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
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_paste_after_empty_register_returns_success() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_paste_after_empty_content_returns_success() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise(""));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_paste_after_no_window_returns_error() {
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
        registers.set(RegisterContent::characterwise("X"));
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
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_paste_after_linewise() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line one\nline two");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::linewise("pasted\n"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 3);
        assert_eq!(buf_read.line(0), Some("line one"));
        assert_eq!(buf_read.line(1), Some("pasted"));
        assert_eq!(buf_read.line(2), Some("line two"));
        drop(buf_read);

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 0);
    }

    #[test]
    fn test_paste_after_linewise_empty_buffer() {
        let kernel = create_test_context();
        let buffer = Buffer::new();
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state
            .registers
            .set(RegisterContent::linewise("pasted line\n"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 0);
    }

    #[test]
    fn test_paste_after_characterwise() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("XYZ"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("hXYZello"));

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 3);
    }

    #[test]
    fn test_paste_after_characterwise_on_empty_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("AB"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("AB"));
    }

    #[test]
    fn test_paste_after_characterwise_multiline() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("X\nY"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    fn test_paste_after_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("X"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(3));
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("hXXXello"));
    }

    #[test]
    fn test_paste_after_linewise_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line one");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::linewise("pasted\n"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 3);
        assert_eq!(buf_read.line(1), Some("pasted"));
        assert_eq!(buf_read.line(2), Some("pasted"));
        drop(buf_read);
    }

    // =========================================================================
    // PasteBefore tests
    // =========================================================================

    #[test]
    fn test_paste_before_id() {
        assert_eq!(PasteBefore.id().name(), "paste-before");
    }

    #[test]
    fn test_paste_before_description() {
        assert_eq!(PasteBefore.description(), "Paste before cursor");
    }

    #[test]
    fn test_paste_before_args() {
        let args = PasteBefore.args();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].name, "count");
        assert_eq!(args[0].kind, ArgKind::Count);
        assert_eq!(args[1].name, "register");
        assert_eq!(args[1].kind, ArgKind::Register);
    }

    #[test]
    fn test_paste_before_no_buffer_returns_error() {
        let kernel = create_test_context();
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
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_paste_before_empty_register_returns_success() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_paste_before_empty_content_returns_success() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise(""));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_success());
    }

    #[test]
    fn test_paste_before_no_window_returns_error() {
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
        registers.set(RegisterContent::characterwise("X"));
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
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_paste_before_linewise() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line one\nline two");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::linewise("pasted\n"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 3);
        assert_eq!(buf_read.line(0), Some("pasted"));
        assert_eq!(buf_read.line(1), Some("line one"));
        assert_eq!(buf_read.line(2), Some("line two"));
        drop(buf_read);

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 0);
    }

    #[test]
    fn test_paste_before_characterwise() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("XYZ"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("XYZhello"));

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 2);
    }

    #[test]
    fn test_paste_before_characterwise_multiline() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("A\nB"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    fn test_paste_before_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("X"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("XXhello"));
    }

    #[test]
    fn test_paste_after_characterwise_single_char_empty_line() {
        // Tests paste on an empty line with characterwise content
        // Covers the branch at line 104-108 where line_len is 0
        let kernel = create_test_context();
        let buffer = Buffer::from_string("");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("test"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("test"));
    }

    #[test]
    fn test_paste_before_linewise_with_count_multiple() {
        // Tests linewise paste with count, multi-line content
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line one");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::linewise("A\nB\n"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 5);
        assert_eq!(buf_read.line(0), Some("A"));
        assert_eq!(buf_read.line(1), Some("B"));
        assert_eq!(buf_read.line(2), Some("A"));
        assert_eq!(buf_read.line(3), Some("B"));
        assert_eq!(buf_read.line(4), Some("line one"));
        drop(buf_read);
    }

    // =========================================================================
    // Metadata tests
    // =========================================================================

    #[test]
    fn test_paste_after_default() {
        let cmd = PasteAfter;
        assert_eq!(cmd.id().name(), "paste-after");
    }

    #[test]
    fn test_paste_before_default() {
        let cmd = PasteBefore;
        assert_eq!(cmd.id().name(), "paste-before");
    }

    // =========================================================================
    // Characterwise paste cursor position - multiline with empty last line
    // =========================================================================

    #[test]
    fn test_paste_after_characterwise_multiline_empty_last_line() {
        // "A\n".lines() = ["A"] (single element), so single-line branch is taken.
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("A\n"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // "A\n".lines() => ["A"], treated as single-line paste
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 2);
    }

    #[test]
    fn test_paste_before_characterwise_multiline_empty_last_line() {
        // "A\n".lines() = ["A"] (single element), so single-line branch is taken.
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("A\n"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // "A\n".lines() => ["A"], treated as single-line paste
        assert_eq!(window.cursor.line, 0);
        assert_eq!(window.cursor.column, 1);
    }

    // =========================================================================
    // Paste after characterwise with count and multiline
    // =========================================================================

    #[test]
    fn test_paste_after_characterwise_multiline_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("X\nY"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // count=2 means "X\nYX\nY" pasted
        // Last line "Y" has 1 char, col = 0
        assert_eq!(window.cursor.column, 0);
    }

    // =========================================================================
    // Paste before with count characterwise
    // =========================================================================

    #[test]
    fn test_paste_before_characterwise_with_count() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("AB"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("count", ArgValue::Count(2));
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("ABABhello"));

        drop(runtime);
        let window = state.windows.active().unwrap();
        // "ABAB" is 4 chars, col = 4-1 = 3
        assert_eq!(window.cursor.column, 3);
    }

    // =========================================================================
    // Paste after linewise on non-first line
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_paste_after_linewise_on_second_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::linewise("pasted\n"));
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(1, 0).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 4);
        assert_eq!(buf_read.line(1), Some("line 2"));
        assert_eq!(buf_read.line(2), Some("pasted"));
        assert_eq!(buf_read.line(3), Some("line 3"));
        drop(buf_read);

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 2);
        assert_eq!(window.cursor.column, 0);
    }

    // =========================================================================
    // Paste before linewise on non-first line
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_paste_before_linewise_on_second_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::linewise("pasted\n"));
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(1, 0).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 4);
        assert_eq!(buf_read.line(0), Some("line 1"));
        assert_eq!(buf_read.line(1), Some("pasted"));
        assert_eq!(buf_read.line(2), Some("line 2"));
        drop(buf_read);

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 0);
    }

    // =========================================================================
    // Paste after characterwise at end of line
    // =========================================================================

    #[test]
    fn test_paste_after_characterwise_multiline_last_line_empty() {
        // "A\n\n".lines() = ["A", ""], last_line_len = 0, col = 0 branch
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("A\n\n"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // "A\n\n".lines() => ["A", ""], len=2, last_line_len=0, col=0
        // insert at (0, 1), final_line = 0 + 2 - 1 = 1
        assert_eq!(window.cursor.column, 0);
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    fn test_paste_before_characterwise_multiline_last_line_empty() {
        // "A\n\n".lines() = ["A", ""], last_line_len = 0, col = 0 branch
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("A\n\n"));
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        drop(runtime);
        let window = state.windows.active().unwrap();
        // "A\n\n".lines() => ["A", ""], len=2, last_line_len=0, col=0
        // insert at (0, 0), final_line = 0 + 2 - 1 = 1
        assert_eq!(window.cursor.column, 0);
        assert_eq!(window.cursor.line, 1);
    }

    #[test]
    fn test_paste_after_named_register_not_set_returns_success() {
        // When a named register (e.g., 'a') hasn't been set, get_register returns None.
        // This covers line 54: `return CommandResult::Success; // Empty register`
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("register", ArgValue::Register('a'));
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Buffer should be unchanged
        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("hello"));
    }

    #[test]
    fn test_paste_before_named_register_not_set_returns_success() {
        // When a named register (e.g., 'a') hasn't been set, get_register returns None.
        // This covers line 181: `return CommandResult::Success; // Empty register`
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        args.set("register", ArgValue::Register('a'));
        let result = PasteBefore.execute(&mut runtime, &args);
        assert!(result.is_success());

        // Buffer should be unchanged
        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("hello"));
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_paste_after_characterwise_at_end_of_line() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        state.registers.set(RegisterContent::characterwise("XY"));
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 4).into(); // last char 'o'
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = PasteAfter.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("helloXY"));

        drop(runtime);
        let window = state.windows.active().unwrap();
        // Cursor at end of pasted text - 1
        assert_eq!(window.cursor.column, 6);
    }
}
