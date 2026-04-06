use {
    super::super::*,
    reovim_driver_command::{ArgKind, Command, CommandContext, CommandResult},
    reovim_driver_session::{
        ClientId, ExtensionMap, Jumplist, MarkBank, Session, SessionRuntime, TextBufferRegistry,
        Window, WindowLayout, testing::StubExecutor,
    },
    reovim_driver_vfs::{MockVfs, VfsDriver},
    reovim_kernel::{
        api::v1::{BufferId, BufferOps, KernelBuffer, KernelContext, ModeStack, RwLock},
        testing::{create_test_context, test_mode},
    },
    reovim_provider_text::Buffer,
    reovim_types_text::{HistoryRing, RegisterBank},
    std::sync::Arc,
};

/// Create a test kernel with a `TextBufferRegistry` in services.
#[cfg_attr(coverage_nightly, coverage(off))]
fn setup_kernel() -> KernelContext {
    let kernel = create_test_context();
    kernel.services.register(Arc::new(TextBufferRegistry::new()));
    kernel
}

/// Register a buffer in both kernel (byte-level) and text registry.
#[cfg_attr(coverage_nightly, coverage(off))]
fn register_buffer(kernel: &KernelContext, buffer: Buffer) -> BufferId {
    let arc = Arc::new(RwLock::new(buffer));
    let id = kernel
        .buffers
        .register(arc.clone() as Arc<RwLock<dyn KernelBuffer>>);
    kernel
        .services
        .get::<TextBufferRegistry>()
        .unwrap()
        .register(arc as Arc<RwLock<dyn BufferOps>>);
    id
}

struct TestState {
    session: Session,
    mode_stack: ModeStack,
    windows: WindowLayout,
    extensions: ExtensionMap,
    compositor: Option<Box<dyn reovim_driver_layout::RootCompositor>>,
    tabs: reovim_driver_session::TabPageSet,
    registers: RegisterBank,
    clipboard_history: HistoryRing,
    local_marks: MarkBank,
    jumplist: Jumplist,
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
            jumplist: Jumplist::new(),
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
                jumplist: &mut self.jumplist,
                active_buffer: &mut self.active_buffer,
                terminal_size: &mut self.terminal_size,
            },
            kernel,
            executor,
        )
    }
}

#[test]
fn test_write_command_id() {
    let cmd = WriteBufferCommand;
    assert_eq!(cmd.id().name(), "write");
}

#[test]
fn test_write_command_description() {
    let cmd = WriteBufferCommand;
    assert_eq!(cmd.description(), "Write buffer to file");
}

#[test]
fn test_write_command_args() {
    let cmd = WriteBufferCommand;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "file");
    assert_eq!(args[0].kind, ArgKind::FilePath);
}

#[test]
fn test_write_command_names_empty() {
    let cmd = WriteBufferCommand;
    // WriteBufferCommand has no user-facing names — WriteCommand owns ["w", "write"]
    assert!(cmd.names().is_empty());
}

#[test]
fn test_write_no_vfs_returns_error() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    // No VFS set
    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_write_no_buffer_returns_error() {
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
    let mut jumplist = Jumplist::new();
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
    let args = CommandContext::new().with_vfs(vfs);
    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_write_no_file_path_returns_error() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
    let mut args = CommandContext::new().with_vfs(vfs);
    args.set_buffer_id(buffer_id);
    // No file path set (neither in args nor in buffer)
    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_write_buffer_not_found_returns_error() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
    let mut args = CommandContext::new().with_vfs(vfs);
    // Use invalid buffer_id
    args.set("buffer_id", reovim_driver_command::ArgValue::BufferId(99999));
    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert!(result.is_error());
}

#[test]
fn test_write_success_with_file_path_in_args() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello world");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mock_vfs = Arc::new(MockVfs::new());
    let vfs: Arc<dyn VfsDriver> = mock_vfs.clone();
    let mut args = CommandContext::new().with_vfs(vfs);
    args.set_buffer_id(buffer_id);
    args.set(
        "file",
        reovim_driver_command::ArgValue::String("/tmp/test_output.txt".to_string()),
    );

    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify the file was written through VFS
    let write_calls = mock_vfs.write_calls();
    assert_eq!(write_calls.len(), 1);
    assert_eq!(write_calls[0].0, std::path::PathBuf::from("/tmp/test_output.txt"));
    assert_eq!(write_calls[0].1, b"hello world");
}

#[test]
fn test_write_success_with_file_path_from_buffer() {
    let kernel = setup_kernel();
    let mut buffer = Buffer::from_string("buffer content");
    buffer.set_file_path(Some("/tmp/buffer_file.txt".to_string()));
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mock_vfs = Arc::new(MockVfs::new());
    let vfs: Arc<dyn VfsDriver> = mock_vfs.clone();
    let mut args = CommandContext::new().with_vfs(vfs);
    args.set_buffer_id(buffer_id);
    // No "file" arg - should use buffer's file_path

    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert!(result.is_success());

    let write_calls = mock_vfs.write_calls();
    assert_eq!(write_calls.len(), 1);
    assert_eq!(write_calls[0].0, std::path::PathBuf::from("/tmp/buffer_file.txt"));
}

#[test]
fn test_write_vfs_error_returns_error() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mock_vfs = Arc::new(MockVfs::new());
    mock_vfs.set_error("/tmp/readonly.txt", reovim_driver_vfs::MockErrorKind::PermissionDenied);
    let vfs: Arc<dyn VfsDriver> = mock_vfs;
    let mut args = CommandContext::new().with_vfs(vfs);
    args.set_buffer_id(buffer_id);
    args.set("file", reovim_driver_command::ArgValue::String("/tmp/readonly.txt".to_string()));

    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert!(result.is_error());
}

// =========================================================================
// Default trait and metadata tests
// =========================================================================

#[test]
fn test_write_command_debug() {
    let cmd = WriteBufferCommand;
    let debug_str = format!("{cmd:?}");
    assert!(debug_str.contains("WriteBufferCommand"));
}

// =========================================================================
// Error message content tests
// =========================================================================

#[test]
fn test_write_no_vfs_error_message() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mut args = CommandContext::new();
    args.set_buffer_id(buffer_id);
    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::error("VFS not available"));
}

#[test]
fn test_write_no_buffer_error_message() {
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
    let mut jumplist = Jumplist::new();
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
            jumplist: &mut jumplist,
            active_buffer: &mut active_buffer,
            terminal_size: &mut terminal_size,
        },
        &kernel,
        &executor,
    );
    let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
    let args = CommandContext::new().with_vfs(vfs);
    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::error("No active buffer"));
}

#[test]
fn test_write_no_file_path_error_message() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
    let mut args = CommandContext::new().with_vfs(vfs);
    args.set_buffer_id(buffer_id);
    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::error("No file path specified"));
}

// =========================================================================
// Write clears modified flag
// =========================================================================

#[test]
fn test_write_success_clears_modified_flag() {
    let kernel = setup_kernel();
    let mut buffer = Buffer::from_string("content");
    buffer.set_modified(true);
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mock_vfs = Arc::new(MockVfs::new());
    let vfs: Arc<dyn VfsDriver> = mock_vfs;
    let mut args = CommandContext::new().with_vfs(vfs);
    args.set_buffer_id(buffer_id);
    args.set("file", reovim_driver_command::ArgValue::String("/tmp/test.txt".to_string()));

    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify modified flag is cleared
    let buf = kernel.buffers.get(buffer_id).unwrap();
    let buf_read = buf.read();
    assert!(!buf_read.is_modified());
    drop(buf_read);
}

// =========================================================================
// File path from args overrides buffer path
// =========================================================================

#[test]
fn test_write_args_file_path_overrides_buffer_path() {
    let kernel = setup_kernel();
    let mut buffer = Buffer::from_string("content");
    buffer.set_file_path(Some("/tmp/original.txt".to_string()));
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mock_vfs = Arc::new(MockVfs::new());
    let vfs: Arc<dyn VfsDriver> = mock_vfs.clone();
    let mut args = CommandContext::new().with_vfs(vfs);
    args.set_buffer_id(buffer_id);
    args.set("file", reovim_driver_command::ArgValue::String("/tmp/override.txt".to_string()));

    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert!(result.is_success());

    // Verify the file was written to the args path, not the buffer path
    let write_calls = mock_vfs.write_calls();
    assert_eq!(write_calls.len(), 1);
    assert_eq!(write_calls[0].0, std::path::PathBuf::from("/tmp/override.txt"));
}

// =========================================================================
// VFS error message propagation
// =========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_write_vfs_error_message_propagated() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mock_vfs = Arc::new(MockVfs::new());
    mock_vfs.set_error("/tmp/fail.txt", reovim_driver_vfs::MockErrorKind::PermissionDenied);
    let vfs: Arc<dyn VfsDriver> = mock_vfs;
    let mut args = CommandContext::new().with_vfs(vfs);
    args.set_buffer_id(buffer_id);
    args.set("file", reovim_driver_command::ArgValue::String("/tmp/fail.txt".to_string()));

    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert!(result.is_error());
    if let CommandResult::Error(msg) = result {
        assert!(msg.starts_with("Write failed:"));
    }
}

// =========================================================================
// Buffer not found with valid VFS
// =========================================================================

#[test]
fn test_write_buffer_not_found_with_vfs_and_path() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("hello");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let vfs: Arc<dyn VfsDriver> = Arc::new(MockVfs::new());
    let mut args = CommandContext::new().with_vfs(vfs);
    // Set a buffer_id that doesn't exist
    args.set("buffer_id", reovim_driver_command::ArgValue::BufferId(99999));
    args.set("file", reovim_driver_command::ArgValue::String("/tmp/test.txt".to_string()));
    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert_eq!(result, CommandResult::error("Buffer not found"));
}

// =========================================================================
// Write with multiline content
// =========================================================================

#[test]
fn test_write_multiline_content() {
    let kernel = setup_kernel();
    let buffer = Buffer::from_string("line 1\nline 2\nline 3");
    let buffer_id = register_buffer(&kernel, buffer);
    let mut state = TestState::with_window(buffer_id);
    let executor = StubExecutor;
    let mut runtime = state.runtime(&kernel, &executor);
    let mock_vfs = Arc::new(MockVfs::new());
    let vfs: Arc<dyn VfsDriver> = mock_vfs.clone();
    let mut args = CommandContext::new().with_vfs(vfs);
    args.set_buffer_id(buffer_id);
    args.set("file", reovim_driver_command::ArgValue::String("/tmp/multi.txt".to_string()));

    let result = WriteBufferCommand.execute(&mut runtime, &args);
    assert!(result.is_success());

    let write_calls = mock_vfs.write_calls();
    assert_eq!(write_calls.len(), 1);
    let written_content = String::from_utf8_lossy(&write_calls[0].1);
    assert!(written_content.contains("line 1"));
    assert!(written_content.contains("line 2"));
    assert!(written_content.contains("line 3"));
}
