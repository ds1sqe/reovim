//! Insert mode edit commands.
//!
//! Provides commands for editing in insert mode:
//! - `InsertNewline` (Enter)
//! - `InsertTab` (Tab)

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{BufferApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, OptionScopeId, Position},
};

use crate::ids;

/// Extract the leading whitespace (indent) from a line.
///
/// Returns a string slice containing only the leading whitespace characters.
/// This preserves the exact mix of tabs and spaces.
#[must_use]
fn get_line_indent(line: &str) -> &str {
    let non_ws_pos = line
        .char_indices()
        .find(|(_, c)| !c.is_whitespace())
        .map_or(line.len(), |(i, _)| i);
    &line[..non_ws_pos]
}

/// Insert a newline at cursor position (Enter in insert mode).
#[derive(Debug, Clone, Copy, Default)]
pub struct InsertNewline;

impl Command for InsertNewline {
    fn id(&self) -> CommandId {
        ids::INSERT_NEWLINE
    }

    fn description(&self) -> &'static str {
        "Insert newline at cursor position"
    }
}

impl CommandHandler for InsertNewline {
    #[cfg_attr(coverage_nightly, coverage(off))]
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

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Get indent from current line if autoindent is enabled
        let indent = if autoindent {
            runtime
                .buffer_line(buffer_id, pos.line)
                .map(|line| get_line_indent(&line).to_owned())
                .unwrap_or_default()
        } else {
            String::new()
        };

        // Insert newline + indent (splits the line at cursor position)
        let insert_text = format!("\n{indent}");
        runtime.insert_text(buffer_id, pos, &insert_text);

        // Update cursor position to end of indent on new line
        let indent_len = indent.chars().count();
        let new_pos = Position::new(pos.line + 1, indent_len);
        // Update cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = new_pos.into();
        }

        CommandResult::Success
    }
}

/// Insert a tab at cursor position (Tab in insert mode).
///
/// Respects `expandtab` and `tabstop` options.
#[derive(Debug, Clone, Copy, Default)]
pub struct InsertTab;

impl Command for InsertTab {
    fn id(&self) -> CommandId {
        ids::INSERT_TAB
    }

    fn description(&self) -> &'static str {
        "Insert tab at cursor position"
    }
}

impl CommandHandler for InsertTab {
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get options (escape hatch - OptionsApi not yet available)
        let scope = OptionScopeId::Buffer(buffer_id);
        let expandtab = runtime
            .kernel()
            .options
            .get("expandtab", scope)
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let tabstop = runtime
            .kernel()
            .options
            .get("tabstop", scope)
            .and_then(|v| v.as_int())
            .map_or(4, |n| n.max(1) as usize);

        let text = if expandtab {
            " ".repeat(tabstop)
        } else {
            "\t".to_string()
        };

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        // Insert tab/spaces at current position
        runtime.insert_text(buffer_id, pos, &text);

        // Update cursor position to after inserted text
        let text_len = text.chars().count();
        let new_pos = Position::new(pos.line, pos.column + text_len);
        // Update cursor via per-client Window (#471)
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor = new_pos.into();
        }

        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_driver_command::CommandContext,
        reovim_driver_session::{
            ClientId, ExtensionMap, Session, SessionRuntime, Window, WindowLayout,
            api::CommandExecutor,
        },
        reovim_kernel::api::{
            ServiceRegistry,
            v1::{
                Buffer, BufferError, BufferId, BufferManager, CommandId as KernelCommandId,
                EventBus, KernelContext, MarkBank, ModeId, ModeStack, ModuleId, MotionEngine,
                OptionRegistry, OptionScope, OptionSpec, OptionValue, RegisterBank, RwLock,
                TextObjectEngine,
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
        compositor: Option<Box<dyn reovim_driver_display::layout::RootCompositor>>,
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
                &mut self.compositor,
                kernel,
                executor,
            )
        }
    }

    // =========================================================================
    // get_line_indent tests
    // =========================================================================

    #[test]
    fn test_get_line_indent_no_indent() {
        assert_eq!(get_line_indent("hello"), "");
    }

    #[test]
    fn test_get_line_indent_spaces() {
        assert_eq!(get_line_indent("    hello"), "    ");
    }

    #[test]
    fn test_get_line_indent_tabs() {
        assert_eq!(get_line_indent("\t\thello"), "\t\t");
    }

    #[test]
    fn test_get_line_indent_mixed() {
        assert_eq!(get_line_indent("  \t hello"), "  \t ");
    }

    #[test]
    fn test_get_line_indent_all_whitespace() {
        assert_eq!(get_line_indent("   "), "   ");
    }

    #[test]
    fn test_get_line_indent_empty() {
        assert_eq!(get_line_indent(""), "");
    }

    // =========================================================================
    // InsertNewline tests
    // =========================================================================

    #[test]
    fn test_insert_newline_id() {
        let cmd = InsertNewline;
        assert_eq!(cmd.id().name(), "insert-newline");
    }

    #[test]
    fn test_insert_newline_description() {
        let cmd = InsertNewline;
        assert_eq!(cmd.description(), "Insert newline at cursor position");
    }

    #[test]
    fn test_insert_newline_no_buffer_returns_error() {
        let kernel = KernelContext::default();
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &mut compositor,
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = InsertNewline.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_insert_newline_no_window_returns_error() {
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
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &mut compositor,
            &kernel,
            &executor,
        );
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = InsertNewline.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_insert_newline_at_end() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 5).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InsertNewline.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 2);
        assert_eq!(buf_read.line(0), Some("hello"));
        assert_eq!(buf_read.line(1), Some(""));

        drop(buf_read);
        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 0);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_insert_newline_at_middle() {
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

        let result = InsertNewline.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 2);
        assert_eq!(buf_read.line(0), Some("hello"));
        assert_eq!(buf_read.line(1), Some(" world"));
        drop(buf_read);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_insert_newline_with_autoindent() {
        let kernel = create_test_context();

        // Register the option spec so set() can find it
        let _ = kernel.options.register(
            OptionSpec::new("autoindent", "Auto indent new lines", OptionValue::bool(true))
                .with_scope(OptionScope::Buffer),
        );

        let buffer = Buffer::from_string("    indented line");
        let buffer_id = kernel.buffers.register(buffer);

        kernel
            .options
            .set("autoindent", OptionValue::bool(true), OptionScopeId::Buffer(buffer_id))
            .expect("autoindent option should be settable");

        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 17).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InsertNewline.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 2);
        assert_eq!(buf_read.line(1), Some("    "));

        drop(buf_read);
        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 4);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_insert_newline_without_autoindent() {
        let kernel = create_test_context();

        // Register the option spec so set() can find it
        let _ = kernel.options.register(
            OptionSpec::new("autoindent", "Auto indent new lines", OptionValue::bool(true))
                .with_scope(OptionScope::Buffer),
        );

        let buffer = Buffer::from_string("    indented line");
        let buffer_id = kernel.buffers.register(buffer);

        kernel
            .options
            .set("autoindent", OptionValue::bool(false), OptionScopeId::Buffer(buffer_id))
            .expect("autoindent option should be settable");

        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 17).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InsertNewline.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line(1), Some(""));

        drop(buf_read);
        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 0);
    }

    // =========================================================================
    // InsertTab tests
    // =========================================================================

    #[test]
    fn test_insert_tab_id() {
        let cmd = InsertTab;
        assert_eq!(cmd.id().name(), "insert-tab");
    }

    #[test]
    fn test_insert_tab_description() {
        let cmd = InsertTab;
        assert_eq!(cmd.description(), "Insert tab at cursor position");
    }

    #[test]
    fn test_insert_tab_no_buffer_returns_error() {
        let kernel = KernelContext::default();
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode.clone());
        let executor = StubExecutor;
        let mut mode_stack = ModeStack::new(mode);
        let mut windows = WindowLayout::empty();
        let mut extensions = ExtensionMap::new();
        let mut compositor = None;
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &mut compositor,
            &kernel,
            &executor,
        );
        let args = CommandContext::new();
        let result = InsertTab.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_insert_tab_no_window_returns_error() {
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
        let mut runtime = SessionRuntime::new(
            &mut session,
            &mut mode_stack,
            &mut windows,
            &mut extensions,
            &mut compositor,
            &kernel,
            &executor,
        );
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);
        let result = InsertTab.execute(&mut runtime, &args);
        assert!(result.is_error());
    }

    #[test]
    fn test_insert_tab_expandtab() {
        let kernel = create_test_context();

        // Register the option specs so set() can find them
        let _ = kernel.options.register(
            OptionSpec::new("expandtab", "Use spaces instead of tabs", OptionValue::bool(true))
                .with_scope(OptionScope::Buffer),
        );
        let _ = kernel.options.register(
            OptionSpec::new("tabstop", "Number of spaces per tab", OptionValue::int(4))
                .with_scope(OptionScope::Buffer),
        );

        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);

        kernel
            .options
            .set("expandtab", OptionValue::bool(true), OptionScopeId::Buffer(buffer_id))
            .expect("expandtab option should be settable");
        kernel
            .options
            .set("tabstop", OptionValue::int(4), OptionScopeId::Buffer(buffer_id))
            .expect("tabstop option should be settable");

        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InsertTab.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("    hello"));

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 4);
    }

    #[test]
    fn test_insert_tab_noexpandtab() {
        let kernel = create_test_context();

        // Register the option spec so set() can find it
        let _ = kernel.options.register(
            OptionSpec::new("expandtab", "Use spaces instead of tabs", OptionValue::bool(true))
                .with_scope(OptionScope::Buffer),
        );

        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);

        kernel
            .options
            .set("expandtab", OptionValue::bool(false), OptionScopeId::Buffer(buffer_id))
            .expect("expandtab option should be settable");

        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InsertTab.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        assert_eq!(content.as_deref(), Some("\thello"));

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 1);
    }

    // =========================================================================
    // InsertNewline at beginning of line
    // =========================================================================

    #[test]
    fn test_insert_newline_at_beginning() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        // Cursor at column 0
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InsertNewline.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 2);
        assert_eq!(buf_read.line(0), Some(""));
        assert_eq!(buf_read.line(1), Some("hello"));

        drop(buf_read);
        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 1);
        assert_eq!(window.cursor.column, 0);
    }

    // =========================================================================
    // InsertTab with custom tabstop
    // =========================================================================

    #[test]
    fn test_insert_tab_expandtab_custom_tabstop() {
        let kernel = create_test_context();

        let _ = kernel.options.register(
            OptionSpec::new("expandtab", "Use spaces instead of tabs", OptionValue::bool(true))
                .with_scope(OptionScope::Buffer),
        );
        let _ = kernel.options.register(
            OptionSpec::new("tabstop", "Number of spaces per tab", OptionValue::int(4))
                .with_scope(OptionScope::Buffer),
        );

        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);

        kernel
            .options
            .set("expandtab", OptionValue::bool(true), OptionScopeId::Buffer(buffer_id))
            .expect("expandtab option should be settable");
        kernel
            .options
            .set("tabstop", OptionValue::int(2), OptionScopeId::Buffer(buffer_id))
            .expect("tabstop option should be settable");

        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InsertTab.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        // tabstop=2, expandtab=true -> 2 spaces
        assert_eq!(content.as_deref(), Some("  hello"));

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 2);
    }

    // =========================================================================
    // InsertTab default (no options set, should use defaults)
    // =========================================================================

    #[test]
    fn test_insert_tab_default_options() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InsertTab.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        // Default: expandtab=true, tabstop=4 -> 4 spaces
        assert_eq!(content.as_deref(), Some("    hello"));

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 4);
    }

    // =========================================================================
    // InsertNewline in multiline buffer
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_insert_newline_in_middle_of_multiline() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(1, 4).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InsertNewline.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let buf_read = buf.read();
        assert_eq!(buf_read.line_count(), 4);
        assert_eq!(buf_read.line(1), Some("line"));
        assert_eq!(buf_read.line(2), Some(" 2"));
        drop(buf_read);

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.line, 2);
        assert_eq!(window.cursor.column, 0);
    }

    // =========================================================================
    // InsertTab in middle of text
    // =========================================================================

    #[cfg_attr(coverage_nightly, coverage(off))]
    #[test]
    fn test_insert_tab_in_middle_of_text() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("helloworld");
        let buffer_id = kernel.buffers.register(buffer);
        let mut state = TestState::with_window(buffer_id);
        if let Some(window) = state.windows.active_mut() {
            window.cursor = Position::new(0, 5).into();
        }
        let executor = StubExecutor;
        let mut runtime = state.runtime(&kernel, &executor);
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = InsertTab.execute(&mut runtime, &args);
        assert!(result.is_success());

        let buf = kernel.buffers.get(buffer_id).unwrap();
        let content = buf.read().line(0).map(str::to_owned);
        // Default: expandtab=true, tabstop=4 -> 4 spaces
        assert_eq!(content.as_deref(), Some("hello    world"));

        drop(runtime);
        let window = state.windows.active().unwrap();
        assert_eq!(window.cursor.column, 9); // 5 + 4
    }

    // =========================================================================
    // Metadata tests
    // =========================================================================

    #[test]
    fn test_insert_newline_args_empty() {
        let cmd = InsertNewline;
        let args = cmd.args();
        assert!(args.is_empty());
    }

    #[test]
    fn test_insert_tab_args_empty() {
        let cmd = InsertTab;
        let args = cmd.args();
        assert!(args.is_empty());
    }
}
