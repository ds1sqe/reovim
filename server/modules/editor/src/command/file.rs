//! File operations.
//!
//! Provides file operation commands:
//! - `WriteBufferCommand` (`:w`)

use std::path::Path;

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{BufferApi, SessionRuntime},
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

/// Write buffer to file.
///
/// Saves the current buffer's contents to a file. If no path is provided,
/// uses the buffer's current file path.
///
/// This command demonstrates VFS integration - file operations go through
/// the VFS abstraction, enabling test isolation and future remote FS support.
///
/// # Arguments
///
/// - `file`: Optional file path to save to (defaults to buffer's path)
///
/// # Examples
///
/// - `:w` - Save to current file
/// - `:w newfile.txt` - Save to specified file
#[derive(Debug, Clone, Copy, Default)]
pub struct WriteBufferCommand;

impl Command for WriteBufferCommand {
    fn id(&self) -> CommandId {
        ids::WRITE
    }

    fn description(&self) -> &'static str {
        "Write buffer to file"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "file",
            ArgKind::FilePath,
            "Target file path (optional)",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["w", "write"]
    }
}

impl CommandHandler for WriteBufferCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Get VFS from context
        let Some(vfs) = args.vfs() else {
            return CommandResult::error("VFS not available");
        };

        // Get active buffer
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get content and file path from buffer via API
        let Some(content) = runtime.buffer_content(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };
        let buffer_file_path = runtime.buffer_file_path(buffer_id);

        // Try to get path from args first, then from buffer
        let file_path_str = args.string("file").map(String::from).or(buffer_file_path);

        let Some(file_path_str) = file_path_str else {
            return CommandResult::error("No file path specified");
        };

        let file_path = Path::new(&file_path_str);

        // Write to file via VFS
        match vfs.write(file_path, content.as_bytes()) {
            Ok(()) => {
                // Clear modified flag via API
                runtime.set_buffer_modified(buffer_id, false);
                CommandResult::Success
            }
            Err(e) => CommandResult::error(&format!("Write failed: {e}")),
        }
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
        reovim_driver_vfs::{MockVfs, VfsDriver},
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
        registers: RegisterBank,
        clipboard_history: HistoryRing,
        local_marks: MarkBank,
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
                registers: RegisterBank::new(),
                clipboard_history: HistoryRing::new(),
                local_marks: MarkBank::new(),
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
    fn test_write_command_names() {
        let cmd = WriteBufferCommand;
        let names = cmd.names();
        assert!(names.contains(&"w"));
        assert!(names.contains(&"write"));
    }

    #[test]
    fn test_write_no_vfs_returns_error() {
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
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
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
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
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
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
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello world");
        let buffer_id = kernel.buffers.register(buffer);
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
        let kernel = create_test_context();
        let mut buffer = Buffer::from_string("buffer content");
        buffer.set_file_path(Some("/tmp/buffer_file.txt".to_string()));
        let buffer_id = kernel.buffers.register(buffer);
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
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
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
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
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
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
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
        let kernel = create_test_context();
        let mut buffer = Buffer::from_string("content");
        buffer.set_modified(true);
        let buffer_id = kernel.buffers.register(buffer);
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
        let kernel = create_test_context();
        let mut buffer = Buffer::from_string("content");
        buffer.set_file_path(Some("/tmp/original.txt".to_string()));
        let buffer_id = kernel.buffers.register(buffer);
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
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
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
        let kernel = create_test_context();
        let buffer = Buffer::from_string("hello");
        let buffer_id = kernel.buffers.register(buffer);
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
        let kernel = create_test_context();
        let buffer = Buffer::from_string("line 1\nline 2\nline 3");
        let buffer_id = kernel.buffers.register(buffer);
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
}
